//! Native OS notifications: a service crashed, or a long task (a download, a
//! QuickApp project) finished while Lumina was in the background. Texts
//! follow the UI language, like the tray.

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

/// What happened.
pub enum Note<'a> {
    /// A service exited unexpectedly and is being restarted.
    Crashed {
        service: &'a str,
    },
    /// A service exited unexpectedly and will not be restarted.
    Failed {
        service: &'a str,
    },
    Installed {
        package: &'a str,
        version: &'a str,
    },
    InstallFailed {
        package: &'a str,
        version: &'a str,
    },
    ProjectReady {
        name: &'a str,
    },
    ProjectFailed {
        name: &'a str,
    },
    UpdateAvailable {
        version: &'a str,
    },
    /// A tray action failed; `message` is the error (English, from the backend).
    ActionFailed {
        message: &'a str,
    },
}

/// The name shown for a supervisor process ID (`None`: not worth a notification).
pub fn service_name(id: &str) -> Option<&'static str> {
    Some(match id {
        "php-fcgi" => "PHP",
        "nginx" => "nginx",
        "apache" => "Apache",
        "mysql" => "MySQL",
        "mariadb" => "MariaDB",
        "redis" => "Redis",
        "postgresql" => "PostgreSQL",
        // The tunnel agent's exits are shown on the Sharing page.
        _ => return None,
    })
}

/// Title and body of `note` in `language`.
pub fn text(language: &str, note: &Note<'_>) -> (String, String) {
    let si = language == "si";
    match *note {
        Note::Crashed { service } if si => (
            format!("{service} අනපේක්ෂිතව නතර විය"),
            "Lumina එය නැවත ආරම්භ කරමින් සිටී.".into(),
        ),
        Note::Crashed { service } => (
            format!("{service} stopped unexpectedly"),
            "Lumina is restarting it.".into(),
        ),
        Note::Failed { service } if si => (
            format!("{service} නතර විය"),
            "එය නැවත ආරම්භ කළ නොහැකි විය. විස්තර සඳහා Logs පිටුව බලන්න.".into(),
        ),
        Note::Failed { service } => (
            format!("{service} stopped"),
            "It could not be restarted. The Logs page shows why.".into(),
        ),
        Note::Installed { package, version } if si => (
            format!("{package} {version} ස්ථාපනය කළා"),
            "දැන් භාවිත කළ හැක.".into(),
        ),
        Note::Installed { package, version } => (
            format!("{package} {version} installed"),
            "It is ready to use.".into(),
        ),
        Note::InstallFailed { package, version } if si => (
            format!("{package} {version} ස්ථාපනය නොවීය"),
            "Quick Add පිටුවේ හේතුව දැක්වේ.".into(),
        ),
        Note::InstallFailed { package, version } => (
            format!("{package} {version} was not installed"),
            "Quick Add shows why.".into(),
        ),
        Note::ProjectReady { name } if si => {
            (format!("{name} සූදානම්"), "QuickApp ව්‍යාපෘතිය සාදන ලදී.".into())
        }
        Note::ProjectReady { name } => (
            format!("{name} is ready"),
            "The QuickApp project was created.".into(),
        ),
        Note::UpdateAvailable { version } if si => (
            format!("Lumina {version} ලබා ගත හැක"),
            "Settings → යාවත්කාලීන කිරීම් වෙතින් ස්ථාපනය කරන්න.".into(),
        ),
        Note::UpdateAvailable { version } => (
            format!("Lumina {version} is available"),
            "Install it from Settings → Updates.".into(),
        ),
        Note::ProjectFailed { name } if si => (
            format!("{name} සෑදිය නොහැකි විය"),
            "QuickApp පිටුවේ ප්‍රතිදානය බලන්න.".into(),
        ),
        Note::ActionFailed { message } if si => ("එය සාර්ථක නොවීය".into(), (*message).to_owned()),
        Note::ActionFailed { message } => ("That did not work".into(), (*message).to_owned()),
        Note::ProjectFailed { name } => (
            format!("{name} could not be created"),
            "The QuickApp page shows the output.".into(),
        ),
    }
}

/// Shows `note`. Crashes always; finished tasks only while the window is in
/// the background (in front, the page already shows the result).
pub fn send(app: &AppHandle, note: Note<'_>) {
    let background_only = !matches!(
        note,
        Note::Crashed { .. }
            | Note::Failed { .. }
            | Note::UpdateAvailable { .. }
            | Note::ActionFailed { .. }
    );
    if background_only
        && app
            .get_webview_window("main")
            .is_some_and(|w| w.is_focused().unwrap_or(false) && w.is_visible().unwrap_or(false))
    {
        return;
    }
    let (title, body) = text(&crate::tray::language(app), &note);
    if let Err(err) = app.notification().builder().title(title).body(body).show() {
        log::warn!("notification not shown: {err}");
    }
}
