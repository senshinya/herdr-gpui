use super::*;

#[test]
fn unopened_native_engine_can_be_snapshotted_and_destroyed()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let _native = native_engine_guard();
    let directory = tempfile::tempdir()?;
    let engine = Engine::new(
        &Config::default(),
        &directory.path().join("private"),
        "test-lifecycle",
    )?;
    assert_eq!(engine.snapshot()?.status.state, State::Disconnected);
    drop(engine);
    let engine = Engine::new(
        &Config::default(),
        &directory.path().join("private"),
        "test-lifecycle",
    )?;
    assert_eq!(engine.snapshot()?.status.state, State::Disconnected);
    Ok(())
}

#[test]
fn same_identity_is_exclusive_while_engine_is_alive()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let _native = native_engine_guard();
    let directory = tempfile::tempdir()?;
    let first = Engine::new(
        &Config::default(),
        &directory.path().join("private"),
        "test-exclusive",
    )?;
    assert!(matches!(
        Engine::new(
            &Config::default(),
            &directory.path().join("private"),
            "test-exclusive"
        ),
        Err(Error::Busy)
    ));
    drop(first);
    drop(Engine::new(
        &Config::default(),
        &directory.path().join("private"),
        "test-exclusive",
    )?);
    Ok(())
}

#[test]
fn a_disconnected_owner_cannot_clear_a_new_owners_identity()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let _native = native_engine_guard();
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("private");
    let first = Engine::new(&Config::default(), &root, "ownership-transfer")?;
    first.disconnect()?;
    let second = Engine::new(&Config::default(), &root, "ownership-transfer")?;
    assert!(matches!(first.logout(), Err(Error::Closed)));
    assert!(matches!(
        Engine::new(&Config::default(), &root, "ownership-transfer"),
        Err(Error::Busy)
    ));
    drop(second);
    Ok(())
}
