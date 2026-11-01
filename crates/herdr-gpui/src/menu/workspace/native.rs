//! Native fixture navigation follows the current menu, including hover state.
use super::{HerdrWindow, WorkspaceAction, WorkspaceMenuAction};

pub(crate) fn dialog_steps(view: &HerdrWindow, action: WorkspaceAction) -> Option<usize> {
    let actions = view.workspace_menu_actions();
    let target = actions
        .iter()
        .position(|entry| *entry == WorkspaceMenuAction::Dialog(action))?;
    let current = view
        .menu
        .workspace_selected
        .and_then(|selected| actions.iter().position(|entry| *entry == selected));
    Some(current.map_or(target + 1, |current| {
        (target + actions.len() - current) % actions.len()
    }))
}
