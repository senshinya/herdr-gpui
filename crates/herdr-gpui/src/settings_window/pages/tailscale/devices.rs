use super::*;
use crate::search_input::Changed;
use crate::settings_window::{Section, scrollbar::Pane};
use std::ops::Range;

impl SettingsWindow {
    #[cfg(all(feature = "integration-test", feature = "mockup", target_os = "macos"))]
    pub(in crate::settings_window) fn native_tailnet_devices(
        &mut self,
        snapshot: herdr_remote::tailscale::Snapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tailnet.poll = None;
        self.poll_tailnet(snapshot, cx);
        self.select_tailnet_tab(TailnetTab::Devices, window, cx);
    }

    pub(super) fn open_tailnet_devices(&mut self, cx: &mut Context<Self>) {
        if self.tailnet.peer_search.is_some() {
            return;
        }
        let input = cx.new(SearchInput::new);
        input.update(cx, |input, cx| {
            input.set_appearance(self.config.ui.clone(), self.theme.clone(), cx);
            input.set_placeholder_with(|| fl!("tailnet-search-devices"), cx);
        });
        self.tailnet.peer_changed = Some(cx.subscribe(&input, |view, _, _: &Changed, cx| {
            view.refilter_tailnet_peers(cx);
            view.tailnet
                .peer_scroll
                .scroll_to_item(0, ScrollStrategy::Top);
            cx.notify();
        }));
        self.tailnet.peer_search = Some(input);
        self.tailnet.overview_focus = Some(cx.focus_handle());
        self.tailnet.devices_focus = Some(cx.focus_handle());
        self.tailnet.filter_focus = Some(cx.focus_handle());
        self.refilter_tailnet_peers(cx);
    }

    pub(super) fn refilter_tailnet_peers(&mut self, cx: &App) {
        let query = self
            .tailnet
            .peer_search
            .as_ref()
            .map(|input| input.read(cx).text().trim().to_lowercase())
            .unwrap_or_default();
        self.tailnet.peer_indices = self
            .tailnet
            .snapshot
            .status
            .peers
            .iter()
            .enumerate()
            .filter(|(_, peer)| !self.tailnet.online_only || peer.available())
            .filter(|(_, peer)| {
                query.is_empty()
                    || peer.name.to_lowercase().contains(&query)
                    || peer.dns_name.to_lowercase().contains(&query)
                    || peer.os.to_lowercase().contains(&query)
                    || peer.addresses.iter().any(|ip| ip.contains(&query))
            })
            .map(|(index, _)| index)
            .collect();
    }

    pub(super) fn select_tailnet_tab(
        &mut self,
        tab: TailnetTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if tab == TailnetTab::Devices && self.tailnet.snapshot.status.state != State::Connected {
            return;
        }
        self.tailnet.tab = tab;
        self.body_scroll.set_offset(Point::default());
        if tab == TailnetTab::Devices {
            if let Some(input) = &self.tailnet.peer_search {
                window.focus(&input.read(cx).focus.clone(), cx);
            }
        } else if let Some(focus) = &self.tailnet.overview_focus {
            window.focus(focus, cx);
        }
        cx.notify();
    }

    pub(in crate::settings_window) fn tailnet_devices_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.section != Section::Tailscale
            || self.tailnet.snapshot.status.state != State::Connected
        {
            return false;
        }
        let Some(input) = &self.tailnet.peer_search else {
            return false;
        };
        if input.read(cx).is_composing()
            || event.keystroke.modifiers.control
            || event.keystroke.modifiers.platform
            || event.keystroke.modifiers.alt
        {
            return false;
        }
        let mut focus: Vec<_> = [&self.tailnet.overview_focus, &self.tailnet.devices_focus]
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        if self.tailnet.tab == TailnetTab::Devices {
            focus.push(input.read(cx).focus.clone());
            focus.extend(self.tailnet.filter_focus.clone());
        }
        let Some(index) = focus.iter().position(|focus| focus.is_focused(window)) else {
            if event.keystroke.key == "tab" && self.focus.is_focused(window) {
                let next = if event.keystroke.modifiers.shift {
                    focus.last()
                } else {
                    focus.first()
                };
                if let Some(next) = next {
                    window.focus(next, cx);
                    return true;
                }
            }
            return false;
        };
        match event.keystroke.key.as_str() {
            "tab" => {
                let next = (index
                    + if event.keystroke.modifiers.shift {
                        focus.len() - 1
                    } else {
                        1
                    })
                    % focus.len();
                window.focus(&focus[next], cx);
            }
            "enter" | "space" if index < 2 => self.select_tailnet_tab(
                if index == 0 {
                    TailnetTab::Overview
                } else {
                    TailnetTab::Devices
                },
                window,
                cx,
            ),
            "enter" | "space" if index == 3 => self.toggle_tailnet_online(cx),
            _ => return false,
        }
        true
    }

    fn toggle_tailnet_online(&mut self, cx: &mut Context<Self>) {
        self.tailnet.online_only = !self.tailnet.online_only;
        self.refilter_tailnet_peers(cx);
        self.tailnet
            .peer_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    pub(in crate::settings_window) fn tailnet_peer_scroll(&self) -> ScrollHandle {
        self.tailnet.peer_scroll.0.borrow().base_handle.clone()
    }

    pub(in crate::settings_window) fn render_tailscale_pane(
        &self,
        overview: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        let connected = self.tailnet.snapshot.status.state == State::Connected;
        let mut header = div().flex_none().px(px(28.)).pt(px(24.)).pb(px(14.)).child(
            div()
                .text_size(px(18.))
                .line_height(px(26.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(Section::Tailscale.label()),
        );
        if connected {
            let tab = |tab, id, label, focus: Option<&FocusHandle>, cx: &mut Context<Self>| {
                ui::button(&self.theme, id, label, true)
                    .role(Role::Tab)
                    .aria_selected(self.tailnet.tab == tab)
                    .when(self.tailnet.tab == tab, |button| {
                        button.bg(rgb(self.theme.active))
                    })
                    .when_some(focus, |button, focus| button.track_focus(focus))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.select_tailnet_tab(tab, window, cx)
                    }))
            };
            header = header.child(
                div()
                    .id("tailnet-settings-tabs")
                    .role(Role::TabList)
                    .flex()
                    .gap(px(6.))
                    .mt(px(14.))
                    .child(tab(
                        TailnetTab::Overview,
                        "tailnet-tab-overview",
                        fl!("tailnet-overview"),
                        self.tailnet.overview_focus.as_ref(),
                        cx,
                    ))
                    .child(tab(
                        TailnetTab::Devices,
                        "tailnet-tab-devices",
                        fl!("tailnet-settings-devices"),
                        self.tailnet.devices_focus.as_ref(),
                        cx,
                    )),
            );
        }
        let devices = connected && self.tailnet.tab == TailnetTab::Devices;
        let mut pane = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(header);
        if !devices {
            return pane.child(
                self.scroll_area(
                    Pane::Body,
                    div()
                        .id("settings-body")
                        .debug_selector(|| "settings-body".into())
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.body_scroll)
                        .px(px(28.))
                        .pb(px(28.))
                        .child(overview),
                    cx,
                ),
            );
        }
        let search = div()
            .flex_none()
            .px(px(28.))
            .pb(px(12.))
            .flex()
            .items_center()
            .gap(px(12.))
            .child(
                div()
                    .debug_selector(|| "tailnet-peer-search".into())
                    .flex_1()
                    .min_w_0()
                    .children(self.tailnet.peer_search.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(fl!("tailnet-online-only"))
                    .child(
                        ui::toggle(
                            &self.theme,
                            "tailnet-online-filter",
                            self.tailnet.online_only,
                            true,
                        )
                        .aria_label(fl!("tailnet-online-only"))
                        .when_some(self.tailnet.filter_focus.as_ref(), |toggle, focus| {
                            toggle.track_focus(focus)
                        })
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.toggle_tailnet_online(cx);
                        })),
                    ),
            );
        pane = pane.child(search);
        let count = self.tailnet.peer_indices.len();
        if count == 0 {
            return pane.child(
                div()
                    .flex_1()
                    .px(px(28.))
                    .py(px(20.))
                    .text_color(rgb(self.theme.subtext()))
                    .child(fl!("tailnet-no-matching-devices")),
            );
        }
        let list = uniform_list(
            "tailnet-settings-peer-list",
            count,
            cx.processor(|view, range: Range<usize>, _, _| {
                range
                    .filter_map(|index| {
                        let peer = view
                            .tailnet
                            .snapshot
                            .status
                            .peers
                            .get(*view.tailnet.peer_indices.get(index)?)?;
                        let status = if peer.expired {
                            fl!("tailnet-peer-expired")
                        } else if peer.online {
                            fl!("tailnet-peer-online")
                        } else {
                            fl!("tailnet-peer-offline")
                        };
                        Some(
                            div()
                                .id(ElementId::Name(
                                    format!("tailnet-settings-peer-{}", peer.id).into(),
                                ))
                                .debug_selector({
                                    let id = peer.id.clone();
                                    move || format!("tailnet-settings-peer-{id}")
                                })
                                .w_full()
                                .h(px((view.config.ui.size * 3. + 16.).ceil()))
                                .px(px(28.))
                                .py(px(6.))
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(div().truncate().child(
                                            crate::notifications::safe_text(&peer.name, 256),
                                        ))
                                        .child(
                                            div()
                                                .truncate()
                                                .text_color(rgb(view.theme.subtext()))
                                                .child(format!(
                                                    "{}  {}",
                                                    peer.os,
                                                    peer.addresses.join(", ")
                                                )),
                                        ),
                                )
                                .child(
                                    div()
                                        .id(ElementId::Name(
                                            format!("tailnet-settings-peer-status-{}", peer.id)
                                                .into(),
                                        ))
                                        .debug_selector({
                                            let id = peer.id.clone();
                                            move || format!("tailnet-settings-peer-status-{id}")
                                        })
                                        .flex_none()
                                        .w(rems(7.))
                                        .truncate()
                                        .role(Role::Label)
                                        .aria_label(status.clone())
                                        .tooltip({
                                            let text = SharedString::from(status.clone());
                                            let (foreground, surface) =
                                                (view.theme.foreground, view.theme.surface);
                                            move |_, cx| {
                                                cx.new(|_| crate::usage::Hint {
                                                    text: text.clone(),
                                                    foreground,
                                                    surface,
                                                })
                                                .into()
                                            }
                                        })
                                        .text_color(rgb(view.theme.subtext()))
                                        .child(status),
                                ),
                        )
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .debug_selector(|| "tailnet-settings-peer-list".into())
        .flex_1()
        .min_h_0()
        .track_scroll(&self.tailnet.peer_scroll);
        pane.child(self.scroll_area(Pane::TailnetDevices, list, cx))
            .child(
                div()
                    .flex_none()
                    .px(px(28.))
                    .py(px(10.))
                    .text_color(rgb(self.theme.subtext()))
                    .child(fl!(
                        "tailnet-device-count",
                        shown = count,
                        total = self
                            .tailnet
                            .snapshot
                            .status
                            .total
                            .max(self.tailnet.snapshot.status.peers.len())
                    )),
            )
    }
}
