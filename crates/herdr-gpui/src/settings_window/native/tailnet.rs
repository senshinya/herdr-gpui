//! Real GPU evidence from the isolated fixture, without starting a Tailnet.
use super::*;
use herdr_remote::tailscale::{Peer, Snapshot, State};

fn snapshot() -> Snapshot {
    let mut snapshot = Snapshot {
        generation: 7,
        ..Default::default()
    };
    snapshot.status.state = State::Connected;
    // i18n: data
    snapshot.status.network = "Example network".into();
    snapshot.status.network_id = "fixture-network".into();
    // i18n: data
    snapshot.status.self_name = "Herdr workstation".into();
    snapshot.status.self_ips = vec!["100.64.1.10".into()];
    snapshot.status.peers = (0..80)
        .map(|i| Peer {
            addresses: vec![format!("100.64.0.{}", i + 1)],
            dns_name: format!("workstation-{i:02}.example.ts.net"),
            id: format!("p{i:02}"),
            // i18n: data
            name: format!("Workstation {i:02}"),
            online: i % 3 != 1,
            os: if i % 2 == 0 {
                "macOS".into()
            } else {
                "Linux".into() // i18n: data
            },
            ..Default::default()
        })
        .collect();
    snapshot.status.total = snapshot.status.peers.len();
    snapshot
}

async fn keys(window: AnyWindowHandle, keys: &str, cx: &mut AsyncApp) -> Result<()> {
    for key in keys.split_whitespace() {
        window.update(cx, |_, window, cx| -> Result<()> {
            window.dispatch_keystroke(Keystroke::parse(key)?, cx);
            window.draw(cx).clear(cx);
            Ok(())
        })??;
    }
    Ok(())
}

pub(super) async fn capture_devices(
    source: WindowHandle<HerdrWindow>,
    cx: &mut AsyncApp,
) -> Result<()> {
    if std::env::var_os("HERDR_TEST_SETTINGS_CAPTURE").is_none() {
        return Ok(());
    }
    let original_size = source.update(cx, |_, window, _| window.viewport_size())?;
    source.update(cx, |view, window, cx| {
        // Only the fixture's UI target changes; no bridge connects or starts.
        let target = std::mem::replace(
            &mut view.endpoints[0].connection.target,
            herdr_client::ConnectTarget::Local,
        );
        view.open_device_setup(window, cx);
        view.endpoints[0].connection.target = target;
        view.native_tailnet_picker(snapshot(), window, cx);
    })?;
    capture_window(source.into(), "picker.png", cx).await?;
    source.update(cx, |_, window, _| window.resize(size(px(680.), px(560.))))?;
    capture_window(source.into(), "picker-narrow.png", cx).await?;
    // i18n: data
    keys(source.into(), "down enter", cx).await?;
    capture_window(source.into(), "details.png", cx).await?;
    // i18n: data
    keys(source.into(), "shift-tab enter tab tab tab enter", cx).await?;
    capture_window(source.into(), "manual.png", cx).await?;
    source.update(cx, |view, window, cx| {
        view.dismiss_menu(window, cx);
        window.resize(original_size);
    })?;
    capture_window(source.into(), "status-bar.png", cx).await?;
    Ok(())
}

pub(super) async fn capture_settings_devices(
    settings: WindowHandle<SettingsWindow>,
    cx: &mut AsyncApp,
) -> Result<()> {
    if std::env::var_os("HERDR_TEST_SETTINGS_CAPTURE").is_none() {
        return Ok(());
    }
    for (width, height, name) in [
        (960., 780., "devices-wide.png"),
        (680., 560., "devices-narrow.png"),
    ] {
        settings.update(cx, |view, window, cx| {
            window.resize(size(px(width), px(height)));
            view.select_section(Section::Tailscale, window, cx);
            view.native_tailnet_devices(snapshot(), window, cx);
        })?;
        capture_window(settings.into(), name, cx).await?;
    }
    settings.update(cx, |view, window, cx| {
        view.select_section(Section::General, window, cx);
        view.close_tailscale();
    })?;
    Ok(())
}
