use super::*;

#[cfg(target_os = "macos")]
mod credential_policy;

fn release_with_inherited_description(release: impl FnOnce(Engine) -> Result<()>) -> Result<()> {
    let _native = crate::tailscale::tests::native_engine_guard();
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("private");
    let first = Engine::new(&Config::default(), &root, "inherited-description")?;
    // A forked child can retain the same open file description until exec,
    // even when CLOEXEC is set. A duplicate deterministically models that window.
    let inherited = first
        .0
        .lock
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .ok_or(Error::Closed)?
        .0
        .try_clone()?;
    release(first)?;
    let second = Engine::new(&Config::default(), &root, "inherited-description")?;
    drop(inherited);
    assert!(matches!(
        Engine::new(&Config::default(), &root, "inherited-description"),
        Err(Error::Busy)
    ));
    drop(second);
    Ok(())
}

#[test]
fn disconnect_releases_identity_with_an_inherited_file_description() -> Result<()> {
    release_with_inherited_description(|engine| engine.disconnect())
}

#[test]
fn drop_releases_identity_with_an_inherited_file_description() -> Result<()> {
    release_with_inherited_description(|engine| {
        drop(engine);
        Ok(())
    })
}
