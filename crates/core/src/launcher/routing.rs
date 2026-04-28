use crate::commands::APP_REINDEX_COMMAND_ID;
use rayon_types::{
    parse_browser_tab_command_id, parse_open_window_command_id, BrowserTabTarget, CommandId,
    OpenWindowTarget,
};

pub(super) enum ExecutionTarget {
    Reindex,
    App(CommandId),
    BrowserTab(BrowserTabTarget),
    OpenWindow(OpenWindowTarget),
    Bookmark(CommandId),
    Image(CommandId),
    Provider(CommandId),
}

pub(super) fn resolve_execution_target(
    command_id: &CommandId,
    bookmark_exists: bool,
    image_exists: bool,
) -> ExecutionTarget {
    if command_id.as_str() == APP_REINDEX_COMMAND_ID {
        return ExecutionTarget::Reindex;
    }

    if command_id.as_str().starts_with("app:macos:") {
        return ExecutionTarget::App(command_id.clone());
    }

    if let Some(target) = parse_browser_tab_command_id(command_id) {
        return ExecutionTarget::BrowserTab(target);
    }

    if let Some(target) = parse_open_window_command_id(command_id) {
        return ExecutionTarget::OpenWindow(target);
    }

    if bookmark_exists {
        return ExecutionTarget::Bookmark(command_id.clone());
    }

    if image_exists {
        return ExecutionTarget::Image(command_id.clone());
    }

    ExecutionTarget::Provider(command_id.clone())
}
