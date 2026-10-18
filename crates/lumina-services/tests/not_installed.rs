//! Starting something that is not installed says so, and the service is not
//! marked as failed. No downloads needed: the data root is empty.

use std::path::Path;

use lumina_config::AppConfig;
use lumina_core::Error;
use lumina_core::paths::Layout;
use lumina_core::types::{ServiceKind, ServiceStatus};
use lumina_process::Supervisor;
use lumina_process::ledger::Ledger;
use lumina_services::orchestrator::Orchestrator;
use lumina_services::template::Templates;

#[tokio::test]
async fn a_missing_program_is_reported_as_not_installed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let layout = Layout::new(tmp.path().join("Lumina"));
    layout.ensure().expect("layout");
    let templates = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/templates");
    let stack = Orchestrator::new(
        Supervisor::new(Ledger::disabled()),
        layout,
        Templates::new(templates),
    );
    let cfg = AppConfig::default();

    let err = stack
        .start(ServiceKind::Mysql, &cfg)
        .await
        .expect_err("nothing installed");
    match err {
        Error::NotInstalled { package, name, .. } => {
            assert_eq!(package, "mysql");
            assert_eq!(name, "MySQL");
        }
        other => panic!("expected NotInstalled, got {other}"),
    }
    let err = stack
        .start(ServiceKind::PhpFcgi, &cfg)
        .await
        .expect_err("no PHP");
    assert!(
        matches!(&err, Error::NotInstalled { package, .. } if package == "php"),
        "{err}"
    );

    let status = stack.status(&cfg).await;
    assert_eq!(status.database.status, ServiceStatus::Stopped);
    assert_eq!(status.database.error, None);
    assert_eq!(status.php.status, ServiceStatus::Stopped);
}
