use super::*;

#[gpui::test]
fn the_default_settings_window_fits_its_categories_without_scrolling(cx: &mut TestAppContext) {
    let main = cx.add_window(crate::sidebar::layout_tests::fixture_window);
    let weak = cx.update(|cx| main.update(cx, |_, _, cx| cx.weak_entity()).unwrap());
    let (view, cx) = cx.add_window_view(|_, cx| SettingsWindow::new(weak, cx));
    let default_size = size(px(960.), px(780.));
    for viewport in [default_size, size(px(680.), px(560.)), default_size] {
        cx.simulate_resize(viewport);
        for _ in 0..2 {
            cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
        }
        if viewport != default_size {
            assert!(view.read_with(cx, |view, _| view.navigation_scroll.max_offset().y) > px(0.));
            continue;
        }
        let list = cx.debug_bounds("settings-sections").unwrap();
        let last = Section::ALL.last().unwrap().selector().leak();
        let row = cx.debug_bounds(last).unwrap();
        assert!(
            row.bottom() <= list.bottom() + px(1.),
            "last category is below the default viewport: {row:?} in {list:?}"
        );
        assert_eq!(
            view.read_with(cx, |view, _| view.navigation_scroll.max_offset().y),
            px(0.),
            "the default navigation should fit without scrolling"
        );
        assert!(cx.debug_bounds("settings-sections-thumb").is_none());
    }
}
