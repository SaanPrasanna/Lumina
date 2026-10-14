//! Issues/renews leaf certs (SAN: domain + *.domain) signed by the local CA.
//!
//! Valid 397 days (browsers reject longer server certificates). A certificate
//! is reissued when it is missing or unreadable, has fewer than 30 days left,
//! was not signed by the current CA, or lists different names.

use std::collections::BTreeSet;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use rcgen::{
    CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair, KeyUsagePurpose,
    SanType,
};
use serde::Serialize;
use time::{Duration, OffsetDateTime};
use x509_parser::extensions::GeneralName;

use crate::ca::{Ca, random_serial};
use crate::{Error, Result};

pub const LEAF_VALIDITY_DAYS: i64 = 397;
pub const RENEW_BEFORE_DAYS: i64 = 30;

/// A site certificate on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leaf {
    pub cert: PathBuf,
    pub key: PathBuf,
    /// Unix seconds.
    pub not_after: i64,
    /// Upper-case hex serial; changes with every reissue, so configs that
    /// mention it change too and the server reloads the new certificate.
    pub serial: String,
}

/// What a certificate must contain: DNS names and IP addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Names {
    pub dns: Vec<String>,
    pub ips: Vec<IpAddr>,
}

impl Names {
    /// `blog.test` and `*.blog.test`.
    pub fn site(domain: &str) -> Self {
        Self {
            dns: vec![domain.to_owned(), format!("*.{domain}")],
            ips: Vec::new(),
        }
    }

    /// `localhost`, `127.0.0.1`, `::1`.
    pub fn localhost() -> Self {
        Self {
            dns: vec!["localhost".to_owned()],
            ips: vec![
                IpAddr::from([127, 0, 0, 1]),
                IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1]),
            ],
        }
    }

    fn set(&self) -> BTreeSet<String> {
        self.dns
            .iter()
            .cloned()
            .chain(self.ips.iter().map(ToString::to_string))
            .collect()
    }
}

/// Why an existing certificate is replaced (for the log).
fn needs_reissue(cert_pem: &str, ca: &Ca, names: &Names, now: i64) -> Option<String> {
    let Ok((_, pem)) = x509_parser::pem::parse_x509_pem(cert_pem.as_bytes()) else {
        return Some("unreadable".into());
    };
    let Ok((_, cert)) = x509_parser::parse_x509_certificate(&pem.contents) else {
        return Some("unreadable".into());
    };
    let left = cert.validity().not_after.timestamp() - now;
    if left < RENEW_BEFORE_DAYS * 86400 {
        return Some(format!("expires in {} days", left / 86400));
    }
    let Ok((_, ca_cert)) = x509_parser::parse_x509_certificate(&ca.info.der) else {
        return Some("CA unreadable".into());
    };
    if cert.verify_signature(Some(ca_cert.public_key())).is_err() {
        return Some("signed by another CA".into());
    }
    let mut have = BTreeSet::new();
    if let Ok(Some(san)) = cert.subject_alternative_name() {
        for name in &san.value.general_names {
            match name {
                GeneralName::DNSName(d) => {
                    have.insert((*d).to_owned());
                }
                GeneralName::IPAddress(bytes) => {
                    let ip = match bytes.len() {
                        4 => <[u8; 4]>::try_from(*bytes).ok().map(IpAddr::from),
                        16 => <[u8; 16]>::try_from(*bytes).ok().map(IpAddr::from),
                        _ => None,
                    };
                    if let Some(ip) = ip {
                        have.insert(ip.to_string());
                    }
                }
                _ => {}
            }
        }
    }
    (have != names.set()).then(|| "names changed".into())
}

fn info(cert_pem: &str, cert: &Path, key: &Path) -> Result<Leaf> {
    let (_, pem) = x509_parser::pem::parse_x509_pem(cert_pem.as_bytes())
        .map_err(|e| Error::Invalid(format!("{}: {e}", cert.display())))?;
    let (_, parsed) = x509_parser::parse_x509_certificate(&pem.contents)
        .map_err(|e| Error::Invalid(format!("{}: {e}", cert.display())))?;
    Ok(Leaf {
        cert: cert.to_path_buf(),
        key: key.to_path_buf(),
        not_after: parsed.validity().not_after.timestamp(),
        serial: parsed
            .raw_serial()
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect(),
    })
}

/// Reads an existing leaf without changing anything.
pub fn read(cert: &Path, key: &Path) -> Option<Leaf> {
    let pem = std::fs::read_to_string(cert).ok()?;
    key.is_file().then_some(())?;
    info(&pem, cert, key).ok()
}

/// Makes sure `<stem>.crt`/`.key` in `dir` is a valid certificate for
/// `names` from `ca`, issuing a new one if needed. Returns it and whether it
/// was (re)issued.
pub fn ensure(dir: &Path, stem: &str, names: &Names, ca: &Ca) -> Result<(Leaf, bool)> {
    let cert_path = dir.join(format!("{stem}.crt"));
    let key_path = dir.join(format!("{stem}.key"));
    let now = OffsetDateTime::now_utc().unix_timestamp();

    if let (Ok(pem), true) = (std::fs::read_to_string(&cert_path), key_path.is_file()) {
        match needs_reissue(&pem, ca, names, now) {
            None => return Ok((info(&pem, &cert_path, &key_path)?, false)),
            Some(why) => tracing::info!("reissuing the certificate for {stem}: {why}"),
        }
    }

    let (cert_pem, key_pem) = issue(names, ca)?;
    lumina_platform::secure_file::write_private(&key_path, key_pem.as_bytes()).map_err(
        |source| Error::Io {
            path: key_path.clone(),
            source,
        },
    )?;
    lumina_core::fs::write_atomic(&cert_path, cert_pem.as_bytes())
        .map_err(|e| Error::Invalid(e.to_string()))?;
    Ok((info(&cert_pem, &cert_path, &key_path)?, true))
}

fn issue(names: &Names, ca: &Ca) -> Result<(String, String)> {
    let key = KeyPair::generate().map_err(Error::rcgen)?;
    let mut params = CertificateParams::default();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params.distinguished_name.push(
        DnType::CommonName,
        names.dns.first().cloned().unwrap_or_default(),
    );
    let mut sans = Vec::new();
    for dns in &names.dns {
        sans.push(SanType::DnsName(
            dns.clone().try_into().map_err(Error::rcgen)?,
        ));
    }
    sans.extend(names.ips.iter().copied().map(SanType::IpAddress));
    params.subject_alt_names = sans;
    params.is_ca = IsCa::ExplicitNoCa;
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.use_authority_key_identifier_extension = true;
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::hours(1);
    params.not_after = now + Duration::days(LEAF_VALIDITY_DAYS);
    params.serial_number = Some(random_serial());

    let issuer = Issuer::from_ca_cert_pem(&ca.cert_pem, &ca.key).map_err(Error::rcgen)?;
    let cert = params.signed_by(&key, &issuer).map_err(Error::rcgen)?;
    Ok((cert.pem(), key.serialize_pem()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ca;

    fn new_ca(dir: &Path) -> Ca {
        let (pem, key) = ca::create("test", &ca::suffixes(&[])).expect("ca");
        let cert = dir.join("ca.crt");
        let key_path = dir.join("ca.key");
        std::fs::write(&cert, &pem).expect("w");
        std::fs::write(&key_path, key.serialize_pem()).expect("w");
        ca::load(&cert, &key_path).expect("load").expect("some")
    }

    #[test]
    fn issues_once_then_reuses_and_reissues_on_change() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ca = new_ca(tmp.path());
        let sites = tmp.path().join("sites");

        let (leaf, issued) =
            ensure(&sites, "blog.test", &Names::site("blog.test"), &ca).expect("issue");
        assert!(issued);
        let days = (leaf.not_after - OffsetDateTime::now_utc().unix_timestamp()) / 86400;
        assert!((396..=397).contains(&days), "{days}");

        let (again, issued) =
            ensure(&sites, "blog.test", &Names::site("blog.test"), &ca).expect("reuse");
        assert!(!issued);
        assert_eq!(again.serial, leaf.serial);

        // Different names → new certificate.
        let names = Names {
            dns: vec!["blog.test".into()],
            ips: vec![],
        };
        assert!(ensure(&sites, "blog.test", &names, &ca).expect("names").1);

        // A new CA → every certificate is reissued.
        let other_dir = tmp.path().join("other");
        std::fs::create_dir_all(&other_dir).expect("mkdir");
        let other = new_ca(&other_dir);
        assert!(
            ensure(&sites, "blog.test", &names, &other)
                .expect("ca changed")
                .1
        );
    }

    #[test]
    fn leaf_chains_to_the_ca_with_the_right_names() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ca = new_ca(tmp.path());
        let (leaf, _) = ensure(tmp.path(), "localhost", &Names::localhost(), &ca).expect("issue");
        let pem = std::fs::read_to_string(&leaf.cert).expect("read");
        assert_eq!(needs_reissue(&pem, &ca, &Names::localhost(), 0), None);

        let (_, parsed) = x509_parser::pem::parse_x509_pem(pem.as_bytes()).expect("pem");
        let (_, cert) = x509_parser::parse_x509_certificate(&parsed.contents).expect("parse");
        let eku = cert.extended_key_usage().expect("eku").expect("present");
        assert!(eku.value.server_auth);
        assert!(!cert.is_ca());
    }

    #[test]
    fn expiring_certificates_are_renewed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ca = new_ca(tmp.path());
        let (leaf, _) = ensure(tmp.path(), "a.test", &Names::site("a.test"), &ca).expect("issue");
        let pem = std::fs::read_to_string(&leaf.cert).expect("read");
        // As if 370 days had passed: 27 days left.
        let later = OffsetDateTime::now_utc().unix_timestamp() + 370 * 86400;
        assert!(
            needs_reissue(&pem, &ca, &Names::site("a.test"), later)
                .is_some_and(|why| why.starts_with("expires in"))
        );
    }
}
