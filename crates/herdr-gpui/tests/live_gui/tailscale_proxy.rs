//! Headless binary transport regression: no daemon, SSH host, or Tailnet.
use super::*;
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixListener,
    process::Stdio,
    sync::mpsc,
};

const PACKET: [u8; 16] = [0, 0, 0, 12, 4, 20, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51];

fn serve_packet(listener: UnixListener, ready: mpsc::SyncSender<()>) -> io::Result<()> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(io::ErrorKind::TimedOut.into());
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error),
        }
    };
    // BSD accepts inherit the listener's mode; only accepting is polled here.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    stream.set_write_timeout(Some(Duration::from_secs(15)))?;
    let mut prefix = [0; 4];
    stream.read_exact(&mut prefix)?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length > 2048 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut request = vec![0; length];
    stream.read_exact(&mut request)?;
    stream.write_all(&[1])?;
    stream.write_all(&PACKET)?;
    ready.send(()).map_err(io::Error::other)?;
    // The peer stays open and waits for the client, as an SSH handshake does.
    let mut reply = [0];
    stream.read_exact(&mut reply)?;
    if reply != [7] {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(())
}

#[test]
fn tailnet_proxy_forwards_binary_handshake_before_newline_or_eof() {
    let mut isolated = Isolated {
        sandbox: Sandbox::new(),
        daemon: None,
        gui: None,
    };
    let endpoint = isolated.sandbox.dir.join("p");
    let listener = UnixListener::bind(&endpoint).unwrap();
    let mut command = isolated
        .sandbox
        .command(env!("CARGO_BIN_EXE_herdr-gpui"), "proxy.log");
    command
        .arg("tailscale-proxy")
        .arg("--endpoint")
        .arg(&endpoint);
    command.args([
        "--engine",
        &"a".repeat(64),
        "--network-hex",
        "666978747572652d6e6574776f726b",
        "--peer-hex",
        "666978747572652d70656572",
        "--generation",
        "0",
        "--port",
        "22",
    ]);
    isolated.gui = Some(
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let child = isolated.gui.as_mut().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (packet_tx, packet_rx) = mpsc::sync_channel(1);
    let output = thread::spawn(move || {
        let mut packet = [0; PACKET.len()];
        let result = stdout.read_exact(&mut packet).map(|()| packet);
        let _ = packet_tx.send(result);
    });
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let mut server = Some(thread::spawn(move || serve_packet(listener, ready_tx)));
    let ready = ready_rx.recv_timeout(Duration::from_secs(120));
    let packet = packet_rx.recv_timeout(Duration::from_secs(5));
    if packet
        .as_ref()
        .is_ok_and(|result| result.as_ref().is_ok_and(|bytes| bytes == &PACKET))
    {
        child.stdin.as_mut().unwrap().write_all(&[7]).unwrap();
    }
    // Reap only our helper before removing its sandbox, including on failure.
    let server_result = if packet.as_ref().is_ok_and(|result| result.is_ok()) {
        Some(server.take().unwrap().join().unwrap())
    } else {
        None
    };
    stop_children([&mut isolated.gui]);
    output.join().unwrap();
    let server_result = match server_result {
        Some(result) => result,
        None => server.take().unwrap().join().unwrap(),
    };
    let diagnostic = fs::read_to_string(isolated.sandbox.dir.join("proxy.log")).unwrap();
    assert!(
        ready.is_ok(),
        "proxy helper never completed its local handshake"
    );
    assert!(
        packet.as_ref().is_ok_and(|result| result.is_ok()),
        "binary SSH data waited for a newline or EOF: {packet:?}; helper: {diagnostic}; server: {server_result:?}"
    );
    assert_eq!(packet.unwrap().unwrap(), PACKET);
    server_result.unwrap();
}
