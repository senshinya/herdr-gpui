use super::*;
use herdr_client::{HostKeyScan, SshFailure};

fn request() -> setup::Request {
    setup::Request::new("dev@100.64.0.12", "", "")
        .unwrap()
        .with_tailnet(Some(crate::tailnet::Selection {
            generation: 1,
            network: "test-network".into(),
            peer: herdr_remote::tailscale::Peer {
                id: "test-peer".into(),
                ..Default::default()
            },
        }))
}

#[gpui::test]
fn tailnet_preparation_failure_releases_the_claim_for_retry(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            open_form(view, Step::Checking(request()), cx);
            view.device_probed(
                Ok((
                    setup::Claim::fixture("tailnet-auth-retry"),
                    Ok(HostProbe::SshFailed(SshFailure::Auth)),
                )),
                cx,
            );
            assert!(matches!(step(view), Step::Form));
            let form = view.menu.device_setup.as_ref().unwrap();
            assert!(form.claim.is_none());
            assert!(form.error.is_some());
        });
    });
}

#[gpui::test]
fn tailnet_changed_host_key_keeps_its_diagnostic_without_offering_external_setup(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            open_form(view, Step::FetchingKey(request()), cx);
            view.menu.device_setup.as_mut().unwrap().claim =
                Some(setup::Claim::fixture("tailnet-key-retry"));
            view.host_key_fetched(Ok(HostKeyScan::Refused(SshFailure::HostKey)), cx);
            assert!(matches!(step(view), Step::Form));
            let form = view.menu.device_setup.as_ref().unwrap();
            assert!(form.claim.is_none());
            assert!(form.error.as_ref().unwrap().contains("differs"));
        });
    });
}
