use super::*;
use herdr_remote::tailscale::{Peer, Snapshot, State};

#[gpui::test]
fn peer_statuses_share_one_column_despite_different_device_details(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    let mut snapshot = Snapshot::default();
    snapshot.status.state = State::Connected;
    snapshot.status.peers = vec![
        Peer {
            id: "online".into(),
            name: "Short".into(),
            os: "macOS".into(),
            addresses: vec!["100.64.0.1".into()],
            online: true,
            ..Default::default()
        },
        Peer {
            id: "offline".into(),
            name: "A much longer workstation name".into(),
            os: "Linux".into(),
            addresses: vec!["fd7a:115c:a1e0::1234".into()],
            ..Default::default()
        },
        Peer {
            id: "expired".into(),
            name: "Expired workstation".into(),
            os: "Windows".into(),
            addresses: vec!["100.64.12.123".into()],
            expired: true,
            ..Default::default()
        },
    ];
    view.update_in(cx, |view, window, cx| {
        view.select_section(Section::Tailscale, window, cx);
        view.poll_tailnet(snapshot, cx);
    });
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    let tab = cx.debug_bounds("tailnet-tab-devices").unwrap().center();
    cx.simulate_click(tab, Default::default());
    for width in [680., 960.] {
        for font_size in [12., 24.] {
            cx.simulate_resize(size(px(width), px(780.)));
            view.update(cx, |view, cx| {
                view.config.ui.size = font_size;
                cx.notify();
            });
            cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
            let online = cx
                .debug_bounds("tailnet-settings-peer-status-online")
                .unwrap();
            for id in [
                "tailnet-settings-peer-status-offline",
                "tailnet-settings-peer-status-expired",
            ] {
                let status = cx.debug_bounds(id).unwrap();
                assert_eq!(
                    status.left(),
                    online.left(),
                    "{width}px, {font_size}px: {id}"
                );
                assert_eq!(status.size.width, online.size.width);
            }
        }
    }
}
