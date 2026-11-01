//! Embedded Tailnet settings. Workers own IO; the page polls cached state.
use super::{SettingsWindow, ui};
use crate::{
    i18n::{self, fl},
    search_input::SearchInput,
};
use gpui::{prelude::*, *};
use herdr_remote::tailscale::{Config, State};

mod advanced;
mod devices;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum TailnetTab {
    #[default]
    Overview,
    Devices,
}

#[derive(Default)]
pub(in crate::settings_window) struct TailnetSettings {
    advanced: crate::motion::Toggle,
    advanced_focus: Option<FocusHandle>,
    busy: bool,
    confirm_logout: bool,
    control: Option<Entity<SearchInput>>,
    devices_focus: Option<FocusHandle>,
    filter_focus: Option<FocusHandle>,
    hostname: Option<Entity<SearchInput>>,
    online_only: bool,
    opened_login: bool,
    overview_focus: Option<FocusHandle>,
    peer_changed: Option<Subscription>,
    peer_indices: Vec<usize>,
    peer_scroll: UniformListScrollHandle,
    peer_search: Option<Entity<SearchInput>>,
    plaintext: bool,
    poll: Option<Task<()>>,
    snapshot: herdr_remote::tailscale::Snapshot,
    tab: TailnetTab,
}

pub(crate) fn open(source: WeakEntity<crate::HerdrWindow>, cx: &mut App) {
    open_tab(source, TailnetTab::Overview, cx);
}

pub(crate) fn open_devices(source: WeakEntity<crate::HerdrWindow>, cx: &mut App) {
    open_tab(source, TailnetTab::Devices, cx);
}

fn open_tab(source: WeakEntity<crate::HerdrWindow>, tab: TailnetTab, cx: &mut App) {
    crate::settings_window::open(source, cx);
    cx.defer(move |cx| {
        if let Some(handle) = cx
            .default_global::<crate::settings_window::SettingsWindowHandle>()
            .window
        {
            let _ = handle.update(cx, |view, window, cx| {
                view.select_section(super::super::Section::Tailscale, window, cx);
                view.select_tailnet_tab(tab, window, cx)
            });
        }
    });
}

fn state_label(state: State) -> String {
    i18n::message(
        match state {
            State::Disconnected => "tailnet-disconnected",
            State::Connecting => "tailnet-connecting",
            State::Disconnecting => "tailnet-disconnecting",
            State::NeedsLogin => "tailnet-needs-login",
            State::NeedsApproval => "tailnet-needs-approval",
            State::Connected => "tailnet-connected",
            State::Failed => "tailnet-failed",
        },
        &[],
    )
}

impl SettingsWindow {
    pub(in crate::settings_window) fn open_tailscale(&mut self, cx: &mut Context<Self>) {
        self.tailnet.tab = TailnetTab::Overview;
        self.tailnet.snapshot = crate::tailnet::snapshot();
        if self.tailnet.hostname.is_none() {
            let field = |text: &str, placeholder: &'static str, cx: &mut Context<Self>| {
                cx.new(|cx| {
                    let mut input = SearchInput::new(cx);
                    input.set_text_selected(text, cx);
                    input.set_placeholder(placeholder, cx);
                    input.set_appearance(self.config.terminal.clone(), self.theme.clone(), cx);
                    input
                })
            };
            self.tailnet.hostname = Some(field(&self.config.tailscale.hostname, "herdr-ui", cx));
            self.tailnet.control = Some(field(
                &self.config.tailscale.control_url,
                "https://control.example.com",
                cx,
            ));
            self.tailnet.plaintext = self.config.tailscale.allow_plaintext_credentials;
            self.tailnet.advanced.set(
                !self.config.tailscale.control_url.is_empty() || self.tailnet.plaintext,
                crate::motion::now(),
                true,
            );
        }
        if self.tailnet.advanced_focus.is_none() {
            self.tailnet.advanced_focus = Some(cx.focus_handle());
        }
        self.open_tailnet_devices(cx);
        if self.tailnet.poll.is_none() {
            self.tailnet.poll = Some(cx.spawn(async |view, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    if view
                        .update(cx, |view, cx| {
                            view.poll_tailnet(crate::tailnet::snapshot(), cx)
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
    }

    pub(in crate::settings_window) fn poll_tailnet(
        &mut self,
        snapshot: herdr_remote::tailscale::Snapshot,
        cx: &mut Context<Self>,
    ) {
        if self.section == super::super::Section::Tailscale
            && snapshot.status.state == State::NeedsLogin
            && !self.tailnet.opened_login
            && let Some(url) = snapshot.auth_url.as_ref()
        {
            use secrecy::ExposeSecret;
            let url = url.expose_secret();
            if valid_login_url(url) {
                self.tailnet.opened_login = true;
                cx.open_url(url);
            }
        }
        self.tailnet.snapshot = snapshot;
        if self.tailnet.snapshot.status.state != State::Connected {
            self.tailnet.tab = TailnetTab::Overview;
        }
        self.refilter_tailnet_peers(cx);
        cx.notify();
    }

    pub(in crate::settings_window) fn sync_tailnet_fields(&mut self, cx: &mut Context<Self>) {
        if let Some(input) = &self.tailnet.peer_search {
            input.update(cx, |input, cx| {
                input.set_appearance(self.config.ui.clone(), self.theme.clone(), cx)
            });
        }
        for field in [&self.tailnet.hostname, &self.tailnet.control]
            .into_iter()
            .flatten()
        {
            field.update(cx, |input, cx| {
                input.set_appearance(self.config.terminal.clone(), self.theme.clone(), cx)
            });
        }
    }

    fn tailnet_action(&mut self, action: Action, cx: &mut Context<Self>) {
        if self.tailnet.busy {
            return;
        }
        if matches!(action, Action::Logout) && !self.tailnet.confirm_logout {
            self.tailnet.confirm_logout = true;
            cx.notify();
            return;
        }
        self.tailnet.confirm_logout = false;
        if matches!(action, Action::Connect) {
            self.tailnet.opened_login = false;
        }
        let config = Config {
            enabled: true,
            hostname: self
                .tailnet
                .hostname
                .as_ref()
                .map(|f| f.read(cx).text().trim().to_owned())
                .unwrap_or_default(),
            control_url: self
                .tailnet
                .control
                .as_ref()
                .map(|f| f.read(cx).text().trim().to_owned())
                .unwrap_or_default(),
            allow_plaintext_credentials: self.tailnet.plaintext,
        };
        self.tailnet.busy = true;
        let job = cx.background_executor().spawn(async move {
            match action {
                Action::Connect => crate::tailnet::apply(config),
                Action::Disconnect => crate::tailnet::disconnect(false),
                Action::Logout => crate::tailnet::disconnect(true),
            }
        });
        cx.spawn(async move |view, cx| {
            let result = job.await;
            let _ = view.update(cx, |view, cx| {
                view.tailnet.busy = false;
                view.error = result.err().map(|e| e.to_string());
                view.poll_tailnet(crate::tailnet::snapshot(), cx);
            });
        })
        .detach();
        cx.notify();
    }

    pub(in crate::settings_window) fn close_tailscale(&mut self) {
        self.tailnet.poll = None;
        self.tailnet.confirm_logout = false;
        self.tailnet.snapshot = Default::default();
    }

    pub(in crate::settings_window) fn render_tailscale(&self, cx: &mut Context<Self>) -> Div {
        if self.tailnet.tab == TailnetTab::Devices
            && self.tailnet.snapshot.status.state == State::Connected
        {
            return div();
        }
        let snapshot = self.tailnet.snapshot.clone();
        let status = &snapshot.status;
        let busy = self.tailnet.busy || status.state == State::Disconnecting;
        let mut body = div().flex().flex_col().min_w_0();
        if status.state != State::Disconnected {
            body = body.child(
                ui::row(&self.theme, "tailnet-status", fl!("tailnet-status"))
                    .description(state_label(status.state)),
            );
        }
        if status.state == State::Connected {
            body = body
                .child(
                    ui::row(&self.theme, "tailnet-network", fl!("tailnet-network"))
                        .description(status.network.clone()),
                )
                .child(
                    ui::row(&self.theme, "tailnet-node", fl!("tailnet-node")).description(format!(
                        "{}  {}",
                        status.self_name,
                        status.self_ips.join(", ")
                    )),
                );
        }
        if let Some(error) = &status.error {
            body = body.child(
                div()
                    .text_color(rgb(self.theme.foreground))
                    .child(crate::notifications::safe_text(error, 300)),
            );
        }
        if let Some(url) = snapshot.auth_url {
            use secrecy::ExposeSecret;
            let url = url.expose_secret().to_owned();
            if valid_login_url(&url) {
                body = body.child(
                    ui::row(&self.theme, "tailnet-login", fl!("tailnet-needs-login")).control(
                        ui::button(&self.theme, "tailnet-browser", fl!("tailnet-browser"), true)
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    ),
                );
            }
        }
        body = body.child(
            div()
                .flex()
                .flex_col()
                .relative()
                .child(crate::settings_window::search::track_bounds(
                    "tailnet-hostname".into(),
                ))
                .gap(px(6.))
                .py(px(10.))
                .child(fl!("tailnet-hostname"))
                .children(self.tailnet.hostname.clone())
                .id("tailnet-hostname")
                .debug_selector(|| "tailnet-hostname".into()),
        );
        body.child(self.render_tailnet_advanced(busy, cx)).child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(8.))
                .py(px(12.))
                .child(
                    ui::primary_button(
                        &self.theme,
                        "tailnet-connect",
                        fl!("tailnet-connect"),
                        !busy && !matches!(status.state, State::Connected | State::Connecting),
                    )
                    .when(
                        !busy && !matches!(status.state, State::Connected | State::Connecting),
                        |b| {
                            b.on_click(cx.listener(|this, _, _, cx| {
                                this.tailnet_action(Action::Connect, cx)
                            }))
                        },
                    ),
                )
                .child(
                    ui::button(
                        &self.theme,
                        "tailnet-disconnect",
                        fl!("tailnet-disconnect"),
                        !busy && status.state != State::Disconnected,
                    )
                    .when(!busy && status.state != State::Disconnected, |b| {
                        b.on_click(
                            cx.listener(|this, _, _, cx| {
                                this.tailnet_action(Action::Disconnect, cx)
                            }),
                        )
                    }),
                )
                .child(
                    ui::button(
                        &self.theme,
                        "tailnet-logout",
                        if self.tailnet.confirm_logout {
                            fl!("tailnet-confirm-logout")
                        } else {
                            fl!("tailnet-logout")
                        },
                        !busy,
                    )
                    .when(!busy, |b| {
                        b.on_click(
                            cx.listener(|this, _, _, cx| this.tailnet_action(Action::Logout, cx)),
                        )
                    }),
                ),
        )
    }
}
#[derive(Clone, Copy)]
enum Action {
    Connect,
    Disconnect,
    Logout,
}

fn valid_login_url(value: &str) -> bool {
    value.len() <= 4096
        && url::Url::parse(value).is_ok_and(|url| {
            url.scheme() == "https"
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
        })
}
