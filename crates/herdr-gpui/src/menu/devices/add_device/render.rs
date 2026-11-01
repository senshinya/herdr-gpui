use super::*;

impl HerdrWindow {
    pub(in crate::menu) fn render_add_device(
        &self,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = &self.theme;
        let frame = |pane: Div, cx: &mut Context<Self>| {
            self.add_device_frame(
                "device-setup-dialog",
                "device-setup-close",
                compact,
                pane,
                cx,
            )
        };
        let Some(setup) = &self.menu.device_setup else {
            return frame(div(), cx);
        };
        if let Step::SavedWarning(warning) = setup.step {
            return frame(self.render_device_saved_warning(warning, cx), cx);
        }
        if matches!(setup.step, Step::Form) && setup.picker.choosing() {
            return frame(self.render_device_picker(cx), cx);
        }
        setup.fields[0].update(cx, |field, cx| field.set_completion(None, cx));
        let mut body = div()
            .id("device-setup-body")
            .debug_selector(|| "device-setup-body".into())
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.));
        if matches!(setup.step, Step::Form) {
            body = body.child(self.render_device_choice_summary(setup, cx));
        }
        let target = setup.fields[0].read(cx).text().trim();
        let named_user = target
            .strip_prefix("ssh://")
            .unwrap_or(target)
            .split_once('@')
            .is_some_and(|(user, _)| !user.is_empty());
        let username_hint = (matches!(setup.step, Step::Form) && !target.is_empty() && !named_user)
            .then(|| {
                fl!(
                    "add-device-username-hint",
                    example = offer::example_login(target),
                )
            });
        for (index, (label, field)) in [
            fl!("add-device-target"),
            fl!("add-device-label"),
            fl!("add-device-session"),
        ]
        .into_iter()
        .zip(&setup.fields)
        .enumerate()
        {
            body = body.child(
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(label)
                    .child(field.clone())
                    .when(index == 0, |field| {
                        field.children(username_hint.as_ref().map(|hint| {
                            div()
                                .id("device-setup-username-hint")
                                .debug_selector(|| "device-setup-username-hint".into())
                                .role(Role::Status)
                                .text_color(rgb(theme.subtext()))
                                .aria_description(fl!("add-device-username-help"))
                                .tooltip({
                                    let (foreground, surface) = (theme.foreground, theme.surface);
                                    move |_, cx| {
                                        cx.new(|_| crate::usage::Hint {
                                            text: fl!("add-device-username-help").into(),
                                            foreground,
                                            surface,
                                        })
                                        .into()
                                    }
                                })
                                .child(hint.clone())
                        }))
                    }),
            );
        }
        body = body.child(self.render_host_key_choice(setup, cx));
        if let Some(error) = &setup.error {
            body = body.child(
                div()
                    .flex_none()
                    .text_color(crate::menu::danger(theme))
                    .child(error.clone()),
            );
        }
        let status = match &setup.step {
            Step::Form => None,
            Step::Checking(request) => Some(fl!("add-device-checking", target = request.target())),
            Step::AwaitingLocalNetwork(request) => Some(
                super::super::local_network::waiting_status(request.target()),
            ),
            Step::Saving(request, HostProbe::Stopped) => {
                Some(fl!("add-device-starting", target = request.target()))
            }
            Step::Saving(request, _) => Some(fl!("add-device-saving", target = request.target())),
            Step::Saved | Step::SavedWarning(_) => None,
            Step::FetchingKey(request) => {
                Some(fl!("add-device-fetching-key", target = request.target()))
            }
            Step::TrustKey(request, key) => {
                body = body.child(self.render_host_key(request.target(), key));
                None
            }
            Step::Confirm(request, offer) => Some(offer.question(
                request.target(),
                setup.claim.as_ref().map(setup::Claim::user),
            )),
            Step::Verifying(request, _) => {
                Some(fl!("add-device-verifying", target = request.target()))
            }
            Step::Activating(..) | Step::Opening(..) => Some(fl!("add-device-opening")),
        };
        if let Some(status) = status {
            body = body.child(
                div()
                    .debug_selector(|| "device-setup-status".into())
                    .flex_none()
                    .text_color(rgb(theme.muted))
                    .child(status),
            );
        }
        let button = |id: &'static str, label: SharedString, enabled: bool| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .role(Role::Button)
                .aria_label(label.clone())
                .when(id == "device-setup-submit" && enabled, |button| {
                    button.track_focus(&setup.submit_focus)
                })
                .focus_visible(|button| {
                    button.shadow(vec![BoxShadow {
                        color: rgb(theme.primary()).into(),
                        offset: point(px(0.), px(0.)),
                        blur_radius: px(0.),
                        spread_radius: px(2.),
                        inset: true,
                    }])
                })
                .p(px(8.))
                .rounded(px(crate::config::corners::CONTROL))
                .bg(rgb(theme.active))
                .when(
                    enabled && matches!(id, "device-setup-submit" | "device-setup-trust"),
                    |button| {
                        button
                            .bg(rgb(theme.primary_wash()))
                            .border_1()
                            .border_color(rgb(theme.primary()))
                            .font_weight(FontWeight::SEMIBOLD)
                    },
                )
                .when(enabled, |button| {
                    button.cursor_pointer().hover(|s| {
                        s.bg(rgb(theme.active).blend(rgba((theme.foreground << 8) | 0x20)))
                    })
                })
                .when(!enabled, |button| button.text_color(rgb(theme.muted)))
                .child(label)
        };
        let mut footer = div()
            .debug_selector(|| "device-setup-footer".into())
            .flex_none()
            .p(px(16.))
            .border_t_1()
            .border_color(rgb(theme.active))
            .flex()
            .justify_end()
            .gap(px(8.));
        let cancel = button("device-setup-cancel", fl!("add-device-cancel").into(), true)
            .on_click(cx.listener(|this, _, _, cx| this.cancel_device_step(cx)));
        footer = match &setup.step {
            Step::Confirm(..) if cfg!(windows) => footer.child(cancel),
            Step::Confirm(_, offer) => footer.child(cancel).child(
                button("device-setup-submit", offer.action().into(), true)
                    .on_click(cx.listener(|this, _, _, cx| this.open_setup_space(cx))),
            ),
            Step::AwaitingLocalNetwork(_) => footer
                .children(self.local_network_settings_button("device-setup-settings", cx))
                .child(cancel),
            // Enter does not trust a key: only this button does.
            Step::TrustKey(..) => footer.child(cancel).child(
                button(
                    "device-setup-trust",
                    fl!("add-device-trust-key").into(),
                    true,
                )
                .on_click(cx.listener(|this, _, _, cx| this.trust_host_key(cx))),
            ),
            step => {
                let ready = matches!(step, Step::Form);
                footer.child(
                    button(
                        "device-setup-submit",
                        if ready {
                            fl!("add-device-submit")
                        } else {
                            fl!("add-device-submit-checking")
                        }
                        .into(),
                        ready,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.submit_device_setup(cx))),
                )
            }
        };
        frame(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(body)
                .child(footer),
            cx,
        )
    }
}
