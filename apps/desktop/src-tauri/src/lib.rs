use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};

use rayon_core::{load_config_providers, CommandRegistry, LauncherService};
use rayon_db::TantivySearchIndex;
use rayon_features::built_in_providers;
use rayon_platform::MacOsAppManager;
use rayon_types::{CommandExecutionRequest, CommandExecutionResult, SearchResult};
use std::path::PathBuf;
use std::sync::Arc;

const MAIN_WINDOW_LABEL: &str = "main";
const LAUNCHER_OPENED_EVENT: &str = "launcher:opened";

struct AppState {
    launcher: LauncherService,
}

impl AppState {
    fn new(app: &AppHandle) -> Result<Self, String> {
        let mut registry = CommandRegistry::new();

        for provider in built_in_providers() {
            registry
                .register_provider(provider)
                .map_err(|error| format!("failed to register built-in provider: {error}"))?;
        }

        for provider in load_config_providers().map_err(|error| error.to_string())? {
            registry
                .register_provider(provider)
                .map_err(|error| error.to_string())?;
        }

        let app_index = Arc::new(
            TantivySearchIndex::open_or_create(app_search_index_path(app)?)
                .map_err(|error| error.to_string())?,
        );
        let platform = Arc::new(MacOsAppManager);

        Ok(Self {
            launcher: LauncherService::new(registry, platform, app_index),
        })
    }
}

fn app_search_index_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|path| path.join("search").join("apps"))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn search(query: String, state: tauri::State<'_, AppState>) -> Vec<SearchResult> {
    state.launcher.search(&query)
}

#[tauri::command]
fn execute_command(
    request: CommandExecutionRequest,
    state: tauri::State<'_, AppState>,
) -> Result<CommandExecutionResult, String> {
    state
        .launcher
        .execute(&request)
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
            hide_launcher
        ])
        .run(tauri::generate_context!());

    if let Err(error) = app {
        eprintln!("error while running tauri application: {error}");
    }
}
