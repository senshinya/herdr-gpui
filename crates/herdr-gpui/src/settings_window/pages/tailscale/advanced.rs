use super::*;

impl SettingsWindow {
    pub(in crate::settings_window) fn reveal_tailnet_advanced(&mut self) {
        self.tailnet.advanced.set(true, crate::motion::now(), true);
    }

    pub(in crate::settings_window) fn tailnet_advanced_moving(&self) -> bool {
        self.tailnet.advanced.moving(crate::motion::now())
    }

    fn toggle_tailnet_advanced(&mut self, cx: &mut Context<Self>) {
        self.tailnet.advanced.set(
            !self.tailnet.advanced.is_open(),
            crate::motion::now(),
            cfg!(test),
        );
        cx.notify();
    }

    pub(super) fn render_tailnet_advanced(&self, busy: bool, cx: &mut Context<Self>) -> Div {
        let open = self.tailnet.advanced.is_open();
        let shown = self.tailnet.advanced.shown(crate::motion::now());
        let button = ui::button(
            &self.theme,
            "tailnet-advanced",
            fl!("tailnet-advanced"),
            true,
        )
        .aria_expanded(open)
        .when_some(self.tailnet.advanced_focus.as_ref(), |button, focus| {
            button.track_focus(focus)
        })
        .on_click(cx.listener(|this, _, _, cx| this.toggle_tailnet_advanced(cx)))
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space")
                && event.keystroke.modifiers == Modifiers::default()
            {
                this.toggle_tailnet_advanced(cx);
                window.prevent_default();
                cx.stop_propagation();
            }
        }));
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(button)
            .when(open || shown > 0., |body| {
                body.child(
                    div()
                        .w_full()
                        .relative()
                        .top(px((1. - shown) * -4.))
                        .opacity(shown)
                        .child(
                            div()
                                .id("tailnet-control")
                                .debug_selector(|| "tailnet-control".into())
                                .flex()
                                .flex_col()
                                .relative()
                                .child(crate::settings_window::search::track_bounds(
                                    "tailnet-control".into(),
                                ))
                                .gap(px(6.))
                                .py(px(10.))
                                .child(fl!("tailnet-control"))
                                .children(self.tailnet.control.clone()),
                        )
                        .child(
                            ui::row(&self.theme, "tailnet-plaintext", fl!("tailnet-plaintext"))
                                .description(if cfg!(unix) {
                                    fl!("tailnet-plaintext-note")
                                } else {
                                    fl!("tailnet-credentials-system-only")
                                })
                                .switch(
                                    ui::toggle(
                                        &self.theme,
                                        "tailnet-plaintext-toggle",
                                        cfg!(unix) && self.tailnet.plaintext,
                                        !busy && cfg!(unix),
                                    )
                                    .when(
                                        !busy && cfg!(unix),
                                        |toggle| {
                                            toggle.on_click(cx.listener(|this, _, _, cx| {
                                                this.tailnet.plaintext = !this.tailnet.plaintext;
                                                cx.notify();
                                            }))
                                        },
                                    ),
                                ),
                        ),
                )
            })
    }
}
