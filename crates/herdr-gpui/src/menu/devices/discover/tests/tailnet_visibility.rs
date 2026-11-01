use super::*;
use crate::i18n::{leak_tests::under_en_xa, message};

#[test]
fn a_disconnected_tailnet_is_absent_from_the_add_device_form() {
    under_en_xa("disconnected Tailnet visibility", |cx, recorder| {
        let (_view, cx) = form::open(cx, form::report);
        form::settle(cx);
        let shown = recorder.take();
        assert!(shown.contains(&message("devices-discover-heading", &[])));
        assert!(!shown.contains(&message("tailnet-devices", &[])));
    });
}

#[gpui::test]
fn tailnet_devices_follow_connection_state_even_with_a_cached_peer(cx: &mut gpui::TestAppContext) {
    use herdr_remote::tailscale::{Peer, Snapshot, State};
    let (view, cx) = form::open(cx, form::report);
    view.update_in(cx, |view, _, cx| {
        let setup = view.menu.device_setup.as_ref().unwrap();
        let mut snapshot = Snapshot::default();
        snapshot.status.peers.push(Peer {
            id: "fixture-peer".into(),
            name: "Example device".into(),
            ..Peer::default()
        });
        for (state, shown) in [
            (State::Connected, true),
            (State::Disconnected, false),
            (State::Connecting, false),
            (State::NeedsLogin, false),
            (State::NeedsApproval, false),
            (State::Failed, false),
            (State::Disconnecting, false),
            (State::Connected, true),
        ] {
            snapshot.status.state = state;
            assert_eq!(
                view.render_tailnet_devices(setup, snapshot.clone(), cx)
                    .is_some(),
                shown,
                "{state:?}",
            );
        }
    });
}
