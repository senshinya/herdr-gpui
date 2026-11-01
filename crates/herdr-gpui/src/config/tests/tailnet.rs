use super::*;

#[test]
fn embedded_tailscale_config_is_typed_and_defaults_to_disconnected() -> anyhow::Result<()> {
    let config = Config::parse("[tailscale]\nhostname = 'my-herdr'\nenabled = true\n")?;
    assert_eq!(config.tailscale.hostname, "my-herdr");
    assert!(config.tailscale.enabled);
    assert!(!Config::default().tailscale.enabled);
    assert!(Config::parse("[tailscale]\nenabled = 'true'\n").is_err());
    assert!(Config::parse("[tailscale]\ncontrol_url = 'https://user:pass@example.com'\n").is_err());
    Ok(())
}

#[test]
fn tailscale_saves_preserve_unrelated_settings_and_comments() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("config.toml");
    fs::write(
        &path,
        "# my settings\nlanguage = 'en-US'\n[tailscale]\n# node\nhostname = 'old'\n",
    )?;
    let config = herdr_remote::tailscale::Config {
        enabled: true,
        hostname: "new-node".into(),
        ..Default::default()
    };
    Config::save_tailscale_path(&config, &path)?;
    let text = fs::read_to_string(&path)?;
    assert!(text.contains("# my settings") && text.contains("# node"));
    assert_eq!(Config::parse(&text)?.tailscale, config);
    Ok(())
}
