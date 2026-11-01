//! Device-scoped creation waits for the selected connection's surface barrier.
use crate::{HerdrWindow, controls::Command};
use gpui::{Context, Window};

impl HerdrWindow {
    pub(crate) fn new_workspace_on_device(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_menu(window, cx);
        if !self.select_endpoint(id, cx) {
            return;
        }
        self.menu.device_workspace = Some((
            self.selection_epoch,
            self.endpoints[self.selected_endpoint].generation,
        ));
        self.poll_device_workspace(window, cx);
    }

    pub(crate) fn poll_device_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.menu.device_workspace else {
            return;
        };
        if target
            != (
                self.selection_epoch,
                self.endpoints[self.selected_endpoint].generation,
            )
            || self.menu.page.is_some()
        {
            self.menu.device_workspace = None;
            return;
        }
        if !self.input_ready() || self.activation_deadline.is_some() {
            return;
        }
        self.menu.device_workspace = None;
        self.command(Command::Workspace, window, cx);
    }
}
