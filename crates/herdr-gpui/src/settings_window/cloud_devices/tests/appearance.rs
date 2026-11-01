#![allow(clippy::unwrap_used)]
use super::super::super::{Section, SettingsWindow};
use crate::{cloud::CloudProvider, config::Theme, sidebar::layout_tests::full_draw};
use gpui::{Background, TestAppContext, px, rgb, size};

#[gpui::test]
fn cloud_fields_follow_theme_and_font_changes_after_the_cards_open(cx: &mut TestAppContext) {
    let main = cx.add_window(crate::sidebar::layout_tests::fixture_window);
    let weak = cx.update(|cx| main.update(cx, |_, _, cx| cx.weak_entity()).unwrap());
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut view = SettingsWindow::new(weak, cx);
        view.section = Section::CloudDevices;
        view.open_cloud_devices(cx);
        view
    });
    cx.simulate_resize(size(px(1200.), px(4000.)));
    let mut initial = Vec::new();
    for &provider in CloudProvider::ALL {
        view.update(cx, |view, cx| {
            view.cloud_tab = provider;
            cx.notify();
        });
        cx.update(|window, cx| full_draw(window, cx).clear(cx));
        initial.push((
            provider,
            cx.debug_bounds("theme-search").unwrap().size.height,
        ));
    }
    view.update(cx, |view, cx| {
        view.config.ui.size = 24.;
        view.theme = Theme::builtin("Catppuccin Latte").unwrap();
        view.refresh_control_appearance(cx);
    });
    for (provider, before) in initial {
        view.update(cx, |view, cx| {
            view.cloud_tab = provider;
            cx.notify();
        });
        cx.update(|window, cx| full_draw(window, cx).clear(cx));
        let field = cx.debug_bounds("theme-search").unwrap();
        assert!(
            field.size.height > before,
            "{provider:?}: font stayed unchanged"
        );
        cx.update(|window, cx| {
            let fill = Background::from(rgb(view.read(cx).theme.background));
            let region = field.scale(window.scale_factor());
            assert!(
                window
                    .painted_quads()
                    .iter()
                    .any(|quad| { quad.bounds == region && quad.background == fill }),
                "{provider:?}: field kept the previous theme"
            );
        });
    }
}
