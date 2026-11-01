use super::*;
#[test]
fn a_stalled_peer_cannot_hold_a_bounded_writer_after_close() {
    let address = address();
    let (_listener, server, _client) = pair(&address);
    let (_, mut writer, closer) = server.split().unwrap();
    writer.set_timeout(Some(Duration::from_millis(50))).unwrap();
    let deadline = Instant::now() + WAIT;
    loop {
        match writer.write(&[0; 65536]) {
            Ok(written) => {
                assert!(written > 0);
                assert!(Instant::now() < deadline, "pipe did not reach backpressure");
            }
            Err(error) => {
                assert!(matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ));
                break;
            }
        }
    }
    closer.close();
    assert!(writer.write(b"closed").is_err());
}
