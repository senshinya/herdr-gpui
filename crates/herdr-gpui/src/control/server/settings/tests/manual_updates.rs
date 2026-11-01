use super::*;

#[test]
fn a_browser_update_reports_the_available_version_without_claiming_it_is_staged() {
    assert_eq!(
        describe(&State::Manual {
            version: "20261028.1-shinya.2".into(),
            notes: "Release notes".into(),
        }),
        ("manual", None, Some("20261028.1-shinya.2")),
    );
}
