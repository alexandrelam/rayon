use crate::app::AppState;
use rayon_types::ThemePreference;
use std::sync::Arc;

#[tauri::command]
pub fn get_theme_preference(state: tauri::State<'_, Arc<AppState>>) -> ThemePreference {
    state.theme_preference()
}
