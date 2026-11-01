use super::*;
use herdr_remote::tailscale::{Peer, Snapshot, State};

fn connected() -> Snapshot {
    let mut snapshot = Snapshot::default();
    snapshot.status.state = State::Connected;
    snapshot.status.network = "example.tailnet".into();
    snapshot.status.peers = (0..80)
        .map(|i| Peer {
            id: format!("p{i:03}"),
            name: format!("Device {i:03}"),
            addresses: vec![format!("100.64.0.{}", i + 1)],
            online: i % 2 == 0,
            ..Default::default()
        })
        .collect();
    snapshot
}

#[gpui::test]
fn connected_devices_have_a_separate_settings_view(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    cx.simulate_resize(size(px(680.), px(560.)));
    view.update_in(cx, |view, window, cx| {
        view.select_section(Section::Tailscale, window, cx);
        view.poll_tailnet(connected(), cx);
    });
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    let devices = cx.debug_bounds("tailnet-tab-devices").unwrap();
    cx.simulate_click(devices.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    assert!(cx.debug_bounds("tailnet-settings-peer-list").is_some());
    assert!(
        cx.debug_bounds("tailnet-hostname").is_none(),
        "overview form remains beside the list"
    );
    view.update(cx, |view, cx| view.poll_tailnet(Snapshot::default(), cx));
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    assert!(cx.debug_bounds("tailnet-settings-peer-list").is_none());
    assert!(cx.debug_bounds("tailnet-hostname").is_some());
}

#[gpui::test]
fn devices_search_filter_and_scroll_use_one_viewport(cx: &mut TestAppContext) {
    use gpui::{ScrollDelta, ScrollWheelEvent};
    let (view, cx) = cx.add_window_view(skill_fixture);
    cx.simulate_resize(size(px(680.), px(560.)));
    view.update_in(cx, |view, window, cx| {
        view.select_section(Section::Tailscale, window, cx);
        view.poll_tailnet(connected(), cx);
    });
    let draw = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx))
    };
    draw(cx);
    let click = cx.debug_bounds("tailnet-tab-devices").unwrap().center();
    cx.simulate_click(click, Default::default());
    draw(cx);
    assert!(
        cx.debug_bounds("settings-body").is_none(),
        "list is nested in another scrollport"
    );
    let header = cx.debug_bounds("tailnet-peer-search").unwrap();
    let list = cx.debug_bounds("tailnet-settings-peer-list").unwrap();
    cx.simulate_event(ScrollWheelEvent {
        position: list.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-100000.))),
        ..Default::default()
    });
    draw(cx);
    assert_eq!(cx.debug_bounds("tailnet-peer-search"), Some(header));
    assert!(
        cx.debug_bounds("tailnet-settings-peer-p079").is_some(),
        "last device is unreachable"
    );
    assert!(
        cx.debug_bounds("tailnet-settings-peer-p000").is_none(),
        "list did not virtualize offscreen rows"
    );
    cx.simulate_keystrokes("0 7 9");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("tailnet-settings-peer-p079").is_some());
    let click = cx.debug_bounds("tailnet-online-filter").unwrap().center();
    cx.simulate_click(click, Default::default());
    draw(cx);
    assert!(
        cx.debug_bounds("tailnet-settings-peer-p079").is_none(),
        "offline row survived Online only"
    );
    cx.simulate_click(header.center(), Default::default());
    cx.simulate_keystrokes("tab space");
    draw(cx);
    assert!(
        cx.debug_bounds("tailnet-settings-peer-p079").is_some(),
        "keyboard cannot toggle Online only"
    );
}

#[gpui::test]
fn keyboard_can_enter_the_tabs_and_round_trip_through_overview(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    cx.simulate_resize(size(px(680.), px(560.)));
    view.update_in(cx, |view, window, cx| {
        view.select_section(Section::Tailscale, window, cx);
        view.poll_tailnet(connected(), cx);
    });
    let draw = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx))
    };
    draw(cx);
    cx.simulate_keystrokes("tab tab enter");
    draw(cx);
    assert!(
        cx.debug_bounds("tailnet-settings-peer-list").is_some(),
        "root focus cannot reach Devices"
    );
    cx.simulate_keystrokes("shift-tab shift-tab enter");
    draw(cx);
    assert!(
        cx.debug_bounds("tailnet-hostname").is_some(),
        "Overview did not open"
    );
    cx.simulate_keystrokes("tab enter");
    draw(cx);
    assert!(
        cx.debug_bounds("tailnet-settings-peer-list").is_some(),
        "Overview trapped keyboard focus"
    );
}
