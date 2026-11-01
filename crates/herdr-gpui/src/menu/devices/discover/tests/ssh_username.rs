use super::*;
use crate::sidebar::layout_tests::full_draw;
use gpui::TestAppContext;
use herdr_remote::tailscale::{Peer, Snapshot, State};

#[gpui::test]
fn nearby_targets_explain_the_implicit_login_until_a_user_is_named(cx: &mut TestAppContext) {
    let (view, cx) = form::open(cx, form::report);
    for (target, named) in [
        ("192.168.1.20", "alice@192.168.1.20"),
        ("ssh://nas.local:2222", "ssh://alice@nas.local:2222"),
        (
            "ssh://[fd7a:115c:a1e0::44]",
            "ssh://alice@[fd7a:115c:a1e0::44]",
        ),
    ] {
        view.update_in(cx, |view, window, cx| {
            view.use_device_suggestion(target, "Nearby workstation", window, cx);
        });
        cx.update(|window, cx| full_draw(window, cx).clear(cx));
        assert!(
            cx.debug_bounds("device-setup-username-hint").is_some(),
            "{target}"
        );
        assert_eq!(form::fields(&view, cx).0, target);
        let input = view.read_with(cx, |view, _| {
            view.menu.device_setup.as_ref().unwrap().fields[0].clone()
        });
        input.update(cx, |input, cx| input.set_text_selected(named, cx));
        cx.run_until_parked();
        cx.update(|window, cx| full_draw(window, cx).clear(cx));
        assert!(
            cx.debug_bounds("device-setup-username-hint").is_none(),
            "{named}"
        );
        input.update(cx, |input, cx| input.set_text_selected(target, cx));
        cx.run_until_parked();
        cx.update(|window, cx| full_draw(window, cx).clear(cx));
        assert!(cx.debug_bounds("device-setup-username-hint").is_some());
    }
}

#[gpui::test]
fn selecting_a_tailnet_peer_explains_the_missing_ssh_login(cx: &mut TestAppContext) {
    let (view, cx) = form::open(cx, form::report);
    let mut snapshot = Snapshot {
        generation: 8,
        ..Default::default()
    };
    snapshot.status.state = State::Connected;
    snapshot.status.network_id = "fixture-network".into();
    snapshot.status.peers.push(Peer {
        id: "username-peer".into(),
        name: "Tailnet workstation".into(),
        addresses: vec!["100.64.0.42".into()],
        online: true,
        ..Default::default()
    });
    view.update(cx, |view, _| {
        let setup = view.menu.device_setup.as_mut().unwrap();
        setup.tailnet_poll = None;
        setup.picker.update_snapshot(snapshot);
    });
    form::settle(cx);
    let tab = cx.debug_bounds("device-picker-tailnet").unwrap().center();
    cx.simulate_click(tab, Default::default());
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
    assert_eq!(form::fields(&view, cx).0, "100.64.0.42");
    assert!(cx.debug_bounds("device-setup-username-hint").is_some());
    view.read_with(cx, |view, _| {
        let setup = view.menu.device_setup.as_ref().unwrap();
        assert_eq!(
            setup.tailnet_peer.as_ref().unwrap().peer.id,
            "username-peer"
        );
        assert!(matches!(
            setup.step,
            super::super::super::add_device::Step::Form
        ));
    });
}
