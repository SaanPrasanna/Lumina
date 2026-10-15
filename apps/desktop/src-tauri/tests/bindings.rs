//! `src/lib/bindings.ts` is generated from the Rust commands; this fails when
//! it is out of date. Regenerate it with
//! `LUMINA_UPDATE_BINDINGS=1 cargo test -p lumina-desktop --test bindings`.

use std::path::Path;

#[test]
fn bindings_are_up_to_date() {
    // Walking ~100 nested types recurses deeply; Windows threads have small stacks.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(check)
        .expect("thread")
        .join()
        .expect("bindings check");
}

fn check() {
    let checked_in = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/bindings.ts");
    let tmp = tempfile::tempdir().expect("tempdir");
    let fresh = tmp.path().join("bindings.ts");
    lumina_desktop_lib::specta_builder()
        .export(
            specta_typescript::Typescript::default().header(
                "// Generated from the Rust commands by tauri-specta; do not edit.\n\
                 // Regenerate: LUMINA_UPDATE_BINDINGS=1 cargo test -p lumina-desktop --test bindings",
            ),
            &fresh,
        )
        .expect("export bindings");
    let fresh = std::fs::read_to_string(&fresh)
        .expect("read fresh")
        .replace("\r\n", "\n");
    if std::env::var_os("LUMINA_UPDATE_BINDINGS").is_some() {
        std::fs::write(&checked_in, &fresh).expect("write bindings");
        return;
    }
    let current = std::fs::read_to_string(&checked_in)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        current == fresh,
        "src/lib/bindings.ts is out of date: run `LUMINA_UPDATE_BINDINGS=1 cargo test -p lumina-desktop --test bindings`"
    );
}
