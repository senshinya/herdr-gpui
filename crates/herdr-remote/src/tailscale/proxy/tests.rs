#![allow(clippy::unwrap_used)]
use super::*;

mod flush;

#[test]
fn oversized_proxy_header_is_refused_before_payload_allocation() {
    let bytes = 100_000_u32.to_be_bytes();
    assert_eq!(
        read_request(&mut bytes.as_slice()).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn unknown_proxy_request_fields_are_rejected() {
    let json =
        br#"{"generation":0,"engine":"a","network":"n","peer":"p","port":22,"command":"sh"}"#;
    let mut bytes = (json.len() as u32).to_be_bytes().to_vec();
    bytes.extend_from_slice(json);
    assert!(read_request(&mut bytes.as_slice()).is_err());
}

#[test]
fn explicit_shutdown_releases_the_private_endpoint_directory() -> Result<()> {
    let manager = Manager::new()?;
    let proxy = Proxy::bind(manager.clone())?;
    let directory = proxy.directory().to_path_buf();
    assert!(directory.is_dir());
    manager.shutdown()?;
    proxy.shutdown()?;
    assert!(!directory.exists());
    Ok(())
}

#[test]
fn a_valid_peer_permission_does_not_authorize_a_stale_network_epoch() {
    let mut request = ProxyRequest {
        generation: 0,
        engine: "a".repeat(64),
        network: "tail.net".into(),
        peer: "node".into(),
        port: 22,
    };
    let allowed = BTreeSet::from([request.clone()]);
    request.generation = 7;
    assert!(authorized(&allowed, &request, 7));
    assert!(!authorized(&allowed, &request, 8));
    request.port = 2222;
    assert!(!authorized(&allowed, &request, 7));
}
