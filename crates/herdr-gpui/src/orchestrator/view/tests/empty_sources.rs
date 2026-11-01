use super::*;
use crate::orchestrator::{Error, SourceStatus, SyncState, view::list::Empty};
use std::sync::Arc;

#[gpui::test]
fn ui_audit_an_unread_source_cannot_claim_there_are_no_items(cx: &mut gpui::TestAppContext) {
    let (_dir, view, cx) = empty::open(cx);
    for state in [
        SyncState::SignIn,
        SyncState::Failed(Arc::new(Error::NoDataDirectory)),
        SyncState::Unsupported,
    ] {
        view.update(cx, |view, _| {
            let repo = &mut view.repos[0];
            repo.snapshot.error = None;
            repo.snapshot.sources = vec![SourceStatus {
                key: source(Provider::Github),
                state,
            }];
            for tab in [Tab::Issues, Tab::PullRequests] {
                view.tab = tab;
                assert_ne!(view.empty(), Empty::Nothing);
                assert_ne!(view.empty(), Empty::Loading);
            }
        });
    }
}
