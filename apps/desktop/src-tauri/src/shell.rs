use crate::MAIN_WINDOW_LABEL;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

const LAUNCHER_OPENED_EVENT: &str = "launcher:opened";

pub fn show_launcher(app: &AppHandle) -> tauri::Result<()> {
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

pub fn toggle_launcher(app: &AppHandle) -> tauri::Result<()> {
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
pub fn set_macos_activation_policy(app: &mut tauri::App) {
    use tauri::ActivationPolicy;

    app.set_activation_policy(ActivationPolicy::Accessory);
    app.set_dock_visibility(false);
}

#[cfg(not(target_os = "macos"))]
pub fn set_macos_activation_policy(_app: &mut tauri::App) {}

pub fn register_global_shortcut(app: &AppHandle) -> tauri::Result<()> {
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

pub fn build_tray(app: &mut tauri::App) -> tauri::Result<()> {
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
