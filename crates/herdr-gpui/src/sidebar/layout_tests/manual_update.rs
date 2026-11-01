use super::*;

#[gpui::test]
fn a_manual_release_keeps_its_browser_action_visible_at_narrow_widths(
    cx: &mut gpui::TestAppContext,
) {
    let (fixture, cx) = cx.add_window_view(|window, cx| {
        crate::bind_keys(cx);
        let view = cx.new(|cx| fixture_window(window, cx));
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        SidebarFixture(view)
    });
    let view = cx.update(|_, cx| fixture.read(cx).0.clone());
    let state = crate::updater::State::Manual {
        version: "20261028.65535-shinya.65535".into(),
        notes: "- A fork release with release notes.\n".repeat(40),
    };
    for (width, height) in [(320., 480.), (800., 600.)] {
        cx.simulate_resize(size(px(width), px(height)));
        let (panel, action) = draw_update_state(cx, &view, &state);
        let action = action.expect("a manual release has a browser action");
        assert!(action.bottom() <= panel.bottom());
        assert!(action.left() >= panel.left() && action.right() <= panel.right());
        assert!(panel.left() >= px(0.) && panel.right() <= px(width));
        assert!(panel.top() >= px(0.) && panel.bottom() <= px(height));
        assert!(cx.debug_bounds("app-update-progress").is_none());
    }
}
