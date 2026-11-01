//! The real update panel, captured inside the isolated native fixture.
use super::*;

pub(super) async fn capture(source: WindowHandle<HerdrWindow>, cx: &mut AsyncApp) -> Result<()> {
    if std::env::var_os("HERDR_TEST_SETTINGS_CAPTURE").is_none() {
        return Ok(());
    }
    let original_size = source.update(cx, |_, window, _| window.viewport_size())?;
    for (width, height, name) in [
        (800., 600., "update-wide.png"),
        (440., 560., "update-narrow.png"),
    ] {
        source.update(cx, |view, window, cx| {
            window.resize(size(px(width), px(height)));
            view.open_update_progress_preview(
                crate::updater::State::Manual {
                    version: "20261028.1-shinya.2".into(),
                    // i18n: data
                    notes: "## Changed\n\n- Fork release notes are shown before opening the download page.".into(),
                },
                window,
                cx,
            );
        })?;
        capture_window(source.into(), name, cx).await?;
        source.update(cx, |view, window, cx| view.dismiss_menu(window, cx))?;
    }
    source.update(cx, |_, window, _| window.resize(original_size))?;
    Ok(())
}
