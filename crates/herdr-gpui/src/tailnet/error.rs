#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Embedded Tailscale is unavailable")]
    Unavailable,
    #[error(transparent)]
    Native(#[from] herdr_remote::tailscale::Error),
    #[error("Invalid internal Tailscale proxy arguments")]
    ProxyArguments,
}
