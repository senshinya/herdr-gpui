use super::*;

#[gpui::test]
fn ui_audit_gui_config_warning_offers_a_recovery_entry(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.simulate_resize(size(px(1000.), px(600.)));
    view.update(cx, |view, cx| {
        view.gui_config_diagnostic
            .sync(Some("Unknown setting; using defaults"));
        cx.notify();
    });
    draw(cx);
    assert!(cx.debug_bounds("gui-config-recovery").is_some());
}
