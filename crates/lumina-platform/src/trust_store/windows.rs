//! trust_store: windows implementation (CurrentUser\Root through Crypt32;
//! adding or deleting a root there makes Windows ask the user to confirm).
#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows_sys::Win32::Foundation::{ERROR_CANCELLED, GetLastError};
use windows_sys::Win32::Security::Cryptography::{
    CERT_FIND_SHA1_HASH, CERT_STORE_ADD_REPLACE_EXISTING, CERT_STORE_PROV_SYSTEM_W,
    CERT_SYSTEM_STORE_CURRENT_USER, CRYPT_INTEGER_BLOB, CertAddEncodedCertificateToStore,
    CertCloseStore, CertDeleteCertificateFromStore, CertFindCertificateInStore,
    CertFreeCertificateContext, CertOpenStore, HCERTSTORE, PKCS_7_ASN_ENCODING, X509_ASN_ENCODING,
};

use super::{CaCert, TrustError};

/// HRESULT_FROM_WIN32(ERROR_CANCELLED): what Crypt32 reports when the user
/// answers "No".
const HRESULT_CANCELLED: u32 = 0x8007_04C7;

/// The current user's "Trusted Root Certification Authorities" store.
struct RootStore(HCERTSTORE);

impl RootStore {
    fn open() -> Result<Self, TrustError> {
        let name: Vec<u16> = OsStr::new("ROOT").encode_wide().chain([0]).collect();
        // SAFETY: `name` is NUL-terminated and outlives the call.
        let store = unsafe {
            CertOpenStore(
                CERT_STORE_PROV_SYSTEM_W,
                0,
                0,
                CERT_SYSTEM_STORE_CURRENT_USER,
                name.as_ptr().cast(),
            )
        };
        if store.is_null() {
            return Err(TrustError::Failed(format!(
                "cannot open the certificate store: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(Self(store))
    }
}

impl Drop for RootStore {
    fn drop(&mut self) {
        // SAFETY: the handle came from CertOpenStore and is closed once.
        unsafe { CertCloseStore(self.0, 0) };
    }
}

fn last_error() -> TrustError {
    // SAFETY: plain thread-local read.
    let code = unsafe { GetLastError() };
    if code == ERROR_CANCELLED || code == HRESULT_CANCELLED {
        return TrustError::Declined;
    }
    TrustError::Failed(std::io::Error::from_raw_os_error(code as i32).to_string())
}

/// Runs `f` with the store's context for `sha1`, or `None`.
fn with_cert<T>(
    store: &RootStore,
    sha1: &[u8; 20],
    f: impl FnOnce(*mut windows_sys::Win32::Security::Cryptography::CERT_CONTEXT) -> T,
) -> Option<T> {
    let blob = CRYPT_INTEGER_BLOB {
        cbData: 20,
        pbData: sha1.as_ptr().cast_mut(),
    };
    // SAFETY: `blob` points at 20 valid bytes for the duration of the call.
    let ctx = unsafe {
        CertFindCertificateInStore(
            store.0,
            X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
            0,
            CERT_FIND_SHA1_HASH,
            (&raw const blob).cast(),
            std::ptr::null(),
        )
    };
    (!ctx.is_null()).then(|| f(ctx))
}

pub(super) fn is_trusted(ca: &CaCert<'_>) -> bool {
    let Ok(store) = RootStore::open() else {
        return false;
    };
    with_cert(&store, &ca.sha1, |ctx| {
        // SAFETY: freeing the context returned by CertFindCertificateInStore.
        unsafe { CertFreeCertificateContext(ctx) };
    })
    .is_some()
}

pub(super) fn install(ca: &CaCert<'_>) -> Result<(), TrustError> {
    let store = RootStore::open()?;
    let len = u32::try_from(ca.der.len())
        .map_err(|_| TrustError::Failed("certificate too large".into()))?;
    // SAFETY: `der` is valid for `len` bytes; no context is requested back.
    let ok = unsafe {
        CertAddEncodedCertificateToStore(
            store.0,
            X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
            ca.der.as_ptr(),
            len,
            CERT_STORE_ADD_REPLACE_EXISTING,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(last_error());
    }
    Ok(())
}

pub(super) fn remove(ca: &CaCert<'_>) -> Result<(), TrustError> {
    let store = RootStore::open()?;
    // SAFETY: CertDeleteCertificateFromStore frees the context, success or not.
    let deleted = with_cert(&store, &ca.sha1, |ctx| unsafe {
        CertDeleteCertificateFromStore(ctx)
    });
    match deleted {
        None => Ok(()),
        Some(0) => Err(last_error()),
        Some(_) => Ok(()),
    }
}
