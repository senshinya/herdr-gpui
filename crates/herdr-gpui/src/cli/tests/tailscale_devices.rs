use super::*;

#[test]
fn tailnet_devices_view_keeps_window_targeting() {
    let parsed = LaunchOptions::parse(["tailscale", "devices", "--json", "--window", "3"]).unwrap();
    let LaunchMode::Control(invocation) = parsed.mode else {
        panic!("expected control mode");
    };
    assert!(invocation.json);
    assert_eq!(invocation.window, Some(3));
    assert_eq!(
        invocation.command,
        control::ControlCommand::Tailscale(control::TailscaleCommand::Devices)
    );
}
