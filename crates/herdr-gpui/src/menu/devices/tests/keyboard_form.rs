use super::*;
use crate::sidebar::layout_tests::full_draw;
use herdr_client::HostKeys;

#[gpui::test]
fn ui_audit_keyboard_users_can_change_host_key_verification_and_return_to_fields(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(fixture_window);
    view.update_in(cx, |view, window, cx| {
        open_form(view, Step::Form, cx);
        let field = &view.menu.device_setup.as_ref().unwrap().fields[0];
        let focus = field.read(cx).focus.clone();
        window.focus(&focus, cx);
    });
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
    cx.simulate_keystrokes("tab tab tab space");
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.menu.device_setup.as_ref().unwrap().host_keys,
            HostKeys::Skip
        );
    });
    cx.simulate_keystrokes("shift-tab shift-tab shift-tab");
    view.update_in(cx, |view, window, cx| {
        assert!(
            view.menu.device_setup.as_ref().unwrap().fields[0]
                .read(cx)
                .focus
                .is_focused(window)
        );
    });
}
