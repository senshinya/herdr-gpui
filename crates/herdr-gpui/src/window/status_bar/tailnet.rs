//! The embedded network's state, independent of any daemon or SSH session.
use super::HerdrWindow;
use crate::{i18n::fl, window::chrome_focus::Region};
use gpui::{prelude::*, *};
use herdr_remote::tailscale::State;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct Tailnet {
    checked: Option<Instant>,
    state: State,
}

impl Tailnet {
    pub(super) fn shown(&self, enabled: bool, available: f32) -> bool {
        enabled || self.state != State::Disconnected || available >= 420.
    }
    pub(in crate::window) fn poll(&mut self, now: Instant) -> bool {
        if self.checked.is_some_and(|checked| {
            now.saturating_duration_since(checked) < Duration::from_millis(500)
        }) {
            return false;
        }
        self.checked = Some(now);
        let state = crate::tailnet::snapshot().status.state;
        let changed = self.state != state;
        self.state = state;
        changed
    }
}

impl HerdrWindow {
    pub(super) fn render_tailnet_status(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let state = self.tailnet_status.state;
        let label = match state {
            State::Disconnected if !self.config.tailscale.enabled => fl!("tailnet-bar-off"),
            State::Disconnected => fl!("tailnet-bar-disconnected"),
            State::Connecting => fl!("tailnet-bar-connecting"),
            State::Disconnecting => fl!("tailnet-bar-disconnecting"),
            State::NeedsLogin => fl!("tailnet-bar-needs-login"),
            State::NeedsApproval => fl!("tailnet-bar-needs-approval"),
            State::Connected => fl!("tailnet-bar-connected"),
            State::Failed => fl!("tailnet-bar-failed"),
        };
        let compact = self.status_bar_width() < 420.;
        let width = if compact {
            (self.status_bar_width() - 80.).max(80.)
        } else {
            (self.status_bar_width() * 0.4).clamp(120., 280.)
        };
        let short = match state {
            State::Disconnected => fl!("tailnet-disconnected"),
            State::Connecting => fl!("tailnet-connecting"),
            State::Disconnecting => fl!("tailnet-bar-short-disconnecting"),
            State::NeedsLogin => fl!("tailnet-bar-short-login"),
            State::NeedsApproval => fl!("tailnet-bar-short-approval"),
            State::Connected => fl!("tailnet-connected"),
            State::Failed => fl!("tailnet-bar-short-failed"),
        };
        let color = match state {
            State::Failed => self.theme.palette[1],
            State::NeedsLogin | State::NeedsApproval => self.theme.palette[3],
            _ => self.theme.foreground,
        };
        let color = crate::contrast::ink_on_chrome(
            color,
            [self.theme.background, self.theme.surface, self.theme.active],
            4.5,
        );
        let hint = format!("{label}\n{}", fl!("tailnet-bar-open-settings"));
        self.chrome_stop(Region::StatusBar, div().id("status-tailnet"))
            .debug_selector(|| "status-tailnet".into())
            .role(Role::Button)
            .aria_label(label.clone())
            .aria_description(fl!("tailnet-bar-open-settings"))
            .flex_none()
            .min_w_0()
            .max_w(px(width))
            .h_full()
            .px(px(6.))
            .flex()
            .items_center()
            .gap(px(5.))
            .text_color(rgb(color))
            .whitespace_nowrap()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(self.theme.active)))
            .when(compact, |button| {
                button.child(
                    svg()
                        .path("icons/network/tailscale.svg")
                        .size(px(16.))
                        .flex_none()
                        .text_color(rgb(color)),
                )
            })
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .child(if compact { short } else { label }),
            )
            .tooltip(self.hint(hint))
            .on_click(cx.listener(|_, _, _, cx| {
                crate::settings_window::open_tailscale(cx.weak_entity(), cx);
            }))
    }
}

#[cfg(test)]
mod tests;
