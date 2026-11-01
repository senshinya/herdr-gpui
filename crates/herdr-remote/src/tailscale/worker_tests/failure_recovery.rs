use super::*;

#[test]
fn failed_start_releases_the_node_so_an_unsaved_identity_can_change_store() -> Result<()> {
    let _native = crate::tailscale::tests::native_engine_guard();
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("private");
    let config = Config {
        enabled: true,
        allow_plaintext_credentials: true,
        control_url: "https://127.0.0.1:9".into(),
        ..Default::default()
    };
    let node = Engine::new(&config, &root, "failed-start")?;
    // Force failure before networking or credential access on every platform.
    std::fs::create_dir(root.join(node.identity()).join("tailscaled.log.conf"))?;
    drop(node);
    let manager = Manager::new()?;
    manager.connect(config.clone(), root.clone(), "failed-start".into())?;
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while manager.snapshot().status.state != State::Failed {
        if std::time::Instant::now() >= deadline {
            manager.shutdown()?;
            panic!("the isolated node did not report its forced startup failure");
        }
        thread::sleep(Duration::from_millis(1));
    }
    let retained = manager.engine().is_ok();
    let next = Config {
        allow_plaintext_credentials: false,
        ..config.clone()
    };
    let transition = manager.validate_transition(&config, &next, &root, "failed-start");
    manager.shutdown()?;
    assert!(!retained, "a failed node still owns the active connection");
    transition?;
    Ok(())
}
