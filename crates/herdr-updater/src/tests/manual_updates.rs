use super::*;

#[test]
fn a_manual_offer_notifies_without_authorizing_download_or_installation() {
    let (sender, receiver) = mpsc::sync_channel(2);
    let mut updater = Updater::default();
    publish(
        &updater.mailbox,
        updater.generation,
        State::Manual {
            version: "20261028.1-shinya.2".into(),
            notes: "New release".into(),
        },
        None,
    );
    assert!(updater.poll());
    assert!(updater.update_available());
    assert_eq!(updater.latest_version(), Some("20261028.1-shinya.2"));
    updater.commands = Some(sender);
    updater.download();
    updater.install();
    updater.upgrade();
    updater.restart();
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert!(matches!(updater.state(), State::Manual { .. }));
}
