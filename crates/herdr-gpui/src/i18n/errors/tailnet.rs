//! Embedded Tailnet failures, preserving each layer's typed error and redaction.
use super::Localize;
use crate::i18n::fl;

impl Localize for herdr_client::tailnet::Error {
    fn localize(&self) -> String {
        match self {
            Self::Invalid => fl!("error-tailnet-client-invalid"),
            Self::Conflict => fl!("error-tailnet-client-conflict"),
            Self::Io(_) => fl!("error-tailnet-client-io"),
            Self::Resolve => fl!("error-tailnet-client-resolve"),
        }
    }
}

impl Localize for herdr_remote::tailscale::Error {
    fn localize(&self) -> String {
        match self {
            Self::Busy => fl!("error-tailnet-busy"),
            Self::Closed => fl!("error-tailnet-closed"),
            Self::CredentialsUnavailable => fl!("error-tailnet-credentials-unavailable"),
            Self::Environment => fl!("error-tailnet-environment"),
            Self::IdentityChange => fl!("error-tailnet-identity-change"),
            Self::InvalidConfig => fl!("error-tailnet-invalid-config"),
            Self::Io(_) => fl!("error-tailnet-io"),
            Self::Json(_) => fl!("error-tailnet-json"),
            Self::Limit => fl!("error-tailnet-limit"),
            Self::LogoutPending => fl!("error-tailnet-logout-pending"),
            Self::NativeVersion => fl!("error-tailnet-native-version"),
            Self::NotReady => fl!("error-tailnet-not-ready"),
            Self::PathEncoding => fl!("error-tailnet-path-encoding"),
            Self::PeerUnavailable => fl!("error-tailnet-peer-unavailable"),
            Self::Storage => fl!("error-tailnet-storage"),
            Self::StorageEncoding(_) => fl!("error-tailnet-storage-encoding"),
            Self::Timeout => fl!("error-tailnet-timeout"),
            Self::Worker => fl!("error-tailnet-worker"),
            Self::Credentials(error) => error.localize(),
            Self::Path { path, .. } => fl!("error-tailnet-path", path = path.display().to_string()),
        }
    }
}

impl Localize for crate::tailnet::Error {
    fn localize(&self) -> String {
        match self {
            Self::Unavailable => fl!("error-tailnet-unavailable"),
            Self::ProxyArguments => fl!("error-tailnet-proxy-arguments"),
            Self::Native(error) => error.localize(),
        }
    }
}
