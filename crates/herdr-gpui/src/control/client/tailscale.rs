use super::{Context, Output, unexpected};
use crate::{
    cli::control::TailscaleCommand,
    control::protocol::{Request, Response, tailscale::Reply},
};
pub(super) fn tailscale(command: TailscaleCommand, context: &Context) -> crate::Result<Output> {
    match context.call(Request::Tailscale, command)? {
        Response::Done { message } => Ok(Output::message(message)),
        Response::Tailscale { reply } => {
            let text = match &reply {
                Reply::Status { status } => format!(
                    "{:?}\nNetwork: {}\nNode: {}\nAddresses: {}",
                    status.state,
                    status.network,
                    status.self_name,
                    status.self_ips.join(", ")
                ),
                Reply::Peers { peers, .. } => peers
                    .iter()
                    .map(|p| {
                        format!(
                            "{}\t{}\t{}\t{}",
                            p.id,
                            p.name,
                            if p.available() {
                                "online"
                            } else {
                                "unavailable"
                            },
                            p.addresses.join(", ")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            let output = Output::data(text, &reply)?;
            Ok(
                if matches!(&reply, Reply::Peers { peers, .. } if peers.is_empty()) {
                    output.nothing()
                } else {
                    output
                },
            )
        }
        response => Err(unexpected(response)),
    }
}
