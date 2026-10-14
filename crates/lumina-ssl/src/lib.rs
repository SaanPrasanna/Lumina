//! Local root CA creation, per-domain leaf certificates, OS trust installation.
//!
//! Files in `<data-root>/ssl/`:
//! - `ca.crt`, `ca.key` — the local CA (key readable by the user only)
//! - `sites/<domain>.crt|.key` — one certificate per site, plus `localhost`

pub mod ca;
pub mod leaf;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub use ca::{Ca, CaInfo};
pub use leaf::{Leaf, Names};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("certificate error: {0}")]
    Invalid(String),
}

impl Error {
    pub(crate) fn rcgen(e: rcgen::Error) -> Self {
        Self::Invalid(e.to_string())
    }
}

impl From<Error> for lumina_core::Error {
    fn from(e: Error) -> Self {
        match e {
            Error::Io { path, source } => lumina_core::Error::Io { path, source },
            Error::Invalid(m) => lumina_core::Error::Other(m),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// `len` random letters and digits from the system's secure RNG.
pub fn random_token(len: usize) -> String {
    use ring::rand::SecureRandom;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let rng = ring::rand::SystemRandom::new();
    let mut out = String::with_capacity(len);
    let mut byte = [0u8; 1];
    while out.len() < len {
        if rng.fill(&mut byte).is_err() {
            continue;
        }
        // Rejection sampling: 248 = 4 * 62, so every character is equally likely.
        if byte[0] < 248 {
            out.push(CHARS[(byte[0] % 62) as usize] as char);
        }
    }
    out
}

/// HTTPS settings handed to the web servers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tls {
    pub https_port: u16,
    /// Sites answer plain HTTP with a redirect to HTTPS.
    pub force_https: bool,
    pub localhost: Leaf,
    /// By domain.
    pub sites: BTreeMap<String, Leaf>,
}

/// The `ssl/` folder.
#[derive(Debug, Clone)]
pub struct Ssl {
    dir: PathBuf,
}

impl Ssl {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn ca_cert_path(&self) -> PathBuf {
        self.dir.join("ca.crt")
    }

    pub fn ca_key_path(&self) -> PathBuf {
        self.dir.join("ca.key")
    }

    fn sites_dir(&self) -> PathBuf {
        self.dir.join("sites")
    }

    /// The CA, if one has been created.
    pub fn ca(&self) -> Result<Option<Ca>> {
        ca::load(&self.ca_cert_path(), &self.ca_key_path())
    }

    /// The existing CA, or a new one for `hostname` covering `tld`.
    /// Returns the CA and whether it was just created.
    pub fn ensure_ca(&self, hostname: &str, tld: &str) -> Result<(Ca, bool)> {
        if let Some(ca) = self.ca()? {
            return Ok((ca, false));
        }
        Ok((self.create_ca(hostname, tld)?, true))
    }

    /// Creates a new CA (replacing any existing one) and deletes every site
    /// certificate, which the old CA had signed.
    pub fn create_ca(&self, hostname: &str, tld: &str) -> Result<Ca> {
        let (cert_pem, key) = ca::create(hostname, &ca::suffixes(&[tld]))?;
        let key_path = self.ca_key_path();
        lumina_platform::secure_file::write_private(&key_path, key.serialize_pem().as_bytes())
            .map_err(|source| Error::Io {
                path: key_path,
                source,
            })?;
        lumina_core::fs::write_atomic(&self.ca_cert_path(), cert_pem.as_bytes())
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let _ = std::fs::remove_dir_all(self.sites_dir());
        tracing::info!("created a new local CA");
        self.ca()?
            .ok_or_else(|| Error::Invalid("the new CA could not be read back".into()))
    }

    /// Deletes the CA and every certificate.
    pub fn remove_all(&self) -> Result<()> {
        for path in [self.ca_cert_path(), self.ca_key_path()] {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => return Err(Error::Io { path, source }),
            }
        }
        let sites = self.sites_dir();
        if sites.exists() {
            std::fs::remove_dir_all(&sites).map_err(|source| Error::Io {
                path: sites,
                source,
            })?;
        }
        Ok(())
    }

    /// Certificates for `localhost` and every domain in `domains` (issued or
    /// renewed as needed); certificates of domains no longer listed are deleted.
    pub fn ensure_all(
        &self,
        ca: &Ca,
        domains: &[String],
    ) -> Result<(Leaf, BTreeMap<String, Leaf>)> {
        let dir = self.sites_dir();
        let (localhost, _) = leaf::ensure(&dir, "localhost", &Names::localhost(), ca)?;
        let mut sites = BTreeMap::new();
        for domain in domains {
            let (leaf, _) = leaf::ensure(&dir, domain, &Names::site(domain), ca)?;
            sites.insert(domain.clone(), leaf);
        }
        self.prune(domains);
        Ok((localhost, sites))
    }

    /// The certificate for `domain`, as it is on disk.
    pub fn site(&self, domain: &str) -> Option<Leaf> {
        let dir = self.sites_dir();
        leaf::read(
            &dir.join(format!("{domain}.crt")),
            &dir.join(format!("{domain}.key")),
        )
    }

    fn prune(&self, keep: &[String]) {
        let Ok(entries) = std::fs::read_dir(self.sites_dir()) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(stem) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".crt").or_else(|| n.strip_suffix(".key")))
            else {
                continue;
            };
            if stem != "localhost" && !keep.iter().any(|d| d == stem) {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

/// Builds CA certificates with arbitrary names and constraints, so other
/// crates can test that bad ones are refused. Not used by the app.
#[doc(hidden)]
pub mod rcgen_for_tests {
    use rcgen::{
        BasicConstraints, CertificateParams, DnType, GeneralSubtree, IsCa, KeyPair, NameConstraints,
    };

    pub fn ca_pem(common_name: &str, dns_constraints: Option<Vec<&str>>) -> String {
        let key = KeyPair::generate().expect("key");
        let mut params = CertificateParams::default();
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, common_name);
        params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        params.name_constraints = dns_constraints.map(|names| NameConstraints {
            permitted_subtrees: names
                .into_iter()
                .map(|n| GeneralSubtree::DnsName(n.to_owned()))
                .collect(),
            excluded_subtrees: Vec::new(),
        });
        params.self_signed(&key).expect("sign").pem()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ca_lifecycle_and_site_certificates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ssl = Ssl::new(tmp.path().join("ssl"));
        assert!(ssl.ca().expect("none").is_none());

        let (ca, created) = ssl.ensure_ca("PC", "test").expect("create");
        assert!(created);
        let (again, created) = ssl.ensure_ca("PC", "test").expect("reuse");
        assert!(!created);
        assert_eq!(again.info.thumbprint, ca.info.thumbprint);

        let domains = vec!["blog.test".to_owned(), "shop.test".to_owned()];
        let (localhost, sites) = ssl.ensure_all(&ca, &domains).expect("issue");
        assert!(localhost.cert.is_file());
        assert_eq!(sites.len(), 2);
        assert!(ssl.site("blog.test").is_some());

        // shop.test is gone → its files go too.
        ssl.ensure_all(&ca, &domains[..1]).expect("prune");
        assert!(ssl.site("shop.test").is_none());
        assert!(!tmp.path().join("ssl/sites/shop.test.key").exists());

        // A new CA invalidates all site certificates.
        let new = ssl.create_ca("PC", "test").expect("regenerate");
        assert_ne!(new.info.thumbprint, ca.info.thumbprint);
        assert!(ssl.site("blog.test").is_none());

        ssl.remove_all().expect("remove");
        assert!(ssl.ca().expect("gone").is_none());
    }
}
