use super::*;
use std::io;

#[test]
fn embedded_ssh_errors_are_localized_without_exposing_io_details() {
    use herdr_client::tailnet::Error;
    assert_localized(&[
        Error::Invalid,
        Error::Conflict,
        Error::Io(io::Error::other("private diagnostic")),
        Error::Resolve,
    ]);
    assert_localized(&[herdr_client::Error::Tailnet(Error::Invalid)]);
}

#[test]
fn embedded_network_errors_keep_their_diagnostics_and_redaction() {
    use base64::Engine as _;
    use herdr_remote::tailscale::Error;
    assert_localized(&[
        Error::Busy,
        Error::Closed,
        Error::Credentials(herdr_credentials::Error::Permissions),
        Error::CredentialsUnavailable,
        Error::Environment,
        Error::IdentityChange,
        Error::InvalidConfig,
        Error::Io(io::Error::other("private diagnostic")),
        Error::Json(serde_json::from_str::<u8>("bad").unwrap_err()),
        Error::Limit,
        Error::LogoutPending,
        Error::NativeVersion,
        Error::NotReady,
        Error::Path {
            path: "/tmp/tailnet-state".into(),
            source: io::Error::other("private diagnostic"),
        },
        Error::PathEncoding,
        Error::PeerUnavailable,
        Error::Storage,
        Error::StorageEncoding(
            base64::engine::general_purpose::STANDARD
                .decode("!")
                .unwrap_err(),
        ),
        Error::Timeout,
        Error::Worker,
    ]);
    assert_localized(&[herdr_remote::Error::Tailscale(Error::NotReady)]);
}

#[test]
fn app_tailnet_errors_delegate_to_the_feature_mapping() {
    use crate::tailnet::Error;
    assert_localized(&[
        Error::Unavailable,
        Error::Native(herdr_remote::tailscale::Error::NotReady),
        Error::ProxyArguments,
    ]);
    assert_localized(&[crate::Error::Tailnet(Error::Unavailable)]);
}
