//! Notification texts follow the UI language.

use lumina_desktop_lib::notify::{Note, service_name, text};

#[test]
fn texts_follow_the_language() {
    let note = Note::Crashed { service: "nginx" };
    assert_eq!(text("en", &note).0, "nginx stopped unexpectedly");
    assert!(text("si", &note).0.starts_with("nginx "));
    assert_eq!(service_name("php-fcgi"), Some("PHP"));
    assert_eq!(service_name("ngrok"), None);
}
