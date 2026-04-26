use tauri::{AppHandle, Manager};

pub fn app_search_index_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|path| path.join("search").join("apps"))
        .map_err(|error| error.to_string())
}

pub fn app_theme_settings_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|path| path.join("settings").join("theme.json"))
        .map_err(|error| error.to_string())
}
