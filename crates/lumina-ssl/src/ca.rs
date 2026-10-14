//! Creates and persists the Lumina root CA; installs it via lumina_platform::trust_store.
//!
//! The CA is ECDSA P-256, valid 10 years, `CA:TRUE, pathlen:0`, and carries
//! **name constraints**: it can only vouch for the local suffixes (`.test`,
//! `.local`, `localhost`, the configured suffix) and loopback addresses. Even
//! if its key leaked, it could not be used to impersonate a real website.

use std::net::IpAddr;
use std::path::{Path, PathBuf};

use rcgen::{
    BasicConstraints, CertificateParams, CidrSubnet, DnType, GeneralSubtree, IsCa, KeyPair,
    KeyUsagePurpose, NameConstraints,
};
use serde::Serialize;
use time::{Duration, OffsetDateTime};
use x509_parser::extensions::{GeneralName, ParsedExtension};

use crate::{Error, Result};

/// Common name prefix; the helper checks it before installing a CA on Linux.
pub const CA_NAME_PREFIX: &str = "Lumina Local CA";
const CA_VALIDITY_DAYS: i64 = 3650;
/// Suffixes every Lumina CA covers, besides the configured one.
pub const BASE_SUFFIXES: [&str; 3] = ["test", "local", "localhost"];

/// What the UI and the trust store need to know about the CA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaInfo {
    pub subject: String,
    /// Unix seconds.
    pub not_after: i64,
    /// SHA-1 thumbprint, upper-case hex.
    pub thumbprint: String,
    /// DNS suffixes the CA may sign for.
    pub permitted: Vec<String>,
    pub cert_path: PathBuf,
    #[serde(skip)]
    pub der: Vec<u8>,
    #[serde(skip)]
    pub sha1: [u8; 20],
}

impl CaInfo {
    /// Whether certificates for `<anything>.<suffix>` will be accepted.
    pub fn covers(&self, suffix: &str) -> bool {
        self.permitted.iter().any(|p| p == suffix)
    }
}

/// A loaded CA: its info plus the PEM text and key needed to sign.
pub struct Ca {
    pub info: CaInfo,
    pub cert_pem: String,
    pub key: KeyPair,
}

pub fn sha1(der: &[u8]) -> [u8; 20] {
    let digest = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, der);
    let mut out = [0u8; 20];
    out.copy_from_slice(digest.as_ref());
    out
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>> {
    let (_, parsed) = x509_parser::pem::parse_x509_pem(pem.as_bytes())
        .map_err(|e| Error::Invalid(format!("not a PEM certificate: {e}")))?;
    Ok(parsed.contents)
}

/// Reads the facts Lumina needs from a CA certificate.
pub fn inspect(cert_pem: &str, cert_path: &Path) -> Result<CaInfo> {
    let der = pem_to_der(cert_pem)?;
    let (_, cert) = x509_parser::parse_x509_certificate(&der)
        .map_err(|e| Error::Invalid(format!("unreadable CA certificate: {e}")))?;
    let mut permitted = Vec::new();
    for ext in cert.extensions() {
        if let ParsedExtension::NameConstraints(nc) = ext.parsed_extension() {
            for subtree in nc.permitted_subtrees.iter().flatten() {
                if let GeneralName::DNSName(name) = subtree.base {
                    permitted.push(name.trim_start_matches('.').to_owned());
                }
            }
        }
    }
    let subject = cert
        .subject()
        .iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
        .unwrap_or_default()
        .to_owned();
    let sha1 = sha1(&der);
    Ok(CaInfo {
        subject,
        not_after: cert.validity().not_after.timestamp(),
        thumbprint: lumina_platform::trust_store::hex(&sha1),
        permitted,
        cert_path: cert_path.to_path_buf(),
        der,
        sha1,
    })
}

/// The DNS suffixes a new CA permits: the base ones plus `extra` (the
/// configured TLD), sorted and without duplicates.
pub fn suffixes(extra: &[&str]) -> Vec<String> {
    let mut all: Vec<String> = BASE_SUFFIXES
        .iter()
        .chain(extra)
        .map(|s| s.trim_start_matches('.').to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    all.sort();
    all.dedup();
    all
}

/// Creates a new CA for `hostname` permitted to sign for `suffixes`.
pub fn create(hostname: &str, suffixes: &[String]) -> Result<(String, KeyPair)> {
    let key = KeyPair::generate().map_err(Error::rcgen)?;
    let mut params = CertificateParams::default();
    params.distinguished_name = rcgen::DistinguishedName::new();
    let host = hostname.trim();
    let cn = if host.is_empty() {
        CA_NAME_PREFIX.to_owned()
    } else {
        format!("{CA_NAME_PREFIX} - {host}")
    };
    params.distinguished_name.push(DnType::CommonName, cn);
    params
        .distinguished_name
        .push(DnType::OrganizationName, "Lumina local development");
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::days(1);
    params.not_after = now + Duration::days(CA_VALIDITY_DAYS);
    params.serial_number = Some(random_serial());

    let mut permitted: Vec<GeneralSubtree> = suffixes
        .iter()
        .map(|s| GeneralSubtree::DnsName(s.clone()))
        .collect();
    // With an IP subtree present, only these addresses may appear in leaves.
    permitted.push(GeneralSubtree::IpAddress(CidrSubnet::from_addr_prefix(
        IpAddr::from([127, 0, 0, 0]),
        8,
    )));
    permitted.push(GeneralSubtree::IpAddress(CidrSubnet::from_addr_prefix(
        IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1]),
        128,
    )));
    params.name_constraints = Some(NameConstraints {
        permitted_subtrees: permitted,
        excluded_subtrees: Vec::new(),
    });

    let cert = params.self_signed(&key).map_err(Error::rcgen)?;
    Ok((cert.pem(), key))
}

pub(crate) fn random_serial() -> rcgen::SerialNumber {
    use ring::rand::SecureRandom;
    let mut bytes = [0u8; 16];
    // A failing system RNG would also have failed key generation.
    let _ = ring::rand::SystemRandom::new().fill(&mut bytes);
    bytes[0] &= 0x7f; // positive
    bytes[0] |= 0x01; // no leading zero byte
    rcgen::SerialNumber::from_slice(&bytes)
}

/// Loads `ca.crt` + `ca.key` from `dir`, if both exist.
pub fn load(cert_path: &Path, key_path: &Path) -> Result<Option<Ca>> {
    let (Ok(cert_pem), Ok(key_pem)) = (
        std::fs::read_to_string(cert_path),
        std::fs::read_to_string(key_path),
    ) else {
        return Ok(None);
    };
    let key = KeyPair::from_pem(&key_pem).map_err(Error::rcgen)?;
    let info = inspect(&cert_pem, cert_path)?;
    Ok(Some(Ca {
        info,
        cert_pem,
        key,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ca_is_constrained_to_local_names() {
        let (pem, _key) = create("DEV-PC", &suffixes(&["test", "dev-local"])).expect("create");
        let info = inspect(&pem, Path::new("ca.crt")).expect("inspect");
        assert_eq!(info.subject, "Lumina Local CA - DEV-PC");
        assert_eq!(info.permitted, ["dev-local", "local", "localhost", "test"]);
        assert!(info.covers("test") && !info.covers("com"));
        assert_eq!(info.thumbprint.len(), 40);

        let der = pem_to_der(&pem).expect("der");
        let (_, cert) = x509_parser::parse_x509_certificate(&der).expect("parse");
        let bc = cert.basic_constraints().expect("bc").expect("present");
        assert!(bc.critical && bc.value.ca);
        assert_eq!(bc.value.path_len_constraint, Some(0));
        let nc = cert.name_constraints().expect("nc").expect("present");
        assert!(nc.critical, "name constraints must be critical");
        let years = (cert.validity().not_after.timestamp()
            - cert.validity().not_before.timestamp())
            / (365 * 86400);
        assert_eq!(years, 10);
    }

    #[test]
    fn serials_are_positive_and_distinct() {
        let a = random_serial().to_bytes();
        let b = random_serial().to_bytes();
        assert_ne!(a, b);
        assert!(a[0] < 0x80 && a[0] != 0);
    }
}
