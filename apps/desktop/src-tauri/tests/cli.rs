//! Command-line flags.

use lumina_desktop_lib::cli::Flags;

#[test]
fn no_flags_show_the_window() {
    let flags = Flags::parse(Vec::<String>::new());
    assert_eq!(flags, Flags::default());
    assert!(flags.shows_window());
    assert!(!flags.only_stops());
}

#[test]
fn flags_are_read() {
    let flags = Flags::parse(["--minimized", "--start-profile", "Legacy", "-psn_0_123"]);
    assert!(flags.minimized);
    assert_eq!(flags.start_profile.as_deref(), Some("Legacy"));
    assert!(!flags.shows_window());

    assert_eq!(
        Flags::parse(["--start-profile=Modern"])
            .start_profile
            .as_deref(),
        Some("Modern")
    );
    assert_eq!(Flags::parse(["--start-profile"]).start_profile, None);
}

#[test]
fn stop_flags_never_open_the_window() {
    for flags in [
        Flags::parse(["--stop-all"]),
        Flags::parse(["--quit"]),
        Flags::parse(["--remove-data"]),
    ] {
        assert!(flags.only_stops());
        assert!(!flags.shows_window());
    }
    // Switching a profile is more than stopping.
    assert!(!Flags::parse(["--stop-all", "--start-profile", "x"]).only_stops());
}
