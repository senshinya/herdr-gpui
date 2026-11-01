use super::*;

#[test]
fn identity_round_trips_binary_records_and_logout_removes_every_key()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().join("private");
    super::super::engine::private_directory(&root)?;
    let config = Config {
        allow_plaintext_credentials: true,
        ..Config::default()
    };
    let store =
        super::super::credentials::Storage::new(&config, root.clone(), "credential-test".into());
    store.write(b"_machinekey", Some(&[0, 255, 1, 0]))?;
    store.write(b"profile-one", Some(b"node-secret"))?;
    assert_eq!(
        store.read(b"_machinekey")?.as_deref().map(|v| v.as_slice()),
        Some([0, 255, 1, 0].as_slice())
    );
    store.clear()?;
    assert!(store.read(b"_machinekey")?.is_none());
    assert!(store.read(b"profile-one")?.is_none());
    Ok(())
}
