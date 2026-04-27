#![allow(unexpected_cfgs)]

mod app;
mod invoke;
mod shell;

use std::sync::Arc;
use tauri::{Manager, WindowEvent};

const MAIN_WINDOW_LABEL: &str = "main";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;

                    if event.state() == ShortcutState::Pressed {
                        let _ = shell::toggle_launcher(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_state =
                app::AppState::new(app.handle()).map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(Arc::new(app_state));
            shell::set_macos_activation_policy(app);
            shell::build_tray(app)?;
            shell::register_global_shortcut(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == MAIN_WINDOW_LABEL && matches!(event, WindowEvent::Focused(false)) {
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            invoke::launcher::search,
            invoke::launcher::execute_command,
            invoke::launcher::search_interactive_session,
            invoke::launcher::submit_interactive_session,
            invoke::launcher::hide_launcher,
            invoke::launcher::hide_launcher_and_restore_focus,
            invoke::launcher::resize_launcher,
            invoke::preferences::get_theme_preference
        ])
        .run(tauri::generate_context!());

    if let Err(error) = app {
        eprintln!("error while running tauri application: {error}");
    }
}
