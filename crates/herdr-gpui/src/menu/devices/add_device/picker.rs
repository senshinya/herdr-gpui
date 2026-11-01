use super::*;
use crate::config::{FontConfig, Theme};
use herdr_remote::tailscale::{Snapshot, State};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::menu::devices) enum PickerSource {
    #[default]
    Nearby,
    Tailnet,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum View {
    #[default]
    Choose,
    Details,
}

pub(in crate::menu::devices) struct Picker {
    view: View,
    source: PickerSource,
    input: Entity<SearchInput>,
    nearby: Option<(String, String)>,
    pub(in crate::menu::devices) snapshot: Snapshot,
    nearby_focus: FocusHandle,
    tailnet_focus: FocusHandle,
    manual_focus: FocusHandle,
    pub(super) back_focus: FocusHandle,
    pub(in crate::menu::devices) highlighted_peer: Option<String>,
}

impl Picker {
    pub(in crate::menu::devices) fn new(
        font: &FontConfig,
        theme: &Theme,
        cx: &mut Context<HerdrWindow>,
    ) -> Self {
        let input = cx.new(SearchInput::new);
        input.update(cx, |input, cx| {
            input.set_appearance(font.clone(), theme.clone(), cx);
            input.set_placeholder_with(|| fl!("devices-picker-search"), cx);
        });
        let mut snapshot = crate::tailnet::snapshot();
        snapshot.auth_url = None;
        Self {
            view: View::Choose,
            source: PickerSource::Nearby,
            input,
            nearby: None,
            snapshot,
            nearby_focus: cx.focus_handle(),
            tailnet_focus: cx.focus_handle(),
            manual_focus: cx.focus_handle(),
            back_focus: cx.focus_handle(),
            highlighted_peer: None,
        }
    }
    pub(in crate::menu::devices) fn choosing(&self) -> bool {
        self.view == View::Choose
    }
    pub(in crate::menu::devices) fn input(&self) -> &Entity<SearchInput> {
        &self.input
    }
    pub(in crate::menu::devices) fn query(&self, cx: &App) -> String {
        self.input.read(cx).text().to_owned()
    }
    pub(in crate::menu::devices) fn select_nearby(&mut self, name: &str, target: &str) {
        self.nearby = Some((name.into(), target.into()));
        self.view = View::Details;
    }
    pub(in crate::menu::devices) fn select_tailnet(&mut self) {
        self.nearby = None;
        self.view = View::Details;
    }
    #[cfg(test)]
    pub(in crate::menu::devices) fn enter_details(&mut self) {
        self.view = View::Details;
    }
    pub(in crate::menu::devices) fn update_snapshot(&mut self, mut snapshot: Snapshot) {
        snapshot.auth_url = None;
        if snapshot.status.state != State::Connected {
            self.source = PickerSource::Nearby;
        }
        self.snapshot = snapshot;
    }
}

impl HerdrWindow {
    #[cfg(all(feature = "integration-test", feature = "mockup", target_os = "macos"))]
    pub(crate) fn native_tailnet_picker(
        &mut self,
        snapshot: Snapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(setup) = &mut self.menu.device_setup {
            setup.tailnet_poll = None;
            setup.picker.update_snapshot(snapshot);
        }
        self.pick_device_source(PickerSource::Tailnet, window, cx);
    }

    pub(in crate::menu::devices) fn show_device_picker(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(setup) = &mut self.menu.device_setup {
            setup.picker.view = View::Choose;
            setup.picker.input.update(cx, |input, cx| input.clear(cx));
            setup.suggestion_scroll.set_offset(Point::default());
            setup.highlight = None;
            setup.picker.highlighted_peer = None;
            window.focus(&setup.picker.input.read(cx).focus.clone(), cx);
        }
        cx.notify();
    }

    fn enter_manual_device(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(setup) = &mut self.menu.device_setup {
            setup.picker.view = View::Details;
            setup.picker.nearby = None;
            setup.tailnet_peer = None;
            setup.fields[0].update(cx, |input, cx| input.clear(cx));
            if setup.suggested_label.take().is_some() {
                setup.fields[1].update(cx, |input, cx| input.clear(cx));
            }
            setup.error = None;
            window.focus(&setup.fields[0].read(cx).focus.clone(), cx);
        }
        cx.notify();
    }

    fn pick_device_source(
        &mut self,
        source: PickerSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(setup) = &mut self.menu.device_setup {
            if source == PickerSource::Tailnet
                && setup.picker.snapshot.status.state != State::Connected
            {
                return;
            }
            setup.picker.source = source;
            setup.suggestion_scroll.set_offset(Point::default());
            setup.highlight = None;
            setup.picker.highlighted_peer = None;
            window.focus(&setup.picker.input.read(cx).focus.clone(), cx);
        }
        cx.notify();
    }

    pub(super) fn device_picker_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(setup) = self.menu.device_setup.as_ref() else {
            return false;
        };
        if setup.picker.input.read(cx).is_composing() {
            return false;
        }
        let key = event.keystroke.key.as_str();
        if event.keystroke.modifiers.control
            || event.keystroke.modifiers.platform
            || event.keystroke.modifiers.alt
        {
            return false;
        }
        if key == "escape" {
            self.dismiss_menu(window, cx);
            return true;
        }
        if key == "tab" {
            let picker = &setup.picker;
            let mut focus = vec![
                picker.input.read(cx).focus.clone(),
                picker.nearby_focus.clone(),
            ];
            if picker.snapshot.status.state == State::Connected {
                focus.push(picker.tailnet_focus.clone());
            }
            focus.push(picker.manual_focus.clone());
            let current = focus
                .iter()
                .position(|handle| handle.is_focused(window))
                .unwrap_or(0);
            let next = (current
                + if event.keystroke.modifiers.shift {
                    focus.len() - 1
                } else {
                    1
                })
                % focus.len();
            window.focus(&focus[next], cx);
            return true;
        }
        if key == "enter" || key == "space" {
            if setup.picker.manual_focus.is_focused(window) {
                self.enter_manual_device(window, cx);
                return true;
            }
            let source = if setup.picker.nearby_focus.is_focused(window) {
                Some(PickerSource::Nearby)
            } else if setup.picker.tailnet_focus.is_focused(window) {
                Some(PickerSource::Tailnet)
            } else {
                None
            };
            if let Some(source) = source {
                self.pick_device_source(source, window, cx);
                return true;
            }
        }
        let source = setup.picker.source;
        if source == PickerSource::Nearby {
            if self.complete_device_target(key, window, cx) {
                return true;
            }
            if key == "enter" {
                let Some(setup) = self.menu.device_setup.as_ref() else {
                    return false;
                };
                let query = setup.picker.query(cx);
                let completion = self.device_completion(setup, &query);
                if let Some((name, target)) = completion.highlighted_choice() {
                    let (name, target) = (name.to_owned(), target.to_owned());
                    self.use_device_suggestion(&target, &name, window, cx);
                }
                return true;
            }
        }
        if source == PickerSource::Tailnet {
            return self.tailnet_picker_key(key, window, cx);
        }
        false
    }

    pub(super) fn render_device_picker(&self, cx: &mut Context<Self>) -> Div {
        let Some(setup) = self.menu.device_setup.as_ref() else {
            return div();
        };
        let picker = &setup.picker;
        let theme = &self.theme;
        let tab = |source,
                   id: &'static str,
                   label: String,
                   focus: &FocusHandle,
                   cx: &mut Context<Self>| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .role(Role::Tab)
                .aria_selected(picker.source == source)
                .aria_label(label.clone())
                .track_focus(focus)
                .flex_none()
                .px(px(10.))
                .py(px(8.))
                .cursor_pointer()
                .border_b_2()
                .border_color(rgb(if picker.source == source {
                    theme.foreground
                } else {
                    theme.background
                }))
                .child(label)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.pick_device_source(source, window, cx)
                }))
        };
        let tabs = div()
            .id("device-picker-tabs")
            .debug_selector(|| "device-picker-tabs".into())
            .role(Role::TabList)
            .flex()
            .gap(px(8.))
            .flex_none()
            .child(tab(
                PickerSource::Nearby,
                "device-picker-nearby",
                fl!("devices-discover-heading"),
                &picker.nearby_focus,
                cx,
            ))
            .when(picker.snapshot.status.state == State::Connected, |tabs| {
                tabs.child(tab(
                    PickerSource::Tailnet,
                    "device-picker-tailnet",
                    fl!("tailnet-devices"),
                    &picker.tailnet_focus,
                    cx,
                ))
            });
        let header = div()
            .flex_none()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(div().flex_1().child(fl!("devices-picker-title")))
                    .child(
                        div()
                            .id("device-picker-manual")
                            .debug_selector(|| "device-picker-manual".into())
                            .role(Role::Button)
                            .track_focus(&picker.manual_focus)
                            .cursor_pointer()
                            .px(px(8.))
                            .py(px(5.))
                            .rounded(px(4.))
                            .hover(|s| s.bg(rgb(theme.active)))
                            .child(fl!("devices-picker-manual"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.enter_manual_device(window, cx)
                            })),
                    ),
            )
            .child(tabs)
            .child(picker.input.clone());
        let mut list = div()
            .id("device-picker-scroll")
            .debug_selector(|| "device-picker-scroll".into())
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .px(px(16.))
            .pb(px(16.));
        list = match picker.source {
            PickerSource::Nearby => list.children(
                setup
                    .discovery
                    .as_ref()
                    .map(|discovery| self.render_device_suggestions(setup, discovery, cx)),
            ),
            PickerSource::Tailnet => {
                list.children(self.render_tailnet_devices(setup, picker.snapshot.clone(), cx))
            }
        };
        div()
            .debug_selector(|| "device-picker".into())
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(header)
            .child(list)
    }

    pub(super) fn render_device_choice_summary(
        &self,
        setup: &Setup,
        cx: &mut Context<Self>,
    ) -> Div {
        let name = setup
            .tailnet_peer
            .as_ref()
            .map(|selection| selection.peer.name.clone())
            .or_else(|| setup.picker.nearby.as_ref().map(|(name, _)| name.clone()));
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .id("device-picker-back")
                    .debug_selector(|| "device-picker-back".into())
                    .role(Role::Button)
                    .track_focus(&setup.picker.back_focus)
                    .cursor_pointer()
                    .text_color(rgb(self.theme.subtext()))
                    .child(fl!("devices-picker-back"))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.show_device_picker(window, cx)),
                    ),
            )
            .when_some(name, |summary, name| {
                summary.child(
                    div()
                        .p(px(10.))
                        .border_1()
                        .border_color(rgb(self.theme.active))
                        .rounded(px(4.))
                        .child(name),
                )
            })
    }
}
