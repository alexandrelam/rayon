#![allow(unexpected_cfgs)]

use crate::MAIN_WINDOW_LABEL;
use std::sync::{Mutex, OnceLock};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

const LAUNCHER_OPENED_EVENT: &str = "launcher:opened";
const LAUNCHER_OPEN_ANIMATION_MS: u64 = 110;
const LAUNCHER_CLOSE_ANIMATION_MS: u64 = 110;

#[derive(Clone, Copy, Eq, PartialEq)]
enum PendingHideAction {
    None,
    Hide,
    HideAndRestoreFocus,
}

fn previous_frontmost_pid() -> &'static Mutex<Option<i32>> {
    static PREVIOUS_FRONTMOST_PID: OnceLock<Mutex<Option<i32>>> = OnceLock::new();
    PREVIOUS_FRONTMOST_PID.get_or_init(|| Mutex::new(None))
}

fn launcher_hide_pending() -> &'static Mutex<PendingHideAction> {
    static LAUNCHER_HIDE_PENDING: OnceLock<Mutex<PendingHideAction>> = OnceLock::new();
    LAUNCHER_HIDE_PENDING.get_or_init(|| Mutex::new(PendingHideAction::None))
}

pub fn show_launcher(app: &AppHandle) -> tauri::Result<()> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| tauri::Error::AssetNotFound(MAIN_WINDOW_LABEL.into()))?;

    store_previous_frontmost_application();
    set_launcher_hide_pending(PendingHideAction::None);

    #[cfg(target_os = "macos")]
    {
        app.show()?;
        prepare_launcher_window_for_show(&window)?;
    }

    window.unminimize()?;
    window.center()?;
    window.show()?;

    #[cfg(target_os = "macos")]
    animate_launcher_window_alpha(&window, 1.0, LAUNCHER_OPEN_ANIMATION_MS)?;

    window.set_focus()?;
    window.emit(LAUNCHER_OPENED_EVENT, ())?;
    Ok(())
}

pub fn hide_launcher(app: &AppHandle) -> tauri::Result<()> {
    hide_launcher_window(app, false)
}

pub fn hide_launcher_and_restore_focus(app: &AppHandle) -> tauri::Result<()> {
    hide_launcher_window(app, true)
}

fn hide_launcher_window(app: &AppHandle, restore_focus: bool) -> tauri::Result<()> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| tauri::Error::AssetNotFound(MAIN_WINDOW_LABEL.into()))?;

    if !window.is_visible()? {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        start_launcher_hide_animation(app.clone(), restore_focus)?;
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        window.hide()?;
        if restore_focus {
            restore_previous_frontmost_application();
        } else {
            clear_previous_frontmost_application();
        }
        Ok(())
    }
}

pub fn toggle_launcher(app: &AppHandle) -> tauri::Result<()> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| tauri::Error::AssetNotFound(MAIN_WINDOW_LABEL.into()))?;

    if window.is_visible()? && window.is_focused()? {
        hide_launcher(app)?;
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

#[cfg(target_os = "macos")]
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[cfg(target_os = "macos")]
fn store_previous_frontmost_application() {
    let pid = unsafe { frontmost_application_pid() };
    if let Ok(mut previous_pid) = previous_frontmost_pid().lock() {
        *previous_pid = pid;
    }
}

#[cfg(not(target_os = "macos"))]
fn store_previous_frontmost_application() {}

fn clear_previous_frontmost_application() {
    if let Ok(mut previous_pid) = previous_frontmost_pid().lock() {
        *previous_pid = None;
    }
}

#[cfg(target_os = "macos")]
fn restore_previous_frontmost_application() {
    let pid = previous_frontmost_pid()
        .lock()
        .ok()
        .and_then(|mut previous_pid| previous_pid.take());

    if let Some(pid) = pid {
        unsafe {
            let _ = activate_application(pid);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn restore_previous_frontmost_application() {
    clear_previous_frontmost_application();
}

fn set_launcher_hide_pending(pending_action: PendingHideAction) {
    if let Ok(mut pending) = launcher_hide_pending().lock() {
        *pending = pending_action;
    }
}

fn take_launcher_hide_pending() -> PendingHideAction {
    if let Ok(mut pending) = launcher_hide_pending().lock() {
        let pending_action = *pending;
        *pending = PendingHideAction::None;
        return pending_action;
    }

    PendingHideAction::None
}

#[cfg(target_os = "macos")]
fn start_launcher_hide_animation(app: AppHandle, restore_focus: bool) -> tauri::Result<()> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| tauri::Error::AssetNotFound(MAIN_WINDOW_LABEL.into()))?;

    if register_pending_hide_action(restore_focus) {
        return Ok(());
    }

    animate_launcher_window_alpha(&window, 0.0, LAUNCHER_CLOSE_ANIMATION_MS)?;

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(
            LAUNCHER_CLOSE_ANIMATION_MS,
        ));
        let app_handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let pending_action = take_launcher_hide_pending();
            if pending_action == PendingHideAction::None {
                return;
            }

            if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
                let _ = window.hide();
            }

            if pending_action == PendingHideAction::HideAndRestoreFocus {
                restore_previous_frontmost_application();
            } else {
                clear_previous_frontmost_application();
            }
        });
    });

    Ok(())
}

fn register_pending_hide_action(restore_focus: bool) -> bool {
    if let Ok(mut pending) = launcher_hide_pending().lock() {
        let next_action = if restore_focus {
            PendingHideAction::HideAndRestoreFocus
        } else {
            PendingHideAction::Hide
        };

        if *pending != PendingHideAction::None {
            if *pending == PendingHideAction::Hide && restore_focus {
                *pending = PendingHideAction::HideAndRestoreFocus;
            }
            return true;
        }

        *pending = next_action;
    }

    false
}

#[cfg(target_os = "macos")]
fn prepare_launcher_window_for_show(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    set_launcher_window_alpha(window, 0.0)
}

#[cfg(target_os = "macos")]
fn animate_launcher_window_alpha(
    window: &tauri::WebviewWindow,
    alpha: f64,
    duration_ms: u64,
) -> tauri::Result<()> {
    use objc::{class, msg_send, runtime::Object, sel, sel_impl};

    let ns_window = window.ns_window()? as *mut Object;

    unsafe {
        let animation_context_class = class!(NSAnimationContext);
        let _: () = msg_send![animation_context_class, beginGrouping];
        let context: *mut Object = msg_send![animation_context_class, currentContext];
        let duration = duration_ms as f64 / 1000.0;
        let _: () = msg_send![context, setDuration: duration];
        let animator: *mut Object = msg_send![ns_window, animator];
        let _: () = msg_send![animator, setAlphaValue: alpha];
        let _: () = msg_send![animation_context_class, endGrouping];
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn set_launcher_window_alpha(window: &tauri::WebviewWindow, alpha: f64) -> tauri::Result<()> {
    use objc::{msg_send, runtime::Object, sel, sel_impl};

    let ns_window = window.ns_window()? as *mut Object;

    unsafe {
        let _: () = msg_send![ns_window, setAlphaValue: alpha];
    }

    Ok(())
}

#[cfg(target_os = "macos")]
unsafe fn frontmost_application_pid() -> Option<i32> {
    use objc::{class, msg_send, runtime::Object, sel, sel_impl};

    let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
    if workspace.is_null() {
        return None;
    }

    let application: *mut Object = msg_send![workspace, frontmostApplication];
    if application.is_null() {
        return None;
    }

    let pid: i32 = msg_send![application, processIdentifier];
    if pid == std::process::id() as i32 {
        return None;
    }

    Some(pid)
}

#[cfg(target_os = "macos")]
unsafe fn activate_application(pid: i32) -> bool {
    use objc::{
        class, msg_send,
        runtime::{Object, BOOL, YES},
        sel, sel_impl,
    };

    const NS_APPLICATION_ACTIVATE_ALL_WINDOWS: usize = 1 << 0;
    const NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS: usize = 1 << 1;

    let running_application: *mut Object = msg_send![
        class!(NSRunningApplication),
        runningApplicationWithProcessIdentifier: pid
    ];
    if running_application.is_null() {
        return false;
    }

    let options = NS_APPLICATION_ACTIVATE_ALL_WINDOWS | NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS;
    let activated: BOOL = msg_send![running_application, activateWithOptions: options];
    activated == YES
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn lock_previous_frontmost_pid() -> std::sync::MutexGuard<'static, Option<i32>> {
        previous_frontmost_pid()
            .lock()
            .expect("previous frontmost pid mutex should not be poisoned")
    }

    #[test]
    fn clear_previous_frontmost_application_resets_stored_pid() {
        {
            let mut previous_pid = lock_previous_frontmost_pid();
            *previous_pid = Some(4242);
        }

        clear_previous_frontmost_application();

        let previous_pid = lock_previous_frontmost_pid();
        assert_eq!(*previous_pid, None);
    }

    #[test]
    fn restore_previous_frontmost_application_consumes_stored_pid() {
        {
            let mut previous_pid = lock_previous_frontmost_pid();
            *previous_pid = Some(-1);
        }

        restore_previous_frontmost_application();

        let previous_pid = lock_previous_frontmost_pid();
        assert_eq!(*previous_pid, None);
    }
}
