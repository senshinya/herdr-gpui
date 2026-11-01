use super::*;

#[test]
fn rejects_credential_bearing_or_insecure_control_servers() {
    for control in [
        "http://example.com",
        "https://user:pass@example.com",
        "https://example.com/#secret",
        "file:///tmp/state",
    ] {
        let config = Config {
            control_url: control.into(),
            ..Config::default()
        };
        assert!(matches!(config.validate(), Err(Error::InvalidConfig)));
    }
    assert!(Config::default().validate().is_ok());
}

#[test]
fn authorization_url_is_redacted_from_debug_and_public_json()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let state: Snapshot = serde_json::from_str(
        r#"{"state":"needs_login","auth_url":"https://login.tailscale.com/a/secret-token"}"#,
    )?;
    assert!(!format!("{state:?}").contains("secret-token"));
    assert!(!serde_json::to_string(&state.status)?.contains("secret-token"));
    Ok(())
}
