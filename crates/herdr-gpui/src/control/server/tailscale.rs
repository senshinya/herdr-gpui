use super::{in_free_window, socket, window_for};
use crate::control::protocol::{
    Call, ErrorCode, Response,
    tailscale::{State, TailscaleOp, reply},
};
use gpui::App;
pub(super) fn serve(call: Call<TailscaleOp>, incoming: socket::Incoming, cx: &mut App) {
    if let Err(response) = window_for(&call.caller, cx) {
        incoming.respond(response);
        return;
    }
    let status = crate::tailnet::snapshot().status;
    if call.op == TailscaleOp::Devices && status.state != State::Connected {
        incoming.respond(Response::error(
            ErrorCode::Unsupported,
            "Connect to a Tailnet before opening its devices",
        ));
        return;
    }
    if let Some(reply) = reply(status, &call.op) {
        incoming.respond(Response::Tailscale { reply });
        return;
    }
    if matches!(call.op, TailscaleOp::Peers { .. }) {
        incoming.respond(Response::error(
            ErrorCode::InvalidRequest,
            "Peer offset must be at most 1024",
        ));
        return;
    }
    incoming.respond(in_free_window(&call.caller, cx, |view, _, cx| {
        if call.op == TailscaleOp::Devices {
            crate::settings_window::open_tailscale_devices(cx.entity().downgrade(), cx);
            return Response::done("Showing Tailnet devices");
        }
        crate::settings_window::open_tailscale(cx.entity().downgrade(), cx);
        let _ = view;
        Response::done(
            "Showing Tailscale settings; choose Connect, Disconnect or Sign out there to confirm",
        )
    }));
}
