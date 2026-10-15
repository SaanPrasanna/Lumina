//! Update packages are only installed when their minisign signature matches
//! the public key in tauri.conf.json (tauri-plugin-updater checks it with
//! minisign-verify, as here). With `LUMINA_TEST_SIGNED_FILE=<installer>` (and
//! `<installer>.sig` next to it, as `tauri build` writes), a real release
//! package is checked too, and a one-byte change must be refused.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use minisign_verify::{PublicKey, Signature};

/// The updater's public key, as the app ships it.
fn public_key() -> PublicKey {
    let conf: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
    let encoded = conf["plugins"]["updater"]["pubkey"]
        .as_str()
        .expect("pubkey");
    let text = String::from_utf8(STANDARD.decode(encoded).expect("base64")).expect("utf-8");
    PublicKey::decode(&text).expect("a minisign public key")
}

fn signature(path: &str) -> Signature {
    let encoded = std::fs::read_to_string(format!("{path}.sig")).expect("signature file");
    let text = String::from_utf8(STANDARD.decode(encoded.trim()).expect("base64")).expect("utf-8");
    Signature::decode(&text).expect("a minisign signature")
}

#[test]
fn the_shipped_key_is_a_real_key() {
    public_key();
}

#[test]
fn a_tampered_package_is_refused() {
    let Ok(path) = std::env::var("LUMINA_TEST_SIGNED_FILE") else {
        eprintln!("skipped: set LUMINA_TEST_SIGNED_FILE to a signed release package");
        return;
    };
    let key = public_key();
    let sig = signature(&path);
    let mut bytes = std::fs::read(&path).expect("package");
    key.verify(&bytes, &sig, true)
        .expect("the release package matches its signature");

    let middle = bytes.len() / 2;
    bytes[middle] ^= 0x01;
    assert!(
        key.verify(&bytes, &sig, true).is_err(),
        "a changed package must be refused"
    );
}
