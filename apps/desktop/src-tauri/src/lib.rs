use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};

use rayon_core::{
    load_config, AppPlatform, CommandRegistry, LauncherService, SearchIndex, APP_REINDEX_COMMAND_ID,
};
use rayon_db::TantivySearchIndex;
use rayon_features::built_in_providers;
use rayon_platform::MacOsAppManager;
use rayon_types::{
    BookmarkDefinition, CommandExecutionRequest, CommandExecutionResult, CommandInvocationResult,
    InteractiveSessionQueryRequest, InteractiveSessionState, InteractiveSessionSubmitRequest,
    SearchResult,
};
use std::path::PathBuf;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

const MAIN_WINDOW_LABEL: &str = "main";
const LAUNCHER_OPENED_EVENT: &str = "launcher:opened";

struct AppState {
    launcher: RwLock<LauncherService>,
    platform: Arc<dyn AppPlatform>,
    search_index: Arc<dyn SearchIndex>,
}

impl AppState {
    fn new(app: &AppHandle) -> Result<Self, String> {
        let app_index = Arc::new(
            TantivySearchIndex::open_or_create(app_search_index_path(app)?)
                .map_err(|error| error.to_string())?,
        );
        let platform = Arc::new(MacOsAppManager);
        let launcher = build_launcher(platform.clone(), app_index.clone())?;

        Ok(Self {
            launcher: RwLock::new(launcher),
            platform,
            search_index: app_index,
        })
    }

    fn reload(&self) -> Result<CommandExecutionResult, String> {
        reload_launcher(
            &self.launcher,
            self.platform.clone(),
            self.search_index.clone(),
        )
    }
}

fn build_launcher(
    platform: Arc<dyn AppPlatform>,
    search_index: Arc<dyn SearchIndex>,
) -> Result<LauncherService, String> {
    let (registry, bookmarks) = load_registry_and_bookmarks(platform.clone())?;
    Ok(LauncherService::new(
        registry,
        bookmarks,
        platform,
        search_index,
    ))
}

fn reload_launcher(
    launcher_slot: &RwLock<LauncherService>,
    platform: Arc<dyn AppPlatform>,
    search_index: Arc<dyn SearchIndex>,
) -> Result<CommandExecutionResult, String> {
    let launcher = build_launcher(platform, search_index)?;
    let result = launcher
        .execute_command(&CommandExecutionRequest {
            command_id: APP_REINDEX_COMMAND_ID.into(),
            arguments: Default::default(),
        })
        .map_err(|error| error.to_string())?;
    let CommandInvocationResult::Completed { output } = result else {
        return Err("reindex unexpectedly started an interactive session".into());
    };
    *write_launcher(launcher_slot) = launcher;
    Ok(CommandExecutionResult { output })
}

fn load_registry_and_bookmarks(
    platform: Arc<dyn AppPlatform>,
) -> Result<(CommandRegistry, Vec<BookmarkDefinition>), String> {
    let mut registry = CommandRegistry::new();
    let loaded_config = load_config().map_err(|error| error.to_string())?;

    for provider in built_in_providers(platform.clone()) {
        registry
            .register_provider(provider)
            .map_err(|error| format!("failed to register built-in provider: {error}"))?;
    }

    for provider in loaded_config.command_providers {
        registry
            .register_provider(provider)
            .map_err(|error| error.to_string())?;
    }

    validate_bookmark_ids(&registry, &loaded_config.bookmarks)?;
    Ok((registry, loaded_config.bookmarks))
}

fn validate_bookmark_ids(
    registry: &CommandRegistry,
    bookmarks: &[BookmarkDefinition],
) -> Result<(), String> {
    let command_ids = registry.search_results_by_id();
    for bookmark in bookmarks {
        if command_ids.contains_key(bookmark.id.as_str()) {
            return Err(format!(
                "bookmark id conflicts with an existing command id: {}",
                bookmark.id
            ));
        }
    }

    Ok(())
}

fn app_search_index_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|path| path.join("search").join("apps"))
        .map_err(|error| error.to_string())
}

fn read_launcher(launcher: &RwLock<LauncherService>) -> RwLockReadGuard<'_, LauncherService> {
    match launcher.read() {
        Ok(launcher) => launcher,
        Err(poisoned) => {
            eprintln!("launcher lock poisoned while reading");
            poisoned.into_inner()
        }
    }
}

fn write_launcher(launcher: &RwLock<LauncherService>) -> RwLockWriteGuard<'_, LauncherService> {
    match launcher.write() {
        Ok(launcher) => launcher,
        Err(poisoned) => {
            eprintln!("launcher lock poisoned while writing");
            poisoned.into_inner()
        }
    }
}

#[tauri::command]
fn search(query: String, state: tauri::State<'_, AppState>) -> Vec<SearchResult> {
    read_launcher(&state.launcher).search(&query)
}

#[tauri::command]
fn execute_command(
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

    read_launcher(&state.launcher)
        .execute_command(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn search_interactive_session(
    request: InteractiveSessionQueryRequest,
    state: tauri::State<'_, AppState>,
) -> Result<InteractiveSessionState, String> {
    read_launcher(&state.launcher)
        .search_interactive_session(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn submit_interactive_session(
    request: InteractiveSessionSubmitRequest,
    state: tauri::State<'_, AppState>,
) -> Result<InteractiveSessionState, String> {
    read_launcher(&state.launcher)
        .submit_interactive_session(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn hide_launcher(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main window is not available".to_string())?;

    window.hide().map_err(|error| error.to_string())
}

fn show_launcher(app: &AppHandle) -> tauri::Result<()> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| tauri::Error::AssetNotFound(MAIN_WINDOW_LABEL.into()))?;

    #[cfg(target_os = "macos")]
    {
        app.show()?;
    }

    window.unminimize()?;
    window.center()?;
    window.show()?;
    window.set_focus()?;
    window.emit(LAUNCHER_OPENED_EVENT, ())?;
    Ok(())
}

fn toggle_launcher(app: &AppHandle) -> tauri::Result<()> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| tauri::Error::AssetNotFound(MAIN_WINDOW_LABEL.into()))?;

    if window.is_visible()? && window.is_focused()? {
        window.hide()?;
        return Ok(());
    }

    show_launcher(app)
}

#[cfg(target_os = "macos")]
fn set_macos_activation_policy(app: &mut tauri::App) {
    use tauri::ActivationPolicy;

    app.set_activation_policy(ActivationPolicy::Accessory);
    app.set_dock_visibility(false);
}

#[cfg(not(target_os = "macos"))]
fn set_macos_activation_policy(_app: &mut tauri::App) {}

fn register_global_shortcut(app: &AppHandle) -> tauri::Result<()> {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

    #[cfg(target_os = "macos")]
    let primary = Shortcut::new(Some(Modifiers::SUPER), Code::Space);
    #[cfg(not(target_os = "macos"))]
    let primary = Shortcut::new(Some(Modifiers::CONTROL), Code::Space);

    #[cfg(target_os = "macos")]
    let fallback = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Space);
    #[cfg(not(target_os = "macos"))]
    let fallback = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);

    if let Err(error) = app.global_shortcut().register(primary) {
        eprintln!("failed to register primary launcher shortcut: {error}");
        if let Err(fallback_error) = app.global_shortcut().register(fallback) {
            eprintln!("failed to register fallback launcher shortcut: {fallback_error}");
        }
    }

    Ok(())
}

fn build_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Rayon", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let icon = app
        .default_window_icon()
        .ok_or_else(|| tauri::Error::AssetNotFound("default icon".into()))?
        .clone();

    TrayIconBuilder::with_id("tray")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                let _ = show_launcher(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = toggle_launcher(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;

                    if event.state() == ShortcutState::Pressed {
                        let _ = toggle_launcher(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_state =
                AppState::new(app.handle()).map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(app_state);
            set_macos_activation_policy(app);
            build_tray(app)?;
            register_global_shortcut(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == MAIN_WINDOW_LABEL && matches!(event, WindowEvent::Focused(false)) {
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            search,
            execute_command,
            search_interactive_session,
            submit_interactive_session,
            hide_launcher
        ])
        .run(tauri::generate_context!());

    if let Err(error) = app {
        eprintln!("error while running tauri application: {error}");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rayon_db::SearchIndexStats;
    use rayon_types::{CommandId, InstalledApp, ProcessMatch, SearchableItemDocument};
    use std::fs;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct StubPlatform;

    impl AppPlatform for StubPlatform {
        fn discover_apps(&self) -> Result<Vec<InstalledApp>, String> {
            Ok(Vec::new())
        }

        fn launch_app(&self, _app: &InstalledApp) -> Result<(), String> {
            Ok(())
        }

        fn open_url(&self, _url: &str) -> Result<(), String> {
            Ok(())
        }

        fn search_processes(&self, _query: &str) -> Result<Vec<ProcessMatch>, String> {
            Ok(Vec::new())
        }

        fn terminate_process(&self, _pid: u32) -> Result<(), String> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct StubSearchIndex {
        documents: Mutex<Vec<SearchableItemDocument>>,
    }

    impl SearchIndex for StubSearchIndex {
        fn is_configured(&self) -> bool {
            true
        }

        fn search_item_ids(&self, query: &str, limit: usize) -> Result<Vec<String>, String> {
            let query = query.to_lowercase();
            Ok(self
                .documents
                .lock()
                .unwrap()
                .iter()
                .filter(|document| document.search_text.to_lowercase().contains(&query))
                .take(limit)
                .map(|document| document.id.to_string())
                .collect())
        }

        fn replace_items(
            &self,
            items: &[SearchableItemDocument],
        ) -> Result<SearchIndexStats, String> {
            *self.documents.lock().unwrap() = items.to_vec();
            Ok(SearchIndexStats {
                discovered_count: items.len(),
                indexed_count: items.len(),
                skipped_count: 0,
            })
        }
    }

    fn temp_config_home(test_name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rayon-{test_name}-{unique}"))
    }

    fn write_config(path: &Path, manifests: &[(&str, &str)]) {
        fs::create_dir_all(path.join("rayon")).unwrap();
        for (name, contents) in manifests {
            fs::write(path.join("rayon").join(name), contents).unwrap();
        }
    }

    #[test]
    fn reload_launcher_picks_up_new_commands_and_bookmarks() {
        let _env_guard = env_lock().lock().unwrap();
        let config_home = temp_config_home("reload-success");
        write_config(
            &config_home,
            &[(
                "commands.toml",
                r#"
plugin_id = "user.commands"

[[commands]]
id = "user.echo"
title = "Echo"
program = "/bin/echo"
"#,
            )],
        );
        std::env::set_var("XDG_CONFIG_HOME", &config_home);

        let search_index = Arc::new(StubSearchIndex::default());
        let platform: Arc<dyn AppPlatform> = Arc::new(StubPlatform);
        let launcher = build_launcher(platform.clone(), search_index.clone()).unwrap();
        let launcher_slot = RwLock::new(launcher);

        write_config(
            &config_home,
            &[
                (
                    "commands.toml",
                    r#"
plugin_id = "user.commands"

[[commands]]
id = "user.echo"
title = "Echo"
program = "/bin/echo"

[[commands]]
id = "user.ping"
title = "Ping"
program = "/bin/echo"
base_args = ["pong"]
"#,
                ),
                (
                    "bookmarks.toml",
                    r#"
plugin_id = "user.bookmarks"

[[bookmarks]]
id = "user.jira"
title = "Jira Board"
url = "https://example.com/jira"
keywords = ["jira", "board"]
"#,
                ),
            ],
        );

        let result = reload_launcher(&launcher_slot, platform, search_index).unwrap();
        assert!(result.output.starts_with("reindexed "));

        let launcher = read_launcher(&launcher_slot);
        let ping_results = launcher.search("ping");
        assert!(ping_results
            .iter()
            .any(|result| result.id == CommandId::from("user.ping")));

        let jira_results = launcher.search("jira");
        assert!(jira_results
            .iter()
            .any(|result| result.id == CommandId::from("user.jira")));

        fs::remove_dir_all(config_home).unwrap();
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    fn reload_launcher_keeps_previous_state_when_config_is_invalid() {
        let _env_guard = env_lock().lock().unwrap();
        let config_home = temp_config_home("reload-failure");
        write_config(
            &config_home,
            &[(
                "bookmarks.toml",
                r#"
plugin_id = "user.bookmarks"

[[bookmarks]]
id = "user.docs"
title = "Docs"
url = "https://example.com/docs"
keywords = ["docs"]
"#,
            )],
        );
        std::env::set_var("XDG_CONFIG_HOME", &config_home);

        let search_index = Arc::new(StubSearchIndex::default());
        let platform: Arc<dyn AppPlatform> = Arc::new(StubPlatform);
        let launcher = build_launcher(platform.clone(), search_index.clone()).unwrap();
        let launcher_slot = RwLock::new(launcher);

        write_config(
            &config_home,
            &[(
                "bookmarks.toml",
                r#"
plugin_id = "user.bookmarks"

[[bookmarks]]
id = "user.docs"
title = "Broken"
url = "not-a-url"
"#,
            )],
        );

        let error = reload_launcher(&launcher_slot, platform, search_index).unwrap_err();
        assert!(error.contains("invalid bookmark url"));

        let launcher = read_launcher(&launcher_slot);
        let results = launcher.search("docs");
        assert!(results
            .iter()
            .any(|result| result.id == CommandId::from("user.docs")));

        fs::remove_dir_all(config_home).unwrap();
        std::env::remove_var("XDG_CONFIG_HOME");
    }
}
