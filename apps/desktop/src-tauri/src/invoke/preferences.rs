use crate::app::AppState;
use rayon_types::ThemePreference;

#[tauri::command]
pub fn get_theme_preference(state: tauri::State<'_, AppState>) -> ThemePreference {
    state.theme_preference()
}
