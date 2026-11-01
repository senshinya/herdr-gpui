use super::*;
#[test]
fn nearby_sources_never_schedule_a_system_tailscale_reader() {
    assert_eq!(Source::ALL, [Source::SshConfig, Source::Bonjour]);
}
#[gpui::test]
fn choosing_nearby_clears_the_previous_tailnet_identity(cx: &mut gpui::TestAppContext) {
    let (view, cx) = form::open(cx, form::report);
    view.update_in(cx, |view, window, cx| {
        view.menu.device_setup.as_mut().unwrap().tailnet_peer = Some(crate::tailnet::Selection {
            generation: 0,
            network: String::new(),
            peer: herdr_remote::tailscale::Peer {
                id: "previous-peer".into(),
                ..Default::default()
            },
        });
        view.use_device_suggestion("nearby.local", "Nearby", window, cx);
        assert!(
            view.menu
                .device_setup
                .as_ref()
                .unwrap()
                .tailnet_peer
                .is_none()
        );
    });
}
