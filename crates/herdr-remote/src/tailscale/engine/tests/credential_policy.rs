use super::*;

#[test]
fn an_unsigned_node_requires_credential_consent_before_starting() -> Result<()> {
    if herdr_credentials::KEYRING {
        return Ok(());
    }
    let _native = crate::tailscale::tests::native_engine_guard();
    let directory = tempfile::tempdir()?;
    let config = Config {
        control_url: "https://127.0.0.1:9".into(),
        ..Default::default()
    };
    let engine = Engine::new(&config, &directory.path().join("private"), "policy")?;
    assert!(matches!(engine.start(), Err(Error::CredentialsUnavailable)));
    assert_eq!(
        engine.snapshot()?.status.state,
        super::super::super::State::Disconnected
    );
    Ok(())
}
