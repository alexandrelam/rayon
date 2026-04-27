use crate::{app::AppState, shell, MAIN_WINDOW_LABEL};
use rayon_core::APP_REINDEX_COMMAND_ID;
use rayon_types::{
    CommandExecutionRequest, CommandInvocationResult, InteractiveSessionQueryRequest,
    InteractiveSessionState, InteractiveSessionSubmitRequest, InteractiveSessionSubmitResult,
    SearchResult,
};
use std::sync::Arc;
use tauri::{AppHandle, LogicalSize, Manager, Size};

#[tauri::command]
pub async fn search(
    query: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<SearchResult>, String> {
    let state = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || state.search(&query))
        .await
        .map_err(|error| format!("launcher task failed: {error}"))
}

#[tauri::command]
pub async fn execute_command(
    request: CommandExecutionRequest,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<CommandInvocationResult, String> {
    let state = Arc::clone(state.inner());

    if request.command_id.as_str() == APP_REINDEX_COMMAND_ID {
        return tauri::async_runtime::spawn_blocking(move || state.reload())
            .await
            .map_err(|error| format!("launcher task failed: {error}"))?
            .map(|result| CommandInvocationResult::Completed {
                output: result.output,
            });
    }

    tauri::async_runtime::spawn_blocking(move || state.execute_command(&request))
        .await
        .map_err(|error| format!("launcher task failed: {error}"))?
}

#[tauri::command]
pub async fn search_interactive_session(
    request: InteractiveSessionQueryRequest,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<InteractiveSessionState, String> {
    let state = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || state.search_interactive_session(&request))
        .await
        .map_err(|error| format!("launcher task failed: {error}"))?
}

#[tauri::command]
pub async fn submit_interactive_session(
    request: InteractiveSessionSubmitRequest,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<InteractiveSessionSubmitResult, String> {
    let state = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || state.submit_interactive_session(&request))
        .await
        .map_err(|error| format!("launcher task failed: {error}"))?
}

#[tauri::command]
pub fn hide_launcher(app: AppHandle) -> Result<(), String> {
    shell::hide_launcher(&app).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn hide_launcher_and_restore_focus(app: AppHandle) -> Result<(), String> {
    shell::hide_launcher_and_restore_focus(&app).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn resize_launcher(app: AppHandle, height: f64) -> Result<(), String> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main window is not available".to_string())?;

    let clamped_height = height.clamp(160.0, 420.0);
    window
        .set_size(Size::Logical(LogicalSize::new(760.0, clamped_height)))
        .map_err(|error| error.to_string())
}
