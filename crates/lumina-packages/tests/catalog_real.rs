//! Installs catalog entries from their real download URLs, checks the
//! SHA-256, unpacks them and runs each program once. Opt-in, as it downloads
//! tens of megabytes per package:
//!
//! ```powershell
//! $env:LUMINA_TEST_DOWNLOADS = "mailpit,bun,deno,uv,pnpm"   # or "all"
//! cargo test -p lumina-packages --test catalog_real -- --nocapture
//! ```
//!
//! Only packages with a release for this system are tried. With
//! `LUMINA_TEST_DOWNLOADS_DIR` set, installs go there and are kept (an
//! existing install is reused), for the real-service tests in
//! lumina-services.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use lumina_packages::{Catalog, PackageManager};

/// (package, program inside the install, argument that prints a version).
const PROGRAMS: &[(&str, &str, &str)] = &[
    ("mailpit", "mailpit", "version"),
    ("bun", "bun", "--version"),
    ("deno", "deno", "--version"),
    ("uv", "uv", "--version"),
    ("pnpm", "pnpm", "--version"),
    ("meilisearch", "meilisearch", "--version"),
    ("mongodb", "bin/mongod", "--version"),
    ("seaweedfs", "weed", "version"),
    ("valkey", "bin/valkey-server", "--version"),
    ("typesense", "typesense-server", "--version"),
    ("cloudflared", "cloudflared", "--version"),
];

#[tokio::test]
async fn new_catalog_entries_install_and_run() {
    let Ok(wanted) = std::env::var("LUMINA_TEST_DOWNLOADS") else {
        return;
    };
    let catalog = Catalog::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/catalog/packages.json"),
    )
    .expect("catalog");
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = std::env::var_os("LUMINA_TEST_DOWNLOADS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| tmp.path().to_path_buf());
    let manager = PackageManager::new(
        root.join("bin"),
        root.join("downloads"),
        catalog.clone(),
        None,
    )
    .expect("manager");
    let cancel = Arc::new(AtomicBool::new(false));
    let (os, arch) = lumina_packages::catalog::platform();

    let mut tried = 0;
    for (id, program, version_arg) in PROGRAMS {
        if wanted != "all" && !wanted.split(',').any(|w| w.trim() == *id) {
            continue;
        }
        let Some(package) = catalog.package(id) else {
            panic!("{id} is not in the catalog");
        };
        let Some(release) = package.versions.iter().find(|r| r.fits(os, arch)) else {
            println!("{id}: no release for {os}/{arch}, skipped");
            continue;
        };
        let existing = manager
            .installed(id)
            .into_iter()
            .find(|i| i.version == release.version);
        let installed = match existing {
            Some(installed) => installed,
            None => manager
                .install(id, &release.version, &cancel, &|_| {})
                .await
                .unwrap_or_else(|e| panic!("{id} {}: {e}", release.version)),
        };
        let exe = installed
            .dir
            .join(format!("{program}{}", std::env::consts::EXE_SUFFIX));
        assert!(exe.is_file(), "{id}: {} is missing", exe.display());
        // Outside this repository: pnpm would follow its `packageManager` pin.
        let out = Command::new(&exe)
            .arg(version_arg)
            .current_dir(tmp.path())
            .output()
            .unwrap_or_else(|e| panic!("{id}: run {}: {e}", exe.display()));
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let short = release
            .version
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".");
        assert!(
            text.contains(&short),
            "{id}: `{version_arg}` printed {text:?}"
        );
        println!("{id} {}: ok", release.version);
        tried += 1;
    }
    assert!(tried > 0, "nothing matched LUMINA_TEST_DOWNLOADS={wanted}");
}
