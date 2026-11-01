//! The Add Device dialog: its form, the host check and save that run off the
//! UI thread, and the local workspace that carries a setup needing prompts.

use super::{Page, discover::Discovery, setup};

mod offer;
pub(super) mod picker;
mod render;
pub(super) use picker::Picker;

use crate::i18n::fl;
use crate::{
    HerdrWindow, NavigationTarget,
    search_input::{Changed, SearchInput},
};
use gpui::{prelude::*, *};
use herdr_client::{
    HostKeys, HostProbe, Method, NewHostKey, SshFailure,
    protocol::{ClientKeyCode, ClientKeyKind, ClientPaneInputEvent},
};
pub(super) use offer::Offer;

pub(in crate::menu) struct Setup {
    pub(super) fields: [Entity<SearchInput>; 3],
    pub(super) picker: Picker,
    pub(super) tailnet_peer: Option<crate::tailnet::Selection>,
    pub(super) tailnet_poll: Option<Task<()>>,
    pub(super) step: Step,
    /// This process's claim on the host being added, from the first check
    /// until the device is saved or the dialog closes.
    pub(super) claim: Option<setup::Claim>,
    pub(super) task: Option<Task<()>>,
    /// The search for hosts to suggest, from when the form opens.
    pub(super) discovery: Option<Discovery>,
    /// The label a suggestion filled in, until the user edits it. Only that
    /// label is replaced when another suggestion is chosen.
    pub(super) suggested_label: Option<String>,
    /// Clears `suggested_label` once the label field holds anything else, and
    /// refilters the suggestions as the target changes.
    pub(super) _edits: Vec<Subscription>,
    /// The suggestion Up and Down moved to, by the names and addresses that
    /// identify its machine, with the target text it was chosen for: typing
    /// anything else drops it.
    pub(super) highlight: Option<(String, Vec<String>)>,
    /// Why the last check or save failed. Each source of Add Device keeps
    /// its own, so a job finishing while another source shows stays here.
    pub(super) error: Option<String>,
    pub(super) suggestion_scroll: ScrollHandle,
    /// Whether the host's key is checked; see `host_key_choice`.
    pub(super) host_keys: HostKeys,
    pub(super) host_key_focus: FocusHandle,
    pub(super) submit_focus: FocusHandle,
}

/// Where adding a device stands. Each step after `Form` belongs to the request
/// that was submitted, not to whatever the fields hold now.
pub(super) enum Step {
    Form,
    Checking(setup::Request),
    /// macOS refused the host check the local network; waiting for the user
    /// to allow Herdr, then checking again (see `local_network`).
    AwaitingLocalNetwork(setup::Request),
    /// Herdr is present, so the CLI saves the device without a terminal. A
    /// stopped server is started by that same command.
    Saving(setup::Request, HostProbe),
    /// Saved; the next frame closes the dialog (see `poll_device_setup`).
    Saved,
    /// Already saved; keep the recovery advice readable until dismissed.
    SavedWarning(&'static str),
    /// SSH refused an unknown host key; fetching it for the user to approve.
    FetchingKey(setup::Request),
    /// The user compares the host key's fingerprint before trusting it.
    TrustKey(setup::Request, NewHostKey),
    /// Setup needs prompts, so the user decides whether to run it locally.
    Confirm(setup::Request, Offer),
    /// Checking the catalog on disk again before opening the setup workspace
    /// for the offer the user accepted.
    Verifying(setup::Request, Offer),
    /// Waiting for the window to show this computer's Herdr, whose surface
    /// must be active before it creates a workspace (see `create_setup_space`).
    Activating(setup::Request, setup::TerminalSetup, Offer),
    /// Waiting for the local daemon to create the setup workspace.
    Opening(setup::Request, LocalSpace),
}

/// The local workspace request whose root pane will run the setup command.
pub(super) struct LocalSpace {
    pub(super) request: String,
    pub(super) boot: String,
    /// The relay command the workspace's shell runs, and the setup it shows.
    pub(super) setup: setup::TerminalSetup,
    /// What the user accepted, so a retry after a failure runs it again.
    pub(super) offer: Offer,
}

impl HerdrWindow {
    /// Set the SSH form's error, which stays with that form.
    pub(super) fn set_device_error(&mut self, error: Option<String>) {
        if let Some(setup) = &mut self.menu.device_setup {
            setup.error = error;
        }
    }

    /// Show an empty Add Device form with the target field focused.
    pub(in crate::menu) fn open_add_device(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let fields = std::array::from_fn(|index| {
            let input = cx.new(SearchInput::new);
            input.update(cx, |input, cx| {
                input.set_appearance(self.config.ui.clone(), self.theme.clone(), cx);
                match index {
                    0 => input.set_placeholder_with(|| fl!("add-device-target-placeholder"), cx),
                    1 => input.set_placeholder_with(|| fl!("add-device-label-placeholder"), cx),
                    // Herdr's default session's name.
                    _ => input.set_placeholder("default", cx), // i18n: data
                }
            });
            input
        });
        window.focus(&fields[0].read(cx).focus.clone(), cx);
        let label_edits = cx.subscribe(&fields[1], |this, field, _: &Changed, cx| {
            if let Some(setup) = &mut this.menu.device_setup
                && setup.suggested_label.as_deref() != Some(field.read(cx).text())
            {
                setup.suggested_label = None;
            }
        });
        // An offer runs for the target that was checked, so changing the
        // target withdraws it: the next submit checks the new target.
        let target_edits = cx.subscribe(&fields[0], |this, field, _: &Changed, cx| {
            if let Some(setup) = &mut this.menu.device_setup
                && let Step::Confirm(request, _) = &setup.step
                && request.target() != field.read(cx).text().trim()
            {
                setup.step = Step::Form;
                setup.claim = None;
                setup.error = None;
            }
            cx.notify();
        });
        let picker = Picker::new(&self.config.ui, &self.theme, cx);
        let search_changed = cx.subscribe(picker.input(), |this, _, _: &Changed, cx| {
            if let Some(setup) = &mut this.menu.device_setup {
                setup.highlight = None;
                setup.picker.highlighted_peer = None;
                setup.suggestion_scroll.set_offset(Point::default());
            }
            cx.notify();
        });
        window.focus(&picker.input().read(cx).focus.clone(), cx);
        self.menu.device_setup = Some(Setup {
            picker,
            fields,
            tailnet_peer: None,
            tailnet_poll: None,
            step: Step::Form,
            claim: None,
            task: None,
            discovery: None,
            suggested_label: None,
            _edits: vec![label_edits, target_edits, search_changed],
            highlight: None,
            suggestion_scroll: ScrollHandle::new(),
            error: None,
            host_keys: HostKeys::Verify,
            host_key_focus: cx.focus_handle(),
            submit_focus: cx.focus_handle(),
        });
        self.menu.page = Some(Page::AddDevice);
        self.start_device_discovery(cx);
        self.start_tailnet_devices(cx);
    }

    /// Route a key to the Add Device form. Returns `false` when the key is
    /// left to propagate: during composition, or when the form ignores it.
    pub(super) fn add_device_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(form) = &self.menu.device_setup else {
            return false;
        };
        if matches!(form.step, Step::SavedWarning(_)) {
            if matches!(event.keystroke.key.as_str(), "enter" | "escape") {
                self.dismiss_menu(window, cx);
            }
            // The completed form has no editable fields or further setup action.
            return true;
        }
        if form.picker.choosing() {
            return self.device_picker_key(event, window, cx);
        }
        if form
            .fields
            .iter()
            .any(|field| field.read(cx).is_composing())
        {
            return false;
        }
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.platform || modifiers.alt {
            return false;
        }
        if matches!(form.step, Step::Form)
            && form.host_key_focus.is_focused(window)
            && matches!(key, "enter" | "space")
        {
            self.toggle_host_key_choice(cx);
            return true;
        }
        if matches!(form.step, Step::Form) && form.submit_focus.is_focused(window) && key == "space"
        {
            self.submit_device_setup(cx);
            return true;
        }
        if matches!(form.step, Step::Form)
            && form.picker.back_focus.is_focused(window)
            && matches!(key, "enter" | "space")
        {
            self.show_device_picker(window, cx);
            return true;
        }
        let Some(form) = &self.menu.device_setup else {
            return false;
        };
        match key {
            "tab" => {
                let mut focus = Vec::new();
                if matches!(form.step, Step::Form) {
                    focus.push(form.picker.back_focus.clone());
                }
                focus.extend(form.fields.iter().map(|field| field.read(cx).focus.clone()));
                if matches!(form.step, Step::Form) {
                    focus.push(form.host_key_focus.clone());
                    focus.push(form.submit_focus.clone());
                }
                let index = focus
                    .iter()
                    .position(|focus| focus.is_focused(window))
                    .unwrap_or(0);
                let next =
                    (index + if modifiers.shift { focus.len() - 1 } else { 1 }) % focus.len();
                window.focus(&focus[next], cx);
            }
            "enter" => self.submit_device_setup(cx),
            "escape" => self.dismiss_menu(window, cx),
            _ => return false,
        }
        true
    }

    /// Refuse a host already in the catalog. Checked again before each step
    /// that saves, since another window or the CLI can add it meanwhile. A
    /// device is matched by its saved entry: the sessions list may have attached
    /// it to another session, and that does not add one to the catalog.
    pub(super) fn ensure_new_device(&self, request: &setup::Request) -> crate::Result<()> {
        match self.endpoints.iter().find(|endpoint| {
            endpoint
                .saved_ssh()
                .is_some_and(|(target, session)| request.same_host(target, session))
        }) {
            Some(endpoint) => Err(super::Error::Exists(endpoint.label.clone()).into()),
            None => Ok(()),
        }
    }

    fn submit_device_setup(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &mut self.menu.device_setup else {
            return;
        };
        match &form.step {
            Step::Form => {}
            // Windows offers no terminal setup; the question says what to run.
            Step::Confirm(..) if cfg!(windows) => return,
            Step::Confirm(..) => return self.open_setup_space(cx),
            _ => return,
        }
        let request = setup::Request::new(
            form.fields[0].read(cx).text(),
            form.fields[1].read(cx).text(),
            form.fields[2].read(cx).text(),
        )
        .map(|request| {
            request
                .with_host_keys(form.host_keys)
                .with_tailnet(form.tailnet_peer.clone())
        });
        let request = match request.and_then(|request| {
            self.ensure_new_device(&request)?;
            Ok(request)
        }) {
            Ok(request) => request,
            Err(error) => {
                tracing::info!(category = "device_setup", "Device form refused");
                self.set_device_error(Some(error.to_string()));
                cx.notify();
                return;
            }
        };
        let Some(form) = &mut self.menu.device_setup else {
            return;
        };
        tracing::info!(category = "device_setup", "Checking device");
        form.step = Step::Checking(request.clone());
        form.error = None;
        // Claim before probing: a second add of this host, from any window of
        // this process, is refused from here on rather than after a slow probe.
        let background = cx.background_executor().spawn(async move {
            let mut request = request;
            let result = request.prepare_tailnet().and_then(|()| {
                let claim = setup::claim(&request)?;
                Ok((claim, request.probe()))
            });
            (request, result)
        });
        form.task = Some(cx.spawn(async move |this, cx| {
            let (request, result) = background.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(form) = &mut this.menu.device_setup {
                    form.step = Step::Checking(request);
                }
                this.device_probed(result, cx);
            });
        }));
        cx.notify();
    }

    /// Return to the form when the host is already saved or being added:
    /// offering a terminal would only add it again.
    fn refuse_duplicate(&mut self, error: &crate::Error, cx: &mut Context<Self>) -> bool {
        if !matches!(
            error,
            crate::Error::Devices(super::Error::Exists(_) | super::Error::Adding)
        ) {
            return false;
        }
        if let Some(form) = &mut self.menu.device_setup {
            form.step = Step::Form;
            form.claim = None;
        }
        self.set_device_error(Some(error.to_string()));
        cx.notify();
        true
    }

    pub(super) fn device_probed(
        &mut self,
        result: crate::Result<(setup::Claim, crate::Result<HostProbe>)>,
        cx: &mut Context<Self>,
    ) {
        let (request, waited) = match &self.menu.device_setup {
            Some(Setup {
                step: Step::Checking(request),
                ..
            }) => (request.clone(), false),
            Some(Setup {
                step: Step::AwaitingLocalNetwork(request),
                ..
            }) => (request.clone(), true),
            _ => return,
        };
        let (claim, probe) = match result {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(category = "device_setup", "Could not claim device");
                if !self.refuse_duplicate(&error, cx)
                    && !self.require_prepared_tailnet(&request, cx)
                {
                    self.set_device_error(Some(fl!(
                        "add-device-check-failed",
                        target = request.target(),
                        error = error.to_string(),
                    )));
                    if let Some(form) = &mut self.menu.device_setup {
                        form.step = Step::Confirm(request, Offer::Terminal);
                    }
                    cx.notify();
                }
                return;
            }
        };
        match &probe {
            Ok(probe) => tracing::info!(
                category = "device_setup",
                probe = ?probe,
                "Device probed"
            ),
            Err(_) => tracing::warn!(category = "device_setup", "Device probe failed"),
        }
        // An unknown key can be approved here; a changed one is refused while
        // fetching it, and the terminal is offered instead.
        if let Ok(HostProbe::SshFailed(SshFailure::HostKey)) = probe {
            return self.fetch_host_key(request, claim, cx);
        }
        // A terminal cannot help: the permission belongs to this app. The
        // first refusal can be the system still asking, so wait for the user;
        // a refusal after the wait returns to the form, and the claim drops.
        if let Ok(HostProbe::SshFailed(failure @ SshFailure::LocalNetworkDenied)) = probe {
            if !waited {
                return self.await_local_network(request, claim, cx);
            }
            self.set_device_error(Some(failure.to_string()));
            if let Some(form) = &mut self.menu.device_setup {
                form.step = Step::Form;
            }
            cx.notify();
            return;
        }
        if request.is_tailnet() && !matches!(probe, Ok(HostProbe::Running | HostProbe::Stopped)) {
            self.require_prepared_tailnet(&request, cx);
            return;
        }
        let offer = match probe {
            Ok(probe) => match Offer::for_probe(probe) {
                None => return self.save_device(request, probe, claim, cx),
                Some(offer) => {
                    // Say what SSH refused, since the terminal may not need to.
                    // The key offer already says it, without the advice
                    // about agents that does not apply to a new host.
                    if let HostProbe::SshFailed(failure) = probe
                        && offer != Offer::InstallKey
                    {
                        self.set_device_error(Some(fl!(
                            "add-device-ssh-failed",
                            target = request.target(),
                            error = failure.to_string(),
                        )));
                    }
                    offer
                }
            },
            Err(error) => {
                self.set_device_error(Some(fl!(
                    "add-device-check-failed",
                    target = request.target(),
                    error = error.to_string(),
                )));
                Offer::Terminal
            }
        };
        if let Some(form) = &mut self.menu.device_setup {
            form.step = Step::Confirm(request, offer);
            form.claim = Some(claim);
        }
        cx.notify();
    }

    fn save_device(
        &mut self,
        request: setup::Request,
        probe: HostProbe,
        claim: setup::Claim,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = &mut self.menu.device_setup else {
            return;
        };
        form.step = Step::Saving(request.clone(), probe);
        // The claim travels with the save and comes back, so a failure that
        // offers a terminal still holds the host.
        let background = cx.background_executor().spawn(async move {
            let result = setup::save(&request, &claim);
            (result, claim)
        });
        form.task = Some(cx.spawn(async move |this, cx| {
            let (result, claim) = background.await;
            let _ = this.update(cx, |this, cx| {
                let Some(Setup {
                    step: Step::Saving(request, _),
                    ..
                }) = &this.menu.device_setup
                else {
                    return;
                };
                let request = request.clone();
                match &result {
                    Ok(_) => tracing::info!(category = "device_setup", "Device saved"),
                    Err(_) => tracing::warn!(category = "device_setup", "Device save failed"),
                }
                let error = match result {
                    Ok(outcome) => {
                        this.device_save_succeeded(outcome, cx);
                        return;
                    }
                    Err(error) => error,
                };
                if this.refuse_duplicate(&error, cx) {
                    return;
                }
                this.set_device_error(Some(error.to_string()));
                let offer = match error {
                    crate::Error::Devices(super::Error::ServerRestart) => Offer::Restart,
                    _ => Offer::Terminal,
                };
                if let Some(form) = &mut this.menu.device_setup {
                    form.step = Step::Confirm(request, offer);
                    form.claim = Some(claim);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Back to the form, giving up the host: the next submit, for this host or
    /// another, claims it again.
    pub(super) fn cancel_device_step(&mut self, cx: &mut Context<Self>) {
        if let Some(setup) = &mut self.menu.device_setup {
            setup.step = Step::Form;
            setup.claim = None;
            // Ends a wait for Local Network permission, which holds a claim.
            setup.task = None;
            setup.error = None;
            cx.notify();
        }
    }

    // The external setup terminal cannot inherit this process's network.
    fn require_prepared_tailnet(
        &mut self,
        request: &setup::Request,
        cx: &mut Context<Self>,
    ) -> bool {
        if !request.is_tailnet() {
            return false;
        }
        if let Some(form) = &mut self.menu.device_setup {
            form.step = Step::Form;
            form.claim = None;
        }
        self.set_device_error(Some(fl!("tailnet-setup-required")));
        cx.notify();
        true
    }

    /// Check the catalog on disk once more, then ask the local daemon for a
    /// workspace whose root pane runs the setup (see `poll_device_setup`).
    fn open_setup_space(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &mut self.menu.device_setup else {
            return;
        };
        let Step::Confirm(request, offer) = &form.step else {
            return;
        };
        let (request, offer) = (request.clone(), *offer);
        if self.require_prepared_tailnet(&request, cx) {
            return;
        }
        let Some(form) = &mut self.menu.device_setup else {
            return;
        };
        let claim = form.claim.take();
        form.step = Step::Verifying(request.clone(), offer);
        form.error = None;
        let background = cx.background_executor().spawn(async move {
            // A failed probe left no claim; take one now.
            let claim = match claim {
                Some(claim) => setup::verify_unsaved(&request, &claim).map(|()| claim),
                None => setup::claim(&request),
            }?;
            // The setup runs here, with the user's login environment, which
            // can block the first time it is read.
            let terminal = setup::terminal_setup(&request, offer == Offer::InstallKey);
            Ok::<_, crate::Error>((claim, terminal))
        });
        form.task = Some(cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |this, cx| this.setup_space_verified(result, cx));
        }));
        cx.notify();
    }

    pub(super) fn setup_space_verified(
        &mut self,
        result: crate::Result<(setup::Claim, crate::Result<setup::TerminalSetup>)>,
        cx: &mut Context<Self>,
    ) {
        let Some(Setup {
            step: Step::Verifying(request, offer),
            ..
        }) = &self.menu.device_setup
        else {
            return;
        };
        let (request, offer) = (request.clone(), *offer);
        let (claim, terminal) = match result {
            Ok(verified) => verified,
            Err(error) => {
                tracing::warn!(
                    category = "device_setup",
                    "Device setup verification failed"
                );
                if !self.refuse_duplicate(&error, cx) {
                    self.set_device_error(Some(fl!(
                        "add-device-open-failed",
                        error = error.to_string()
                    )));
                    if let Some(form) = &mut self.menu.device_setup {
                        form.step = Step::Confirm(request, offer);
                    }
                    cx.notify();
                }
                return;
            }
        };
        let Some(form) = &mut self.menu.device_setup else {
            return;
        };
        form.claim = Some(claim);
        let setup = match terminal {
            Ok(setup) => setup,
            Err(error) => return self.fail_setup_space(request, offer, &error, cx),
        };
        // Herdr refuses commands from a connection whose surface is inactive,
        // as Local's is while the window shows another device, so the window
        // moves to Local first; it ends there anyway once the setup opens.
        // Switching closes every dialog, so this one is put back over Local.
        let (page, form) = (self.menu.page, self.menu.device_setup.take());
        let selected = self.select_endpoint(crate::endpoint::LOCAL, cx);
        (self.menu.page, self.menu.device_setup) = (page, form);
        if !selected {
            return self.fail_setup_space(request, offer, &crate::Error::NotConnected, cx);
        }
        if let Some(form) = &mut self.menu.device_setup {
            form.step = Step::Activating(request, setup, offer);
        }
        self.create_setup_space(cx);
    }

    /// Asks the local daemon for the setup workspace once the window has
    /// queued Local's surface activation. The request follows it on the same
    /// ordered connection, so Herdr handles it with the surface active.
    pub(super) fn create_setup_space(&mut self, cx: &mut Context<Self>) {
        let Some(Setup {
            step: Step::Activating(..),
            ..
        }) = &self.menu.device_setup
        else {
            return;
        };
        let local = &self.endpoints[0];
        let connected = local.live.status.is_connected();
        if self.selected_endpoint == 0 && connected && !local.surface_requested() {
            // The next poll activates the surface.
            return;
        }
        let result = (|| {
            if self.selected_endpoint != 0 {
                return Err(crate::Error::StaleConnection);
            }
            let boot = local
                .live
                .snapshot
                .as_ref()
                .filter(|_| connected)
                .map(|snapshot| snapshot.boot_id.clone())
                .ok_or(crate::Error::NotConnected)?;
            let Some(Setup {
                step: Step::Activating(request, ..),
                ..
            }) = &self.menu.device_setup
            else {
                return Err(crate::Error::StaleConnection);
            };
            local
                .connection
                .request_dialog(
                    &boot,
                    Method::WorkspaceCreate,
                    serde_json::json!({
                        "focus": true,
                        // Written into the daemon, which every client
                        // shows, so it is not in this window's language.
                        "label": format!("Set up {}", request.label()), // i18n: data
                    }),
                )
                .map(|id| (id, boot))
        })();
        let Some(form) = &mut self.menu.device_setup else {
            return;
        };
        let Step::Activating(request, setup, offer) = std::mem::replace(&mut form.step, Step::Form)
        else {
            return;
        };
        match result {
            Ok((id, boot)) => {
                form.step = Step::Opening(
                    request,
                    LocalSpace {
                        request: id,
                        boot,
                        setup,
                        offer,
                    },
                );
                cx.notify();
            }
            Err(error) => self.fail_setup_space(request, offer, &error, cx),
        }
    }

    /// Back to the question, keeping the claim for a retry.
    fn fail_setup_space(
        &mut self,
        request: setup::Request,
        offer: Offer,
        error: &crate::Error,
        cx: &mut Context<Self>,
    ) {
        tracing::warn!(category = "device_setup", "Setup workspace failed");
        if let Some(form) = &mut self.menu.device_setup {
            form.step = Step::Confirm(request, offer);
        }
        self.set_device_error(Some(fl!(
            "add-device-open-failed",
            error = error.to_string()
        )));
        cx.notify();
    }

    /// Runs the setup command in the workspace the local daemon created, then
    /// shows it. The command is typed into the pane's shell: the endpoint API
    /// has no method that starts a command in a pane.
    pub(crate) fn poll_device_setup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.poll_saved_device(window, cx) {
            return;
        }
        self.create_setup_space(cx);
        let Some(Setup {
            step: Step::Opening(_, space),
            ..
        }) = &self.menu.device_setup
        else {
            return;
        };
        let local = &self.endpoints[0];
        let fail = |this: &mut Self, error: String, cx: &mut Context<Self>| {
            tracing::warn!(category = "device_setup", "Setup workspace failed");
            if let Some(form) = &mut this.menu.device_setup
                && let Step::Opening(request, space) = &form.step
            {
                form.step = Step::Confirm(request.clone(), space.offer);
            }
            this.set_device_error(Some(fl!("add-device-open-failed", error = error)));
            cx.notify();
        };
        let current = local.live.status.is_connected()
            && local
                .live
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.boot_id == space.boot);
        if !current {
            return fail(self, crate::Error::StaleConnection.to_string(), cx);
        }
        let Some((id, Some(result))) = &local.live.dialog_response else {
            return;
        };
        if id != &space.request {
            return;
        }
        let response = match result {
            Ok(response) => response,
            Err(error) => return fail(self, error.to_string(), cx),
        };
        if let Some(error) = response.get("error") {
            let (code, message) = crate::menu::endpoint_error(error);
            return fail(self, format!("{code}: {message}"), cx);
        }
        let result = &response["result"];
        let created = (result["type"] == "workspace_created")
            .then(|| {
                Some((
                    result["workspace"]["workspace_id"].as_str()?,
                    result["root_pane"]["pane_id"].as_str()?,
                ))
            })
            .flatten()
            .filter(|(workspace, pane)| !workspace.is_empty() && !pane.is_empty());
        let Some((workspace, pane)) = created else {
            return fail(self, fl!("add-device-unexpected-response"), cx);
        };
        let (workspace, pane) = (workspace.to_owned(), pane.to_owned());
        let sent = local
            .connection
            .handle
            .as_ref()
            .ok_or(crate::Error::NotConnected)
            .and_then(|handle| {
                Ok(handle.send_input(
                    &space.boot,
                    &pane,
                    [
                        ClientPaneInputEvent::TextCommit(space.setup.command.clone()),
                        enter(),
                    ],
                )?)
            });
        if let Err(error) = sent {
            return fail(self, error.to_string(), cx);
        }
        // The setup outlives this dialog: its relay keeps waiting, and keeps
        // the host claimed until the setup ends, however it ends, so this
        // process cannot start a second add meanwhile, nor stay locked out.
        if let Some(form) = self.menu.device_setup.as_mut()
            && let Step::Opening(_, space) = &mut form.step
            && let Some(claim) = form.claim.take()
        {
            space.setup.relay.keep(claim);
        }
        let local = local.id.clone();
        self.dismiss_menu(window, cx);
        self.navigate_endpoint(&local, NavigationTarget::Workspace(&workspace), cx);
    }
}

/// The Enter key, which submits a line typed into a pane's shell.
pub(crate) fn enter() -> ClientPaneInputEvent {
    ClientPaneInputEvent::Key {
        code: ClientKeyCode::Enter,
        modifiers: 0,
        kind: ClientKeyKind::Press,
        repeat_count: 1,
        shifted_codepoint: None,
        generated_text: None,
        tracks_release: false,
        physical_key_id: None,
        windows_record: None,
    }
}
