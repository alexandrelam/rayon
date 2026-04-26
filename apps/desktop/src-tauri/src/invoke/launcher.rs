use crate::{app::AppState, MAIN_WINDOW_LABEL};
use rayon_core::APP_REINDEX_COMMAND_ID;
use rayon_types::{
    CommandExecutionRequest, CommandInvocationResult, InteractiveSessionQueryRequest,
    InteractiveSessionState, InteractiveSessionSubmitRequest, InteractiveSessionSubmitResult,
    SearchResult,
};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn search(query: String, state: tauri::State<'_, AppState>) -> Vec<SearchResult> {
    state.read_launcher().search(&query)
}

#[tauri::command]
pub fn execute_command(
    request: CommandExecutionRequest,
    state: tauri::State<'_, AppState>,
) -> Result<CommandInvocationResult, String> {
    if request.command_id.as_str() == APP_REINDEX_COMMAND_ID {
        return state
            .reload()
            .map(|result| CommandInvocationResult::Completed {
                output: result.output,
            });
    }

    state
        .read_launcher()
        .execute_command(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn search_interactive_session(
    request: InteractiveSessionQueryRequest,
    state: tauri::State<'_, AppState>,
) -> Result<InteractiveSessionState, String> {
    state
        .read_launcher()
        .search_interactive_session(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn submit_interactive_session(
    request: InteractiveSessionSubmitRequest,
    state: tauri::State<'_, AppState>,
) -> Result<InteractiveSessionSubmitResult, String> {
    state
        .read_launcher()
        .submit_interactive_session(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn hide_launcher(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main window is not available".to_string())?;

    window.hide().map_err(|error| error.to_string())
}
