use super::*;

#[test]
fn contextual_help_preserves_ownership_and_disabled_reasons() {
    for disabled in [false, true] {
        let mut row = row(&Theme::default(), "help", "Option")
            .help("Additional help")
            .tag(Source::HerdrUi);
        if disabled {
            row = row.disabled("Unavailable here");
        }
        let element = row.into_element();
        let mut node = accesskit::Node::new(Role::Group);
        element.write_a11y_info(&mut node);
        assert_eq!(node.label(), Some("Option"));
        let description = node.description().unwrap();
        assert!(description.contains("Herdr UI"));
        assert!(description.starts_with(if disabled {
            "Unavailable here"
        } else {
            "Additional help"
        }));
    }
}
