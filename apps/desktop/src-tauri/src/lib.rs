use rayon_core::CommandRegistry;
use rayon_features::built_in_providers;
use rayon_types::{CommandExecutionResult, CommandId, SearchResult};

struct AppState {
    registry: CommandRegistry,
}

impl AppState {
    fn new() -> Self {
        let mut registry = CommandRegistry::new();

        for provider in built_in_providers() {
            registry
                .register_provider(provider)
                .expect("built-in providers must register without conflicts");
        }

        Self { registry }
    }
}

#[tauri::command]
fn search(query: String, state: tauri::State<'_, AppState>) -> Vec<SearchResult> {
    state.registry.search(&query)
}

#[tauri::command]
fn execute_command(
    command_id: String,
    payload: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<CommandExecutionResult, String> {
    state
        .registry
        .execute(&CommandId::from(command_id), payload)
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![search, execute_command])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
