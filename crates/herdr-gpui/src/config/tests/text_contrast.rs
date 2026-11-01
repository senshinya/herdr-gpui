#![allow(clippy::unwrap_used)]
use super::*;

#[test]
fn ui_audit_muted_text_remains_readable_on_chrome_in_standard_mode() {
    for name in ["Default", "GitHub Dark Default", "GitHub Light Default"] {
        let original = Theme::builtin(name).unwrap();
        let theme = original.clone().styled_for(&Config::default());
        for background in [theme.background, theme.surface, theme.active] {
            let ratio = crate::contrast::ratio(theme.muted, background);
            assert!(ratio >= 4.5, "{name}: muted text ratio {ratio}");
        }
        assert_eq!(theme.palette, original.palette);
        assert_eq!(theme.foreground, original.foreground);
        assert_eq!(theme.background, original.background);
    }
}
