#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid embedded Tailnet device binding")]
    Invalid,
    #[error(
        "This SSH host defines ProxyCommand, ProxyJump, or HostKeyAlias; use a host without conflicting options for the embedded Tailnet"
    )]
    Conflict,
    #[error("Cannot prepare private Tailnet SSH configuration")]
    Io(#[from] std::io::Error),
    #[error("SSH configuration could not be resolved")]
    Resolve,
}
