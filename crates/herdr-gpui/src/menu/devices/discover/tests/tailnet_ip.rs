use super::*;
use herdr_remote::tailscale::{Peer, Snapshot, State};

#[gpui::test]
fn tailnet_selection_fills_an_ip_and_preserves_the_peer_binding(cx: &mut gpui::TestAppContext) {
    let (view, cx) = form::open(cx, form::report);
    for (addresses, expected, keyboard) in [
        (
            vec!["fd7a:115c:a1e0::42", "100.64.0.42"],
            "100.64.0.42",
            false,
        ),
        (
            vec!["not-an-address", "100.64.0.43", "fd7a:115c:a1e0::43"],
            "100.64.0.43",
            true,
        ),
        (
            vec!["fd7a:115c:a1e0::44"],
            "ssh://[fd7a:115c:a1e0::44]",
            false,
        ),
    ] {
        let peer = Peer {
            id: "fixture-ip-peer".into(),
            name: "IP workstation".into(),
            dns_name: "workstation.example.ts.net.".into(),
            addresses: addresses.into_iter().map(str::to_owned).collect(),
            online: true,
            ..Default::default()
        };
        let mut snapshot = Snapshot {
            generation: 8,
            ..Default::default()
        };
        snapshot.status.state = State::Connected;
        snapshot.status.network_id = "fixture-network".into();
        snapshot.status.peers = vec![peer.clone()];
        view.update_in(cx, |view, window, cx| {
            let setup = view.menu.device_setup.as_mut().unwrap();
            setup.tailnet_poll = None;
            setup.picker.update_snapshot(snapshot);
            view.show_device_picker(window, cx);
        });
        form::settle(cx);
        let tab = cx.debug_bounds("device-picker-tailnet").unwrap().center();
        cx.simulate_click(tab, Default::default());
        cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
        if keyboard {
            cx.simulate_keystrokes("down enter");
        } else {
            let row = cx
                .debug_bounds("tailnet-peer-fixture-ip-peer")
                .unwrap()
                .center();
            cx.simulate_click(row, Default::default());
        }
        view.read_with(cx, |view, cx| {
            let setup = view.menu.device_setup.as_ref().unwrap();
            assert_eq!(setup.fields[0].read(cx).text(), expected);
            assert_eq!(
                setup.tailnet_peer,
                Some(crate::tailnet::Selection {
                    generation: 8,
                    network: "fixture-network".into(),
                    peer,
                })
            );
        });
    }
}
