use super::*;

mod manual_ip;

#[test]
fn missing_embedded_route_cannot_use_a_direct_connection() {
    let target = "ssh://me@ht-a.b.herdr-tailnet.invalid:22";
    let args = options(target);
    assert!(args.iter().any(|arg| arg == "ProxyCommand=false"));
    assert!(options("me@ordinary-host").is_empty());
}

#[test]
fn conflicting_user_proxy_is_rejected_before_route_installation() {
    assert!(matches!(
        validate_ssh_config("hostname box\nproxycommand custom-proxy\n"),
        Err(Error::Conflict)
    ));
    assert!(matches!(
        validate_ssh_config("hostname box\nhostkeyalias shared-alias\n"),
        Err(Error::Conflict)
    ));
    assert!(validate_ssh_config("hostname box\nproxycommand none\nproxyjump none\n").is_ok());
}

#[test]
fn same_named_nodes_in_distinct_networks_have_distinct_trust_keys() {
    let a = Binding {
        engine: "a".repeat(64),
        network: "one.ts.net".into(),
        peer_id: "peer-one".into(),
        original_target: "me@box".into(),
        user: "me".into(),
        port: 22,
    };
    let mut b = a.clone();
    b.network = "two.ts.net".into();
    assert_ne!(a.target(), b.target());
    assert_ne!(a.trust_key(), b.trust_key());
}

#[test]
fn network_names_are_opaque_and_proxy_arguments_cannot_be_shell_syntax() -> Result<()> {
    let file = tempfile::NamedTempFile::new()?;
    let binding = Binding {
        engine: "a".repeat(64),
        network: "someone@example.com".into(),
        original_target: "me@box".into(),
        peer_id: "node-1".into(),
        port: 22,
        user: "me".into(),
    };
    binding.validate()?;
    let target = binding.target();
    install(Route {
        binding,
        endpoint: "/private/proxy".into(),
        executable: "/private/herdr-gpui".into(),
        configuration: Arc::new(Mutex::new(Some(Arc::new(file.into_temp_path())))),
        directory: std::env::temp_dir(),
        generation: || 0,
    })?;
    let command = options(&target)
        .into_iter()
        .find(|s| s.to_string_lossy().starts_with("ProxyCommand="))
        .ok_or(Error::Invalid)?;
    assert!(command.to_string_lossy().contains("--network-hex "));
    assert!(!command.to_string_lossy().contains("someone@example.com"));
    routes()
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&target);
    Ok(())
}

#[test]
fn saved_routes_can_be_registered_before_resolving_ssh_configuration() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let binding = Binding {
        engine: "b".repeat(64),
        network: "tail.example".into(),
        original_target: "me@unreachable-test-alias".into(),
        peer_id: "node".into(),
        port: 22,
        user: "me".into(),
    };
    let route = Route::deferred(
        binding,
        directory.path().join("p"),
        std::env::current_exe()?,
        directory.path(),
    )?;
    assert!(
        route
            .configuration
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_none()
    );
    Ok(())
}

#[test]
fn ssh_configuration_round_trips_quoted_identity_paths() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let key = directory.path().join("key with spaces");
    let source = directory.path().join("source");
    std::fs::write(
        &source,
        format!(
            "Host fixture\n IdentityFile \"{}\"\n IdentityAgent \"{}\"\n CertificateFile \"{}\"\n PKCS11Provider \"{}\"\n UserKnownHostsFile none\n GlobalKnownHostsFile none\n",
            key.display(),
            key.display(),
            key.display(),
            key.display()
        ),
    )?;
    let first = crate::new_command("ssh")
        .args([
            OsStr::new("-G"),
            OsStr::new("-F"),
            source.as_os_str(),
            OsStr::new("fixture"),
        ])
        .output()?;
    assert!(first.status.success());
    let mut rendered = Vec::new();
    render_configuration(
        &mut rendered,
        std::str::from_utf8(&first.stdout).map_err(|_| Error::Resolve)?,
        "fixture",
    )?;
    let replay = directory.path().join("replay");
    std::fs::write(&replay, rendered)?;
    let second = crate::new_command("ssh")
        .args([
            OsStr::new("-G"),
            OsStr::new("-F"),
            replay.as_os_str(),
            OsStr::new("synthetic"),
        ])
        .output()?;
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let text = String::from_utf8_lossy(&second.stdout);
    assert!(
        text.lines()
            .any(|line| line == format!("identityfile {}", key.display()))
    );
    Ok(())
}
