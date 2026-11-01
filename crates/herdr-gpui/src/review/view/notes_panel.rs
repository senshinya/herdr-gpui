//! The notes beside the diff: the composer for the line being noted or the
//! note being edited, the notes, and Send, Copy, or Clear sent.
use super::{PANEL_SHARE, Review};
use crate::{HerdrWindow, browser::TabId, review::notes::Note};
use gpui::{prelude::*, *};

impl HerdrWindow {
    pub(super) fn render_review_notes(
        &self,
        id: TabId,
        review: &Review,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = &self.theme;
        let button = |id: &'static str, label: SharedString, primary: bool| {
            let background = if primary {
                theme.primary()
            } else {
                theme.active
            };
            div()
                .id(id)
                .debug_selector(move || id.into())
                .px_2()
                .py_1()
                .rounded(px(crate::config::corners::CONTROL))
                .cursor_pointer()
                .bg(rgb(background))
                .text_color(rgb(theme.text_on(background)))
                .child(label)
        };
        let editing = review.editing;
        let drafting = review
            .draft
            .zip(review.loaded())
            .and_then(|(row, loaded)| loaded.diff.anchor(row))
            .and_then(|anchor| Note::new(anchor, "x"))
            .map(|note| note.place())
            .or_else(|| Some(review.notes.get(editing?)?.place()));
        let composer = drafting.map(|place| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .border_b_1()
                .border_color(rgb(theme.active))
                .child(div().text_color(rgb(theme.muted)).truncate().child(place))
                .child(
                    div()
                        .id("review-input")
                        .debug_selector(|| "review-input".into())
                        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                            let composing = this
                                .reviews
                                .get(&id)
                                .is_some_and(|review| review.input.read(cx).is_composing());
                            if composing {
                                return;
                            }
                            match event.keystroke.key.as_str() {
                                "enter" => this.add_review_note(id, window, cx),
                                "escape" => this.cancel_review_note(id, window, cx),
                                _ => return,
                            }
                            cx.stop_propagation();
                        }))
                        .child(review.input.clone()),
                )
                .child(
                    div().flex().child(
                        button(
                            "review-add",
                            if editing.is_some() {
                                "Save note"
                            } else {
                                "Add note"
                            }
                            .into(),
                            true,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| this.add_review_note(id, window, cx),
                        )),
                    ),
                )
        });
        let rows = review.notes.iter().enumerate().map(|(index, note)| {
            div()
                .id(("review-note", index))
                .when(note.sent.is_some(), |row| row.opacity(0.6))
                .when(editing == Some(index), |row| row.bg(rgb(theme.active)))
                .flex()
                .gap_2()
                .p_2()
                .border_b_1()
                .border_color(rgb(theme.active))
                .child(
                    div()
                        .flex_none()
                        .size(px(18.))
                        .rounded_full()
                        .bg(rgb(theme.palette[3]))
                        .text_color(rgb(theme.text_on(theme.palette[3])))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child((index + 1).to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().text_color(rgb(theme.muted)).truncate().child(
                            if note.sent.is_some() {
                                format!("{} \u{b7} sent", note.place())
                            } else {
                                note.place()
                            },
                        ))
                        // Clicking the text edits it.
                        .child(
                            div()
                                .id(("review-edit", index))
                                .debug_selector(move || format!("review-edit-{index}"))
                                .cursor_text()
                                .child(note.comment.clone())
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.edit_review_note(id, index, window, cx)
                                })),
                        ),
                )
                .child(
                    div()
                        .id(("review-remove", index))
                        .flex_none()
                        .size(px(18.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .rounded(px(crate::config::corners::CONTROL))
                        .hover(|s| s.bg(rgb(theme.active)))
                        .child(
                            svg()
                                .path("icons/close.svg")
                                .size(px(12.))
                                .text_color(rgb(theme.muted)),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_review_note(id, index, cx)
                        })),
                )
        });
        let has_notes = !review.notes.is_empty();
        let has_agent = review.agent.is_some();
        let unsent = review
            .notes
            .iter()
            .filter(|note| note.sent.is_none())
            .count();
        let send_label = crate::agent_notes::send_label(review.notes.len(), unsent);
        let any_sent = unsent < review.notes.len();
        let queued = review.sending.queued(
            &self.deliveries,
            cx.try_global::<crate::browser::Feedback>(),
        );
        let panel = div()
            .id("review-notes")
            .debug_selector(|| "review-notes".into())
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(rgb(theme.active))
            .child(
                self.render_review_destination(id, review, cx)
                    .p_2()
                    .border_b_1()
                    .border_color(rgb(theme.active)),
            )
            .children(composer)
            .child(
                div()
                    .id("review-note-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows)
                    .when(!has_notes && review.draft.is_none(), |list| {
                        list.child(
                            div()
                                .p_2()
                                .text_color(rgb(theme.muted))
                                .child("Select a line to add a note."),
                        )
                    }),
            )
            // A queued batch stays withdrawable after Clear sent empties
            // the list.
            .when(has_notes || queued.is_some(), |panel| {
                panel.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .p_2()
                        .border_t_1()
                        .border_color(rgb(theme.active))
                        .when_some(queued, |row, queued| {
                            row.child(
                                div()
                                    .id("review-queued")
                                    .debug_selector(|| "review-queued".into())
                                    .w_full()
                                    .text_color(rgb(theme.muted))
                                    .child(queued.label()),
                            )
                            .child(
                                button("review-withdraw", "Withdraw".into(), true).on_click(
                                    cx.listener(move |this, _, _, cx| this.withdraw_review(id, cx)),
                                ),
                            )
                        })
                        .when(has_agent && queued.is_none(), |row| {
                            row.child(button("review-send", send_label.into(), true).on_click(
                                cx.listener(move |this, _, _, cx| this.send_review(id, cx)),
                            ))
                        })
                        .when(has_notes, |row| {
                            row.child(button("review-copy", "Copy".into(), !has_agent).on_click(
                                cx.listener(move |this, _, _, cx| this.copy_review(id, cx)),
                            ))
                        })
                        .when(any_sent, |row| {
                            row.child(
                                button("review-clear-sent", "Clear sent".into(), false).on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.clear_sent_review_notes(id, cx)
                                    }),
                                ),
                            )
                        }),
                )
            });
        self.resizable_panel(
            panel,
            "review-notes-resize",
            crate::panel_resize::PanelDrag::ReviewNotes,
            Some(PANEL_SHARE),
            cx,
        )
    }
    fn render_review_destination(&self, id: TabId, review: &Review, cx: &App) -> Stateful<Div> {
        let theme = &self.theme;
        let context = cx
            .try_global::<crate::browser::Store>()
            .and_then(|store| store.get(id))
            .map(|tab| {
                let endpoint = self
                    .endpoints
                    .iter()
                    .find(|endpoint| crate::browser::scope(endpoint) == tab.scope);
                let host =
                    endpoint.map_or("Unavailable device", |endpoint| endpoint.label.as_str());
                let workspace = endpoint
                    .and_then(|endpoint| endpoint.live.snapshot.as_ref())
                    .and_then(|snapshot| {
                        snapshot
                            .workspaces
                            .iter()
                            .find(|workspace| workspace.workspace_id == tab.workspace_id)
                    })
                    .map_or(tab.workspace_id.as_str(), |workspace| {
                        workspace.label.as_str()
                    });
                format!(
                    "{} · {}",
                    crate::notifications::safe_text(host, 80),
                    crate::notifications::safe_text(workspace, 80)
                )
            })
            .unwrap_or_else(|| "Unavailable workspace".into());
        let destination = match &review.agent {
            Some(agent) => format!(
                "Notes to {} · {} · {}",
                agent.label,
                context,
                crate::notifications::safe_text(&agent.pane_id, 80)
            ),
            None => "No agent in this workspace; notes can be copied".into(),
        };
        let label = review.agent.as_ref().map_or_else(
            || "No agent in this workspace".to_owned(),
            |agent| format!("To {} · {}", agent.label, context),
        );
        let destination_hint = SharedString::from(destination.clone());
        let (foreground, surface) = (theme.foreground, theme.surface);
        div()
            .id("review-destination")
            .debug_selector(|| "review-destination".into())
            .role(Role::Label)
            .aria_label(destination)
            .min_w_0()
            .max_w_full()
            .truncate()
            .tooltip(move |_, cx| {
                cx.new(|_| crate::usage::Hint {
                    text: destination_hint.clone(),
                    foreground,
                    surface,
                })
                .into()
            })
            .text_color(rgb(theme.muted))
            .child(label)
    }
}
