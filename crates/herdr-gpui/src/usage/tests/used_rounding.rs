use super::super::text;
use herdr_usage::model::{Kind, Window};

#[test]
fn fractional_usage_matches_the_whole_percent_shown_in_the_status_bar() {
    for (used, percent) in [(11.4, 11), (11.6, 12), (99.8, 100)] {
        let limit = Window::new(Kind::Weekly, used, None, None);
        let plain = |value: String| value.replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(plain(text::used(&limit)), format!("{percent}% used"));
        assert_eq!(
            plain(text::used_heading(&limit)),
            format!("Weekly {percent}% used")
        );
    }
}
