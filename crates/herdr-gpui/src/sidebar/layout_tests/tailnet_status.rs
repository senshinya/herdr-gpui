use super::*;

#[gpui::test]
fn ui_audit_tailnet_connection_is_visible_independently_of_the_daemon(
    cx: &mut gpui::TestAppContext,
) {
    let (_view, cx) = cx.add_window_view(|window, cx| {
        let mut view = fixture_window(window, cx);
        view.live.status = crate::state::ConnectionStatus::Connected;
        view.config.tailscale.enabled = true;
        view
    });
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
    assert!(cx.debug_bounds("status-tailnet").is_some());
    assert!(cx.debug_bounds("connection-message").is_none());
}
