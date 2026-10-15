//! The real ngrok agent, without an account: it must start, report the
//! refused token clearly, and leave no process behind. Opt-in with
//! `LUMINA_TEST_NGROK_DIR=<folder with ngrok(.exe)>`.

use lumina_process::Supervisor;
use lumina_process::ledger::Ledger;
use lumina_tunnel::ngrok::{self, Ngrok, Settings};
use lumina_tunnel::{Error, TunnelProvider, TunnelSpec};

fn program() -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("LUMINA_TEST_NGROK_DIR")?;
    Some(std::path::PathBuf::from(dir).join(ngrok::program_name()))
}

#[tokio::test]
async fn a_refused_token_is_reported_and_nothing_is_left_running() {
    let Some(program) = program() else {
        eprintln!("skipped: set LUMINA_TEST_NGROK_DIR");
        return;
    };
    let tmp = tempfile::tempdir().expect("tempdir");
    let supervisor = Supervisor::new(Ledger::disabled());
    let agent = Ngrok::new(
        supervisor.clone(),
        &tmp.path().join("etc"),
        &tmp.path().join("logs"),
    );
    let spec = TunnelSpec {
        name: "blog".into(),
        host: "blog.test".into(),
        port: 8080,
        domain: None,
    };
    let settings = Settings {
        program,
        // Well-formed for Lumina, unknown to ngrok.
        authtoken: "2abcdefBOGUS_token_value1234567890".into(),
    };

    // The UI polls the tunnel list while a share is opening.
    let done = std::sync::atomic::AtomicBool::new(false);
    let open = async {
        let result = agent.open(&settings, &spec).await;
        done.store(true, std::sync::atomic::Ordering::Relaxed);
        result
    };
    let poll = async {
        while !done.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = agent.tunnels().await;
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    };
    let (result, ()) = tokio::join!(open, poll);
    let err = result.expect_err("no account");
    assert!(matches!(err, Error::Auth(_)), "{err:?}");
    assert!(err.to_string().contains("ERR_NGROK_"), "{err}");
    assert!(
        !err.to_string().contains("BOGUS"),
        "the token is not repeated: {err}"
    );

    assert!(agent.tunnels().await.is_empty());
    assert_eq!(
        agent.stopped_reason(),
        None,
        "a failed share is not reported as the agent stopping by itself"
    );
    assert!(
        supervisor.info(ngrok::PROCESS_ID).await.is_none(),
        "the agent was cleaned up"
    );
    let config = std::fs::read_to_string(tmp.path().join("etc").join("ngrok.yml")).expect("config");
    assert!(!config.contains("BOGUS"), "no token in the config file");

    // Closing and shutting down without tunnels is harmless.
    agent.close("blog").await.expect("close");
    agent.shutdown().await.expect("shutdown");
}
