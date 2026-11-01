use super::*;
#[test]
fn secret_free_status_excludes_peer_payload_and_pages_are_bounded() {
    let status = Status {
        peers: (0..130)
            .map(|i| Peer {
                id: i.to_string(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    assert!(
        matches!(reply(status.clone(), &TailscaleOp::Status), Some(Reply::Status { status }) if status.peers.is_empty())
    );
    assert!(
        matches!(reply(status.clone(), &TailscaleOp::Peers { offset: 64 }), Some(Reply::Peers { peers, next: Some(128), total: 130 }) if peers.len() == 64 && peers[0].id == "64")
    );
    assert!(reply(status, &TailscaleOp::Peers { offset: 1025 }).is_none());
}

#[test]
fn devices_navigation_is_a_secret_free_operation() -> Result<(), serde_json::Error> {
    assert_eq!(
        serde_json::to_string(&TailscaleOp::Devices)?,
        r#"{"op":"devices"}"#
    );
    assert_eq!(
        serde_json::from_str::<TailscaleOp>(r#"{"op":"devices"}"#)?,
        TailscaleOp::Devices
    );
    assert!(reply(Status::default(), &TailscaleOp::Devices).is_none());
    Ok(())
}
