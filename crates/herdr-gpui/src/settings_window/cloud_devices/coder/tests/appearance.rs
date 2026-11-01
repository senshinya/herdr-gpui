use super::*;

#[gpui::test]
fn restyling_keeps_unsubmitted_fields_and_the_focused_secret(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let (field, secret) = view.read_with(cx, |view, _| {
        let card = view.coder_card.as_ref().unwrap();
        (card.fields[0].clone(), card.secret.clone())
    });
    field.update(cx, |input, cx| {
        input.set_text_selected("https://draft.example.com", cx);
    });
    secret.update(cx, |input, cx| {
        input.set_text_selected("unsaved fixture secret", cx);
    });
    cx.update(|window, cx| window.focus(&secret.read(cx).focus.clone(), cx));
    view.update(cx, |view, cx| {
        view.theme = crate::config::Theme::builtin("Catppuccin Latte").unwrap();
        view.config.ui.size = 24.;
        view.refresh_control_appearance(cx);
    });
    assert_eq!(
        field.read_with(cx, |input, _| input.text().to_owned()),
        "https://draft.example.com"
    );
    assert_eq!(
        secret.read_with(cx, |input, _| input.text().to_owned()),
        "unsaved fixture secret"
    );
    cx.update(|window, cx| assert!(secret.read(cx).focus.is_focused(window)));
    view.read_with(cx, |view, cx| {
        let card = view.coder_card.as_ref().unwrap();
        assert_eq!(card.fields[0], field);
        assert_eq!(card.secret, secret);
        assert!(card.job.is_none());
        assert!(view.config.coder.oauth_client_secret.is_none());
        assert_eq!(card.secret.read(cx).text(), "unsaved fixture secret");
    });
}
