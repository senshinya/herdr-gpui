use super::*;
use crate::menu::{Page, WorkspaceAction};

const HOST: &str = "ssh:0123456789abcdef0123456789abcdef";

#[gpui::test]
fn device_workspace_creation_waits_for_the_chosen_surface_and_runs_once(
    cx: &mut gpui::TestAppContext,
) {
    let (fixture, cx) = cx.add_window_view(|window, cx| {
        Fixture(cx.new(|cx| crate::sidebar::layout_tests::fixture_window(window, cx)))
    });
    let view = fixture.update(cx, |fixture, _| fixture.0.clone());
    let (endpoint, mut server) = connected_endpoint(HOST);
    view.update_in(cx, |view, window, cx| {
        view.endpoints[0].detached = true;
        view.endpoints.push(endpoint);
        view.options = ConnectOptions::default();
        view.new_workspace_on_device(HOST, window, cx);
        assert_eq!(view.selected_endpoint, 1);
        assert!(view.activation_deadline.is_some());
        assert!(view.live.activation.is_none());
        view.poll_endpoints(cx);
        view.poll_device_workspace(window, cx);
        // The command's own focus barrier proves it was dispatched now.
        assert!(view.live.activation.is_some());
        view.poll_device_workspace(window, cx);
    });
    let ClientMessage::ClientShellEndpointRequest { request, .. } = server.receive() else {
        panic!("expected workspace creation on the selected device");
    };
    let request: serde_json::Value = serde_json::from_str(&request).unwrap();
    assert_eq!(request["method"], "workspace.create");
    assert_eq!(
        request["params"],
        serde_json::json!({
            "focus": true, "source_workspace_id": "w1"
        })
    );
    server.respond(&request);
    let ClientMessage::ClientShellEndpointRequest { request, .. } = server.receive() else {
        panic!("expected the creation's surface barrier");
    };
    let barrier: serde_json::Value = serde_json::from_str(&request).unwrap();
    assert_eq!(barrier["method"], "client_shell.surface.set");
    server.respond(&barrier);
    view.update(cx, |view, _| {
        view.endpoints[1]
            .connection
            .handle
            .as_ref()
            .unwrap()
            .set_focus(&snapshot().boot_id, false)
            .unwrap();
    });
    assert!(matches!(
        server.receive(),
        ClientMessage::ClientShellFocus { focused: false }
    ));
}

#[gpui::test]
fn device_workspace_creation_honors_naming_and_cancels_on_connection_reset(
    cx: &mut gpui::TestAppContext,
) {
    let (fixture, cx) = cx.add_window_view(|window, cx| {
        Fixture(cx.new(|cx| crate::sidebar::layout_tests::fixture_window(window, cx)))
    });
    let view = fixture.update(cx, |fixture, _| fixture.0.clone());
    let (endpoint, mut server) = connected_endpoint("local");
    view.update_in(cx, |view, window, cx| {
        view.endpoints = vec![endpoint];
        view.options = ConnectOptions::default();
        view.reset_selected(cx);
        view.settings.shared = Some(
            crate::herdr_settings::Settings::parse_text("[ui]\nprompt_new_workspace_name = true\n")
                .unwrap(),
        );
        let ready = view.live.clone();
        view.live.surface = None;
        view.new_workspace_on_device("local", window, cx);
        assert!(view.menu.page.is_none());
        view.reset_selected(cx);
        view.live = ready;
        view.poll_device_workspace(window, cx);
        assert!(
            view.menu.page.is_none(),
            "a reset must cancel deferred creation"
        );
        view.new_workspace_on_device("local", window, cx);
        assert_eq!(
            view.menu.page,
            Some(Page::Dialog(WorkspaceAction::NewWorkspace))
        );
        view.dismiss_menu(window, cx);
        view.endpoints[0]
            .connection
            .handle
            .as_ref()
            .unwrap()
            .set_focus(&snapshot().boot_id, false)
            .unwrap();
    });
    assert!(matches!(
        server.receive(),
        ClientMessage::ClientShellFocus { focused: false }
    ));
}
