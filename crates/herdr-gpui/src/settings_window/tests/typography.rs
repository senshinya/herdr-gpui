use super::*;

#[gpui::test]
fn descriptions_follow_the_ui_font_size(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    cx.simulate_resize(size(px(1200.), px(1800.)));
    let mut heights = Vec::new();
    for font_size in [12., 24.] {
        view.update(cx, |view, cx| {
            view.section = Section::General;
            view.config.ui.size = font_size;
            cx.notify();
        });
        cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
        heights.push(
            cx.debug_bounds("general-merged-worktrees-description")
                .unwrap()
                .size
                .height,
        );
    }
    assert!(heights[1] > heights[0]);
}
