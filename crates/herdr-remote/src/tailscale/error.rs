use std::{io, path::PathBuf};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("This Tailscale identity is already in use by another Herdr process")]
    Busy,
    #[error("Tailscale connection is closed")]
    Closed,
    #[error(transparent)]
    Credentials(#[from] herdr_credentials::Error),
    #[error(
        "No system credential store is available in this build. Enable 'Allow private file for credentials' in Settings > Tailscale, then connect again."
    )]
    CredentialsUnavailable,
    #[error(
        "Remove inherited TS_AUTHKEY, TS_AUTH_KEY, TS_CLIENT_SECRET, TS_CLIENT_ID, TS_ID_TOKEN and TS_AUDIENCE before using browser login"
    )]
    Environment,
    #[error("Sign out before changing the Tailscale control server or credential store")]
    IdentityChange,
    #[error("Invalid Tailscale configuration")]
    InvalidConfig,
    #[error("Tailscale I/O failed")]
    Io(#[from] io::Error),
    #[error("Unreadable embedded Tailscale response")]
    Json(#[from] serde_json::Error),
    #[error("Tailscale resource limit reached")]
    Limit,
    #[error("Sign out did not finish; retry signing out before connecting")]
    LogoutPending,
    #[error("Embedded Tailscale ABI mismatch")]
    NativeVersion,
    #[error("Tailscale is not connected")]
    NotReady,
    #[error("Cannot access private Tailscale state at {path}")]
    Path {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Tailscale state path is not valid Unicode")]
    PathEncoding,
    #[error("The Tailscale peer is offline, expired, or no longer visible")]
    PeerUnavailable,
    #[error("Tailscale identity storage failed")]
    Storage,
    #[error("Invalid Tailscale identity encoding")]
    StorageEncoding(#[from] base64::DecodeError),
    #[error("Tailscale operation timed out")]
    Timeout,
    #[error("Tailscale background worker is unavailable")]
    Worker,
}
