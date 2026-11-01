use super::*;
use crate::sidebar::layout_tests::{fixture_window, full_draw};
use gpui::{px, size};

#[gpui::test]
fn narrow_bars_keep_network_and_usage_before_telemetry(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut view = fixture_window(window, cx);
        view.live.status = crate::state::ConnectionStatus::Connected;
        view.config.show_system_load = true;
        view.config.usage.show = true;
        view.system_load.record(
            super::super::Host::Local,
            crate::system_load::Sample {
                cpu: Some(12.),
                ..Default::default()
            },
        );
        let codex = provider("codex");
        view.usage = super::super::Usage::showing(
            super::super::Host::Local,
            vec![Reading {
                provider: codex,
                report: Some(report(codex, 80.)),
                error: None,
                access: None,
            }],
        );
        view
    });
    for width in [1280., 960., 1280.] {
        cx.simulate_resize(size(px(width), px(600.)));
        cx.update(|window, cx| full_draw(window, cx).clear(cx));
        assert_eq!(cx.debug_bounds("system-load").is_some(), width >= 1200.);
        let bar = cx.debug_bounds("connection-status").unwrap();
        for selector in ["status-tailnet", "usage-codex"] {
            let item = cx.debug_bounds(selector).unwrap();
            assert!(
                item.left() >= bar.left() && item.right() <= bar.right(),
                "{selector}"
            );
            assert!(
                item.top() >= bar.top() && item.bottom() <= bar.bottom(),
                "{selector}"
            );
        }
    }
    assert!(view.read_with(cx, |view, _| view.config.show_system_load));
}
