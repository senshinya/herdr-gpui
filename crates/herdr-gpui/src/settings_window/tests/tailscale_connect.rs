use super::*;

#[gpui::test]
fn connect_click_reports_missing_runtime_instead_of_being_dropped(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    cx.simulate_resize(size(px(960.), px(780.)));
    view.update_in(cx, |view, window, cx| {
        view.select_section(Section::Tailscale, window, cx)
    });
    cx.update(|window, cx| crate::sidebar::layout_tests::full_draw(window, cx).clear(cx));
    let point = cx.debug_bounds("tailnet-connect").unwrap().center();
    cx.simulate_click(point, Default::default());
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.error.clone()),
        Some("Embedded Tailscale is unavailable".into())
    );
}

#[gpui::test]
fn login_authorization_opens_the_browser_once(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(skill_fixture);
    let login: herdr_remote::tailscale::Snapshot = serde_json::from_str(
        r#"{"state":"needs_login","auth_url":"https://login.tailscale.com/a/test-authorization"}"#,
    )
    .unwrap();
    view.update_in(cx, |view, window, cx| {
        view.select_section(Section::Tailscale, window, cx);
        view.poll_tailnet(login.clone(), cx);
    });
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("https://login.tailscale.com/a/test-authorization")
    );
    cx.update(|_, cx| cx.open_url("https://example.com/another-tab"));
    view.update(cx, |view, cx| view.poll_tailnet(login, cx));
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("https://example.com/another-tab")
    );
}

#[gpui::test]
fn login_poll_refuses_unsafe_urls_and_other_states(cx: &mut TestAppContext) {
    use herdr_remote::tailscale::{Snapshot, State};
    let (view, cx) = cx.add_window_view(skill_fixture);
    view.update_in(cx, |view, window, cx| {
        view.select_section(Section::Tailscale, window, cx)
    });
    for (state, url) in [
        (State::NeedsLogin, "http://example.com/login"),
        (State::NeedsLogin, "https://user:secret@example.com/login"),
        (State::NeedsLogin, "javascript:alert(1)"),
        (State::Connected, "https://login.tailscale.com/a/stale"),
        (State::Failed, "https://login.tailscale.com/a/failed"),
    ] {
        let mut snapshot = Snapshot::default();
        snapshot.status.state = state;
        snapshot.auth_url = Some(secrecy::SecretString::from(url));
        view.update(cx, |view, cx| view.poll_tailnet(snapshot, cx));
        assert!(cx.opened_url().is_none());
    }
}
