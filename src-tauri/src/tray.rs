use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter as _, Manager, Runtime,
};
use tracing::info;
use std::sync::{Arc, Mutex};

pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let polish_enabled = app
        .state::<Arc<Mutex<crate::config::AppConfig>>>()
        .lock()
        .unwrap()
        .polish_enabled;

    let menu = build_tray_menu(app, polish_enabled)?;

    TrayIconBuilder::with_id("main-tray")
        .icon(idle_icon())
        .icon_as_template(true)
        .tooltip("Audio Input — Click or ⌘⇧Space to record")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                info!("User quit");
                app.exit(0);
            }
            "settings" => {
                show_settings_window(app);
            }
            "open-log" => {
                let log_path = dirs::cache_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                    .join("com.audioinput.app")
                    .join("app.log");
                #[cfg(target_os = "macos")]
                let _ = std::process::Command::new("open").arg(&log_path).spawn();
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("explorer").arg(&log_path).spawn();
            }
            "toggle-polish" => {
                let config_state = app.state::<Arc<Mutex<crate::config::AppConfig>>>();
                let new_enabled = {
                    let mut cfg = config_state.lock().unwrap();
                    cfg.polish_enabled = !cfg.polish_enabled;
                    cfg.polish_enabled
                };
                {
                    let cfg = config_state.lock().unwrap();
                    let _ = crate::config::AppConfig::save(app, &cfg);
                }
                info!("AI polish: {}", if new_enabled { "on" } else { "off" });
                let _ = app.emit("polish-changed", new_enabled);
                // Rebuild menu to refresh check state
                if let Some(tray) = app.tray_by_id("main-tray") {
                    if let Ok(menu) = build_tray_menu(app, new_enabled) {
                        let _ = tray.set_menu(Some(menu));
                    }
                }
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
                let app = tray.app_handle();
                let _ = app.emit("toggle-recording", ());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn build_tray_menu<R: Runtime>(app: &AppHandle<R>, polish_enabled: bool) -> tauri::Result<Menu<R>> {
    let last     = MenuItem::with_id(app, "last-result", "No transcription yet", false, None::<&str>)?;
    let sep1     = PredefinedMenuItem::separator(app)?;
    let polish   = CheckMenuItem::with_id(app, "toggle-polish", "AI Polish", true, polish_enabled, None::<&str>)?;
    let sep2     = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let open_log = MenuItem::with_id(app, "open-log", "Open Log File", true, None::<&str>)?;
    let sep3     = PredefinedMenuItem::separator(app)?;
    let quit     = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(app, &[&last, &sep1, &polish, &sep2, &settings, &open_log, &sep3, &quit])
}

pub fn set_tray_icon<R: Runtime>(app: &AppHandle<R>, state: &str) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let (icon, as_template) = match state {
            "recording"  => (recording_icon(), false),
            "processing" => (processing_icon(), false),
            "error"      => (error_icon(), false),
            _            => (idle_icon(), true),
        };
        let _ = tray.set_icon(Some(icon));
        let _ = tray.set_icon_as_template(as_template);
    }
}

pub fn set_tray_last_result<R: Runtime>(app: &AppHandle<R>, text: &str) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let display = if text.chars().count() > 40 {
            format!("{}…", text.chars().take(40).collect::<String>())
        } else {
            text.to_string()
        };
        let _ = tray.set_tooltip(Some(format!("Last: {}", display)));
    }
}

fn show_settings_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
        let win2 = win.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(60)).await;
            let _ = win2.emit("show-settings", ());
        });
    }
}

// --- Embedded icons ---

fn idle_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/tray-idle.png")).expect("tray-idle.png corrupted")
}
fn recording_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/tray-recording.png")).expect("tray-recording.png corrupted")
}
fn processing_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/tray-processing.png")).expect("tray-processing.png corrupted")
}
fn error_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/tray-error.png")).expect("tray-error.png corrupted")
}
