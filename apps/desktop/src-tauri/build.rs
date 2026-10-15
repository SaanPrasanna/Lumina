fn main() {
    tauri_build::build();

    // tauri-build embeds the Windows app manifest (Common Controls v6) only
    // into the app executable. Test executables link the same code and fail to
    // start without it (STATUS_ENTRYPOINT_NOT_FOUND), so they declare the
    // dependency themselves.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
}
