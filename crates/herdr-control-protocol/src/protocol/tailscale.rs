//! Public, secret-free state of the application's embedded Tailnet node.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    #[default]
    Disconnected,
    Connecting,
    Disconnecting,
    NeedsLogin,
    NeedsApproval,
    Connected,
    Failed,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Peer {
    pub addresses: Vec<String>,
    pub dns_name: String,
    pub expired: bool,
    pub id: String,
    pub key_expiry: i64,
    pub last_seen: i64,
    pub name: String,
    pub online: bool,
    pub os: String,
}

impl Peer {
    pub fn available(&self) -> bool {
        self.online && !self.expired && !self.id.is_empty() && !self.addresses.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Status {
    pub error: Option<String>,
    pub error_kind: Option<String>,
    pub network: String,
    pub network_id: String,
    pub peers: Vec<Peer>,
    pub self_ips: Vec<String>,
    pub self_name: String,
    pub state: State,
    pub total: usize,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum TailscaleOp {
    Status,
    Peers {
        #[serde(default)]
        offset: usize,
    },
    Connect,
    Disconnect,
    Logout,
    Devices,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reply {
    Status {
        status: Box<Status>,
    },
    Peers {
        peers: Vec<Peer>,
        next: Option<usize>,
        total: usize,
    },
}

/// Snapshot projection bounds the control response independently of the ABI.
pub fn reply(mut status: Status, op: &TailscaleOp) -> Option<Reply> {
    match op {
        TailscaleOp::Status => {
            status.peers.clear();
            Some(Reply::Status {
                status: Box::new(status),
            })
        }
        TailscaleOp::Peers { offset } if *offset <= 1024 => {
            let total = status.peers.len();
            let peers = status
                .peers
                .into_iter()
                .skip(*offset)
                .take(64)
                .collect::<Vec<_>>();
            let end = offset.saturating_add(peers.len());
            Some(Reply::Peers {
                peers,
                next: (end < total).then_some(end),
                total,
            })
        }
        _ => None,
    }
}
#[cfg(test)]
mod tests;
