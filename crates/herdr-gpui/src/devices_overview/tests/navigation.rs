use super::*;
use crate::sidebar::ActivityPanel;

#[gpui::test]
fn ui_audit_opening_a_page_from_an_activity_section_reveals_its_tab(cx: &mut gpui::TestAppContext) {
    let (view, cx) = window(cx);
    view.update_in(cx, |view, window, cx| {
        view.config.sidebar_style.activity_bar = true;
        view.show_activity_panel(ActivityPanel::Usage, window, cx);
    });
    draw(cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.activity_section()),
        Some(ActivityPanel::Usage)
    );
    open(&view, cx);
    assert!(cx.debug_bounds("devices-tab").is_some());
    assert!(view.read_with(cx, |view, _| view.activity_section().is_none()));
}
