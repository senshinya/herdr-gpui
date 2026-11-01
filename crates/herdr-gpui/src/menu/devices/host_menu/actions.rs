//! Actions offered by each kind of device, in menu order.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    NewWorkspace,
    Rename,
    /// Toggles `[devices.<id>] keybindings` between local and server.
    ServerKeybindings,
    /// Forwards one of the host's listening ports to this computer.
    #[cfg(unix)]
    ForwardPort,
    /// Picks `[devices.<id>] theme`, the terminal's colors on this device.
    Theme,
    /// Opens Settings > Cloud devices on the device's provider.
    #[cfg(feature = "cloud")]
    ProviderSettings,
    /// Runs the configured setup script on the cloud device again.
    #[cfg(feature = "cloud")]
    RunSetup,
    Remove,
}

/// Each row's action and the message naming it.
pub(super) const SSH_ACTIONS: &[(Action, &str)] = &[
    (Action::Rename, "host-menu-rename"),
    (Action::ServerKeybindings, "host-menu-server-keybindings"),
    // Port forwards need an `ssh` control master, which Windows OpenSSH lacks.
    #[cfg(unix)]
    (Action::ForwardPort, "host-menu-forward-port"),
    (Action::Theme, "host-menu-theme"),
    (Action::NewWorkspace, "sidebar-new-workspace"),
    (Action::Remove, "host-menu-remove"),
];

/// Port forwards, server keybindings, and the terminal theme go through the
/// SSH profile, which a cloud device does not have.
#[cfg(feature = "cloud")]
pub(super) const CLOUD_ACTIONS: &[(Action, &str)] = &[
    (Action::Rename, "host-menu-rename"),
    (Action::ProviderSettings, "host-menu-cloud-settings"),
    (Action::RunSetup, "host-menu-run-setup"),
    (Action::NewWorkspace, "sidebar-new-workspace"),
    (Action::Remove, "host-menu-remove"),
];

pub(super) const LOCAL_ACTIONS: &[(Action, &str)] =
    &[(Action::NewWorkspace, "sidebar-new-workspace")];

pub(super) const WSL_ACTIONS: &[(Action, &str)] = &[
    (Action::NewWorkspace, "sidebar-new-workspace"),
    (Action::Remove, "host-menu-remove"),
];
