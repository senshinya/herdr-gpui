#![allow(clippy::unwrap_used)]
use super::*;
use crate::sidebar::layout_tests::{fixture_window, full_draw};
use core::prelude::v1::test;

#[gpui::test]
fn every_tailnet_state_has_visible_and_spoken_feedback(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    for font_size in [12., 24.] {
        for width in [480., 960.] {
            cx.simulate_resize(size(px(width), px(600.)));
            for (state, label) in [
                (State::Disconnected, "Tailnet · Disconnected"),
                (State::Connecting, "Tailnet · Connecting…"),
                (State::Disconnecting, "Tailnet · Disconnecting…"),
                (State::NeedsLogin, "Tailnet · Sign in"),
                (State::NeedsApproval, "Tailnet · Awaiting approval"),
                (State::Connected, "Tailnet · Connected"),
                (State::Failed, "Tailnet · Failed"),
            ] {
                view.update(cx, |view, cx| {
                    view.config.tailscale.enabled = true;
                    view.config.ui.size = font_size;
                    view.tailnet_status.state = state;
                    let element = view.render_tailnet_status(cx);
                    assert_eq!(element.a11y_role(), Some(Role::Button));
                    let mut node = accesskit::Node::new(Role::Button);
                    element.write_a11y_info(&mut node);
                    assert_eq!(node.label(), Some(label));
                    cx.notify();
                });
                cx.update(|window, cx| full_draw(window, cx).clear(cx));
                let bar = cx.debug_bounds("connection-status").unwrap();
                let status = cx.debug_bounds("status-tailnet").unwrap();
                assert!(status.left() >= bar.left() && status.right() <= bar.right());
                assert!(status.top() >= bar.top() && status.bottom() <= bar.bottom());
            }
        }
    }
}

#[test]
fn rapid_ticks_keep_the_cached_network_state() {
    let now = Instant::now();
    let mut status = Tailnet {
        checked: Some(now),
        state: State::NeedsApproval,
    };
    assert!(!status.poll(now + Duration::from_millis(499)));
    assert_eq!(status.state, State::NeedsApproval);
}
