//! In-process Tailscale. Blocking operations belong on background workers.
mod credentials;
mod engine;
mod error;
#[allow(unsafe_code)]
mod ffi;
pub mod proxy;
mod worker;

pub use engine::Engine;
pub use error::{Error, Result};
pub use herdr_control_protocol::protocol::tailscale::{Peer, State, Status};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
pub use worker::Manager;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub allow_plaintext_credentials: bool,
    pub control_url: String,
    pub enabled: bool,
    pub hostname: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            allow_plaintext_credentials: false,
            control_url: String::new(),
            enabled: false,
            hostname: "herdr-ui".into(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.hostname.is_empty()
            || self.hostname.len() > 63
            || self.hostname.starts_with('-')
            || self.hostname.ends_with('-')
            || !self
                .hostname
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(Error::InvalidConfig);
        }
        if self.control_url.len() > 4096 {
            return Err(Error::InvalidConfig);
        }
        if !self.control_url.is_empty() {
            let url = url::Url::parse(&self.control_url).map_err(|_| Error::InvalidConfig)?;
            if url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
                || url.query().is_some()
            {
                return Err(Error::InvalidConfig);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Snapshot {
    #[serde(skip)]
    pub generation: u64,
    #[serde(default)]
    pub auth_url: Option<SecretString>,
    #[serde(flatten)]
    pub status: Status,
}

#[cfg(test)]
mod tests;
