use super::*;

#[gpui::test]
fn normal_settings_keep_explanations_out_of_the_control_layout(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    view.update_in(cx, |view, window, cx| {
        view.select_section(Section::General, window, cx)
    });
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    assert!(cx.debug_bounds("general-language-description").is_none());
    assert!(cx.debug_bounds("general-language").is_some());
}

#[gpui::test]
fn quiet_footer_still_reports_pending_changes_and_failures(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    view.update(cx, |view, _| {
        assert!(view.footer_text().is_empty());
        view.layout_intent = Some(crate::config::LayoutMode::Minimal);
        assert!(view.footer_text().contains("Unsaved:"));
        view.error = Some("Could not save preferences".into());
        assert_eq!(view.footer_text(), "Could not save preferences");
    });
}

#[gpui::test]
fn advanced_tailnet_fields_are_revealed_on_request(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    cx.simulate_resize(size(px(680.), px(560.)));
    view.update_in(cx, |view, window, cx| {
        view.select_section(Section::Tailscale, window, cx)
    });
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    assert!(cx.debug_bounds("tailnet-control").is_none());
    let toggle = cx.debug_bounds("tailnet-advanced").unwrap().center();
    cx.simulate_click(toggle, Default::default());
    cx.run_until_parked();
    cx.executor().advance_clock(crate::motion::ENTER * 2);
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    assert!(cx.debug_bounds("tailnet-control").is_some());
    assert!(cx.debug_bounds("tailnet-plaintext").is_some());
    cx.simulate_keystrokes("space");
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    assert!(cx.debug_bounds("tailnet-control").is_none());
}

#[gpui::test]
fn search_opens_advanced_settings_at_the_matching_field(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    cx.simulate_resize(size(px(960.), px(780.)));
    let index = catalog::CATALOG
        .iter()
        .position(|entry| entry.id == "tailnet-control")
        .unwrap();
    view.update_in(cx, |view, window, cx| view.open_result(index, window, cx));
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    assert!(cx.debug_bounds("tailnet-control").is_some());
}

#[gpui::test]
fn search_scrolls_to_a_field_below_connected_network_details(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    cx.simulate_resize(size(px(680.), px(560.)));
    let index = catalog::CATALOG
        .iter()
        .position(|entry| entry.id == "tailnet-control")
        .unwrap();
    view.update_in(cx, |view, window, cx| {
        view.config.ui.size = 24.;
        view.open_result(index, window, cx);
        let mut snapshot = herdr_remote::tailscale::Snapshot::default();
        snapshot.status.state = herdr_remote::tailscale::State::Connected;
        snapshot.status.network = "Long example network name ".repeat(8);
        view.poll_tailnet(snapshot, cx);
    });
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    view.update(cx, |view, cx| view.scroll_to_row("tailnet-control", cx));
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    view.read_with(cx, |view, _| assert!(view.body_scroll.offset().y < px(0.)));
    let field = cx.debug_bounds("tailnet-control").unwrap();
    let body = cx.debug_bounds("settings-body").unwrap();
    assert!(
        field.top() >= body.top() && field.bottom() <= body.bottom(),
        "{field:?} in {body:?}"
    );
}

#[gpui::test]
fn saved_custom_connection_options_stay_visible_before_connecting(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    view.update_in(cx, |view, window, cx| {
        view.config.tailscale.control_url = "https://control.example.test".into();
        view.select_section(Section::Tailscale, window, cx);
    });
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    assert!(cx.debug_bounds("tailnet-control").is_some());
}
