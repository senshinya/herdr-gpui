use super::*;
#[gpui::test]
fn embedded_tailscale_page_renders_disconnected_without_system_access(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    cx.simulate_resize(size(px(680.), px(560.)));
    view.update_in(cx, |view, window, cx| {
        view.select_section(Section::Tailscale, window, cx)
    });
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    assert!(cx.debug_bounds("tailnet-status").is_none());
    assert!(cx.debug_bounds("tailnet-network").is_none());
    assert!(cx.debug_bounds("tailnet-node").is_none());
    assert!(cx.debug_bounds("tailnet-connect").is_some());
    assert!(cx.debug_bounds("tailnet-hostname").is_some());
    assert_eq!(
        crate::tailnet::snapshot().status.state,
        herdr_remote::tailscale::State::Disconnected
    );
}
