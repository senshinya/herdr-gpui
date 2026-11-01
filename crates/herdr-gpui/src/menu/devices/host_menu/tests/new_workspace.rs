use super::*;

#[gpui::test]
fn device_menu_new_workspace_selects_the_device_that_was_right_clicked(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut view = fixture_window(window, cx);
        add_host(&mut view);
        view
    });
    cx.simulate_resize(size(px(800.), px(600.)));
    for id in [HOST, "local"] {
        view.update_in(cx, |view, window, cx| {
            view.open_host_menu(id, point(px(20.), px(20.)), window, cx);
            assert_eq!(view.menu.page, Some(Page::Host), "{id}");
        });
        let index = view.read_with(cx, |view, _| {
            let Some(index) = host(view)
                .actions()
                .iter()
                .position(|(_, message)| *message == "sidebar-new-workspace")
            else {
                panic!("device menu must offer New Workspace");
            };
            index
        });
        cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
        let selector: &'static str = format!("host-menu-{index}").leak();
        let row = cx.debug_bounds(selector).unwrap();
        cx.simulate_click(row.center(), Modifiers::default());
        view.read_with(cx, |view, _| {
            assert_eq!(view.endpoints[view.selected_endpoint].id, id);
            assert!(view.menu.page.is_none());
        });
    }
}

#[gpui::test]
fn local_menu_only_offers_new_workspace(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    view.update_in(cx, |view, window, cx| {
        view.open_host_menu("local", point(px(20.), px(20.)), window, cx);
        assert_eq!(view.menu.page, Some(Page::Host));
        assert!(
            host(view)
                .actions()
                .iter()
                .all(|(_, message)| *message == "sidebar-new-workspace")
        );
        assert!(!host(view).actions().is_empty());
    });
}
