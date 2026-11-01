//! Embedded network commands, independent of the system Tailscale client.
use super::{Args, CliError, unknown};
pub use herdr_control_protocol::protocol::tailscale::TailscaleOp as TailscaleCommand;
pub(super) fn tailscale(args: &mut Args) -> Result<TailscaleCommand, CliError> {
    let Some(verb) = args.verb() else {
        return Ok(TailscaleCommand::Status);
    };
    Ok(match verb.to_str() {
        Some("status") => TailscaleCommand::Status,
        Some("peers") => TailscaleCommand::Peers {
            offset: args.number("--offset", 0..=1024)?.unwrap_or(0),
        },
        Some("devices") => TailscaleCommand::Devices,
        Some("connect") => TailscaleCommand::Connect,
        Some("disconnect") => TailscaleCommand::Disconnect,
        Some("logout") => TailscaleCommand::Logout,
        _ => return Err(unknown("tailscale", verb)),
    })
}
