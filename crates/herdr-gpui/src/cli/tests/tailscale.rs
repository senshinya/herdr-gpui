use super::*;
use control::{ControlCommand, Invocation, TailscaleCommand};
fn parse(args: &[&str]) -> Result<Invocation, CliError> {
    LaunchOptions::parse(args.iter().copied()).map(|options| match options.mode {
        LaunchMode::Control(invocation) => invocation,
        other => panic!("unexpected mode: {other:?}"),
    })
}
#[test]
fn tailscale_commands_keep_common_targeting_and_bounds() {
    let invocation = parse(&["tailscale", "--json", "--window", "3"]).unwrap();
    assert!(invocation.json);
    assert_eq!(invocation.window, Some(3));
    assert_eq!(
        invocation.command,
        ControlCommand::Tailscale(TailscaleCommand::Status)
    );
    assert_eq!(
        parse(&["tailscale", "peers", "--offset", "64"])
            .unwrap()
            .command,
        ControlCommand::Tailscale(TailscaleCommand::Peers { offset: 64 })
    );
    for verb in ["connect", "disconnect", "logout"] {
        assert!(parse(&["tailscale", verb]).is_ok());
    }
    for args in [
        vec!["tailscale", "peers", "--offset", "1025"],
        vec!["tailscale", "peers", "--offset", "-1"],
        vec!["tailscale", "connect", "--auth-key", "secret"],
        vec!["tailscale", "peers", "--offset", "0", "--offset", "1"],
    ] {
        assert!(parse(&args).is_err());
    }
    assert!(parse(&["tailscale", "--help"]).is_ok());
}
