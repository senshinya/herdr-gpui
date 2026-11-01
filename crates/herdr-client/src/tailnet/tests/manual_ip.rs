use super::*;

#[test]
fn an_installed_tailnet_route_does_not_capture_manually_entered_ips() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let binding = Binding {
        engine: "a".repeat(64),
        network: "manual-ip-test".into(),
        original_target: "ssh://me@100.64.0.42:22".into(),
        peer_id: "manual-ip-peer".into(),
        port: 22,
        user: "me".into(),
    };
    let target = binding.target();
    let route = Route::deferred(
        binding,
        directory.path().join("proxy"),
        directory.path().join("herdr-gpui"),
        directory.path(),
    )?;
    install(route)?;
    assert!(
        routes()
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .contains_key(&target)
    );
    for manual in [
        "100.64.0.42",
        "me@100.64.0.42",
        "ssh://me@100.64.0.42:22",
        "ssh://me@[fd7a:115c:a1e0::42]:22",
    ] {
        assert!(
            options(manual).is_empty(),
            "manual SSH acquired an embedded proxy: {manual}"
        );
    }
    routes()
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&target);
    Ok(())
}
