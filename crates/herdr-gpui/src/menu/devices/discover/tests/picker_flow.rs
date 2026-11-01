use super::*;
use gpui::{Modifiers, TestAppContext};

#[gpui::test]
fn choosing_a_device_replaces_the_list_with_the_connection_form(cx: &mut TestAppContext) {
    let (_view, cx) = form::open(cx, form::report);
    form::settle(cx);
    let row = cx.debug_bounds("device-suggestion-0").unwrap().center();
    cx.simulate_click(row, Modifiers::default());
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    assert!(
        cx.debug_bounds("device-suggestions").is_none(),
        "device list remains nested in the form"
    );
    assert!(cx.debug_bounds("device-setup-submit").is_some());
}

#[gpui::test]
fn opening_add_device_starts_with_selection_and_independent_manual_entry(cx: &mut TestAppContext) {
    let (_view, cx) = form::open(cx, form::report);
    form::settle(cx);
    assert!(
        cx.debug_bounds("device-setup-submit").is_none(),
        "connection form is already displayed"
    );
    let tabs = cx.debug_bounds("device-picker-tabs").unwrap();
    let manual = cx.debug_bounds("device-picker-manual").unwrap();
    assert!(
        !tabs.contains(&manual.center()),
        "manual entry is inside the source tabs"
    );
    cx.simulate_click(manual.center(), Modifiers::default());
    cx.run_until_parked();
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    assert!(cx.debug_bounds("device-setup-submit").is_some());
    assert!(cx.debug_bounds("device-suggestions").is_none());
}

fn connected_peer() -> herdr_remote::tailscale::Snapshot {
    use herdr_remote::tailscale::{Peer, Snapshot, State};
    let mut snapshot = Snapshot {
        generation: 8,
        ..Default::default()
    };
    snapshot.status.state = State::Connected;
    snapshot.status.network_id = "fixture-network".into();
    snapshot.status.peers.push(Peer {
        id: "fixture-peer".into(),
        name: "Test workstation".into(),
        addresses: vec!["100.64.0.42".into()],
        online: true,
        ..Default::default()
    });
    snapshot
}

#[gpui::test]
fn keyboard_selection_and_manual_entry_keep_routes_separate(cx: &mut TestAppContext) {
    let (view, cx) = form::open(cx, form::report);
    view.update(cx, |view, _| {
        view.menu
            .device_setup
            .as_mut()
            .unwrap()
            .picker
            .update_snapshot(connected_peer())
    });
    form::settle(cx);
    let point = cx.debug_bounds("device-picker-tailnet").unwrap().center();
    cx.simulate_click(point, Default::default());
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        let setup = view.menu.device_setup.as_ref().unwrap();
        assert_eq!(
            setup.tailnet_peer.as_ref().map(|s| s.peer.id.as_str()),
            Some("fixture-peer")
        );
        assert!(!setup.picker.choosing());
    });
    view.update_in(cx, |view, window, cx| {
        let setup = view.menu.device_setup.as_mut().unwrap();
        setup.fields[1].update(cx, |input, cx| input.set_text_selected("My label", cx));
        setup.fields[2].update(cx, |input, cx| input.set_text_selected("work", cx));
        view.show_device_picker(window, cx);
    });
    cx.run_until_parked();
    form::settle(cx);
    let point = cx.debug_bounds("device-picker-manual").unwrap().center();
    cx.simulate_click(point, Default::default());
    cx.run_until_parked();
    view.read_with(cx, |view, cx| {
        let setup = view.menu.device_setup.as_ref().unwrap();
        assert!(setup.tailnet_peer.is_none());
        assert!(setup.fields[0].read(cx).text().is_empty());
        assert_eq!(setup.fields[1].read(cx).text(), "My label");
        assert_eq!(setup.fields[2].read(cx).text(), "work");
    });
}

#[gpui::test]
fn peer_poll_compares_against_the_snapshot_installed_in_the_picker(cx: &mut TestAppContext) {
    let (view, cx) = form::open(cx, form::report);
    view.update(cx, |view, cx| {
        view.menu
            .device_setup
            .as_mut()
            .unwrap()
            .picker
            .update_snapshot(connected_peer());
        view.start_tailnet_devices(cx);
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.menu
                .device_setup
                .as_ref()
                .unwrap()
                .picker
                .snapshot
                .status
                .state,
            herdr_remote::tailscale::State::Disconnected,
            "first snapshot change was lost"
        );
    });
}
