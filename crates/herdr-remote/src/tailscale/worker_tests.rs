#![allow(clippy::unwrap_used)]
use super::*;

#[path = "worker_tests/failure_recovery.rs"]
mod failure_recovery;

#[test]
fn full_command_queue_does_not_invalidate_the_accepted_generation() {
    let (commands, _incoming) = mpsc::sync_channel(1);
    let manager = Manager {
        commands,
        shared: Arc::new(Mutex::new(Shared::default())),
        generation: Arc::new(AtomicU64::new(0)),
    };
    manager.disconnect(false).unwrap();
    assert!(matches!(manager.disconnect(false), Err(Error::Busy)));
    assert_eq!(manager.generation.load(Ordering::Acquire), 1);
}

#[test]
fn disconnect_immediately_removes_stale_authorization_and_peers() {
    let (commands, _incoming) = mpsc::sync_channel(1);
    let snapshot: Snapshot = serde_json::from_str(
        r#"{"state":"needs_login","auth_url":"https://login.tailscale.com/a/private"}"#,
    )
    .unwrap();
    let manager = Manager {
        commands,
        shared: Arc::new(Mutex::new(Shared {
            snapshot,
            engine: None,
            pending_logouts: 0,
        })),
        generation: Arc::new(AtomicU64::new(0)),
    };
    manager.disconnect(false).unwrap();
    assert!(manager.snapshot().auth_url.is_none());
    assert_eq!(manager.snapshot().status.state, State::Disconnecting);
}

#[cfg(unix)]
#[test]
fn superseded_logout_still_deletes_identity_before_later_connection() -> Result<()> {
    let _native = crate::tailscale::tests::native_engine_guard();
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("private");
    let config = Config {
        allow_plaintext_credentials: true,
        ..Default::default()
    };
    let engine = Engine::new(&config, &root, "logout-order")?;
    let store = super::super::credentials::Storage::new(
        &config,
        root.join(engine.identity()),
        engine.identity().into(),
    );
    store.write(b"key", Some(b"private identity"))?;
    drop(engine);
    let (sender, receiver) = mpsc::sync_channel(4);
    sender
        .send(Command::Connect {
            config: config.clone(),
            root: root.clone(),
            namespace: "logout-order".into(),
            generation: 1,
        })
        .map_err(|_| Error::Worker)?;
    sender
        .send(Command::Disconnect {
            done: None,
            logout: true,
            generation: 2,
        })
        .map_err(|_| Error::Worker)?;
    sender
        .send(Command::Connect {
            config,
            root,
            namespace: "logout-order".into(),
            generation: 3,
        })
        .map_err(|_| Error::Worker)?;
    drop(sender);
    run(
        receiver,
        Arc::new(Mutex::new(Shared {
            pending_logouts: 1,
            ..Shared::default()
        })),
        Arc::new(AtomicU64::new(3)),
    );
    assert!(store.read(b"key")?.is_none());
    Ok(())
}

#[test]
fn queued_reconnect_keeps_logout_pending_until_the_worker_finishes() {
    let (commands, _incoming) = mpsc::sync_channel(4);
    let manager = Manager {
        commands,
        shared: Arc::new(Mutex::new(Shared::default())),
        generation: Arc::new(AtomicU64::new(0)),
    };
    manager.disconnect(true).unwrap();
    manager
        .connect(Config::default(), PathBuf::from("unused"), "test".into())
        .unwrap();
    assert_eq!(manager.snapshot().status.state, State::Disconnecting);
}

#[cfg(unix)]
#[test]
fn changing_the_control_server_requires_clearing_the_previous_identity() -> Result<()> {
    let _native = crate::tailscale::tests::native_engine_guard();
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("private");
    let config = Config {
        allow_plaintext_credentials: true,
        ..Default::default()
    };
    let engine = Engine::new(&config, &root, "transition")?;
    let store = super::super::credentials::Storage::new(
        &config,
        root.join(engine.identity()),
        engine.identity().into(),
    );
    store.write(b"key", Some(b"private identity"))?;
    drop(engine);
    let previous = (config.clone(), root.clone(), "transition".into());
    let changed = (
        Config {
            control_url: "https://headscale.example".into(),
            ..config
        },
        root,
        "transition".into(),
    );
    assert!(matches!(
        identity_transition(Some(&previous), &changed, None),
        Err(Error::IdentityChange)
    ));
    store.clear()?;
    identity_transition(Some(&previous), &changed, None)?;
    Ok(())
}

#[test]
fn a_second_process_cannot_acknowledge_disconnect_for_the_identity_owner() -> Result<()> {
    let _native = crate::tailscale::tests::native_engine_guard();
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("private");
    let config = Config::default();
    let owner = Engine::new(&config, &root, "owner")?;
    let manager = Manager::new()?;
    manager.connect(config.clone(), root.clone(), "owner".into())?;
    assert!(matches!(manager.disconnect_wait(false), Err(Error::Busy)));
    assert!(matches!(
        Engine::new(&config, &root, "owner"),
        Err(Error::Busy)
    ));
    manager.shutdown()?;
    drop(owner);
    Ok(())
}
