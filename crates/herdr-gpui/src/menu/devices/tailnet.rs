//! Tailnet peers remain separate from local discovery and retain stable IDs.
use super::add_device::{Setup, Step};
use crate::{HerdrWindow, i18n::fl};
use gpui::{prelude::*, *};
use herdr_remote::tailscale::{Peer, Snapshot, State};

fn target(peer: &Peer) -> Option<String> {
    if let Some(ip) = peer
        .addresses
        .iter()
        .find_map(|address| address.parse::<std::net::Ipv4Addr>().ok())
    {
        return Some(ip.to_string());
    }
    let ip = peer
        .addresses
        .iter()
        .find_map(|address| address.parse::<std::net::Ipv6Addr>().ok())?;
    Some(format!("ssh://[{ip}]"))
}
fn matching_peers<'a>(peers: &'a [Peer], query: &str) -> Vec<&'a Peer> {
    let query = query.trim().to_lowercase();
    peers
        .iter()
        .filter(|p| p.available() && target(p).is_some())
        .filter(|p| {
            query.is_empty()
                || p.name.to_lowercase().contains(&query)
                || p.dns_name.to_lowercase().contains(&query)
                || p.addresses.iter().any(|a| a.contains(&query))
        })
        .collect()
}

impl HerdrWindow {
    pub(super) fn tailnet_picker_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(setup) = &mut self.menu.device_setup else {
            return false;
        };
        if !setup.picker.input().read(cx).focus.is_focused(window) {
            return false;
        }
        let peers = matching_peers(&setup.picker.snapshot.status.peers, &setup.picker.query(cx));
        let peers = &peers[..peers.len().min(64)];
        if peers.is_empty() || !matches!(key, "up" | "down" | "enter") {
            return false;
        }
        let current = peers
            .iter()
            .position(|peer| Some(peer.id.as_str()) == setup.picker.highlighted_peer.as_deref());
        if key == "enter" {
            if let Some(index) = current {
                let (id, generation, network) = (
                    peers[index].id.clone(),
                    setup.picker.snapshot.generation,
                    setup.picker.snapshot.status.network_id.clone(),
                );
                self.select_tailnet_device(&id, generation, &network, window, cx);
            }
            return true;
        }
        let index = match (current, key) {
            (Some(index), "up") => (index + peers.len() - 1) % peers.len(),
            (Some(index), _) => (index + 1) % peers.len(),
            (None, "up") => peers.len() - 1,
            _ => 0,
        };
        setup.picker.highlighted_peer = Some(peers[index].id.clone());
        setup.suggestion_scroll.scroll_to_item(index);
        cx.notify();
        true
    }

    fn select_tailnet_device(
        &mut self,
        id: &str,
        generation: u64,
        network: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = &mut self.menu.device_setup else {
            return;
        };
        let snapshot = &form.picker.snapshot;
        if !matches!(form.step, Step::Form)
            || !form.picker.choosing()
            || snapshot.status.state != State::Connected
            || snapshot.generation != generation
            || snapshot.status.network_id != network
        {
            return;
        }
        let Some(peer) = snapshot
            .status
            .peers
            .iter()
            .find(|peer| peer.id == id && peer.available())
            .cloned()
        else {
            return;
        };
        let Some(address) = target(&peer) else {
            return;
        };
        let name = super::discover::display_name(&peer.name);
        form.error = None;
        form.picker.select_tailnet();
        form.tailnet_peer = Some(crate::tailnet::Selection {
            generation,
            network: network.into(),
            peer,
        });
        form.fields[0].update(cx, |input, cx| input.set_text_selected(&address, cx));
        if form.fields[1].read(cx).text().is_empty() || form.suggested_label.is_some() {
            form.fields[1].update(cx, |input, cx| input.set_text_selected(&name, cx));
            form.suggested_label = Some(name);
        }
        window.focus(&form.fields[0].read(cx).focus.clone(), cx);
        cx.notify();
    }

    pub(super) fn start_tailnet_devices(&mut self, cx: &mut Context<Self>) {
        let Some(setup) = &self.menu.device_setup else {
            return;
        };
        let previous = setup.picker.snapshot.clone();
        let poll = cx.spawn(async move |view, cx| {
            let mut previous = previous;
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                let latest = crate::tailnet::snapshot();
                if latest.status == previous.status && latest.generation == previous.generation {
                    continue;
                }
                previous = latest.clone();
                if !matches!(
                    view.update(cx, |view, cx| {
                        if let Some(setup) = &mut view.menu.device_setup {
                            setup.picker.update_snapshot(latest);
                        }
                        cx.notify();
                        view.menu.device_setup.is_some()
                    }),
                    Ok(true)
                ) {
                    break;
                }
            }
        });
        if let Some(form) = &mut self.menu.device_setup {
            form.tailnet_poll = Some(poll);
        }
    }
    pub(super) fn render_tailnet_devices(
        &self,
        setup: &Setup,
        snapshot: Snapshot,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let status = snapshot.status;
        if status.state != State::Connected {
            return None;
        }
        let mut body = div()
            .id("tailnet-devices")
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(6.))
            .min_w_0()
            .child(
                div()
                    .text_color(rgb(self.theme.subtext()))
                    .child(fl!("tailnet-devices")),
            );
        body = body.child(
            div()
                .text_color(rgb(self.theme.subtext()))
                .child(status.network.clone()),
        );
        let query = setup.picker.query(cx);
        let mut list = div()
            .id("tailnet-device-list")
            .debug_selector(|| "tailnet-device-list".into())
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&setup.suggestion_scroll)
            .flex()
            .flex_col();
        let mut count = 0;
        let peers = matching_peers(&status.peers, &query);
        let matching = peers.len();
        for peer in peers.into_iter().take(64) {
            let peer = peer.clone();
            let Some(_) = target(&peer) else {
                continue;
            };
            count += 1;
            let network = status.network_id.clone();
            let generation = snapshot.generation;
            let name = super::discover::display_name(&peer.name);
            list = list.child(
                div()
                    .id(ElementId::Name(format!("tailnet-peer-{}", peer.id).into()))
                    .debug_selector({
                        let id = peer.id.clone();
                        move || format!("tailnet-peer-{id}")
                    })
                    .flex_none()
                    .cursor_pointer()
                    .rounded(px(4.))
                    .p(px(6.))
                    .when(
                        setup.picker.highlighted_peer.as_deref() == Some(peer.id.as_str()),
                        |row| row.bg(rgb(self.theme.active)),
                    )
                    .hover(|s| s.bg(rgb(self.theme.active)))
                    .child(div().truncate().child(name.clone()))
                    .child(
                        div()
                            .truncate()
                            .text_color(rgb(self.theme.subtext()))
                            .child(peer.addresses.join(", ")),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_tailnet_device(&peer.id, generation, &network, window, cx);
                    })),
            );
        }
        if matching > 64 {
            body = body.child(fl!("tailnet-more", count = matching));
        }
        if count == 0 {
            body = body.child(fl!("tailnet-empty"));
        }
        Some(body.child(list))
    }
}
