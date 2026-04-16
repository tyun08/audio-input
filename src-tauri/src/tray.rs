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
            "devtools" => {
                if let Some(win) = app.get_webview_window("main") {
                    win.open_devtools();
                }
            }
            "open-log" => {
                let log_path = dirs::cache_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                    .join("com.audioinput.app")
                    .join("app.log");
                #[cfg(target_os = "macos")]
                {
                    // Escape single quotes in the path for use inside a single-quoted
                    // shell string: replace ' with '\''
                    let escaped = log_path.to_string_lossy().replace('\'', "'\\''");
                    let script = format!(
                        "tell application \"Terminal\"\n  do script \"tail -f '{}'\"\n  activate\nend tell",
                        escaped
                    );
                    let _ = std::process::Command::new("osascript")
                        .arg("-e")
                        .arg(&script)
                        .spawn();
                }
                #[cfg(target_os = "windows")]
                {
                    // Escape single quotes for PowerShell string (double them up)
                    let escaped = log_path.to_string_lossy().replace('\'', "''");
                    let _ = std::process::Command::new("powershell")
                        .args(["-NoExit", "-Command", &format!("Get-Content '{}' -Wait", escaped)])
                        .spawn();
                }
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
    let settings  = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let open_log  = MenuItem::with_id(app, "open-log", "Open Log File", true, None::<&str>)?;
    let devtools  = MenuItem::with_id(app, "devtools", "Open DevTools", true, None::<&str>)?;
    let sep3      = PredefinedMenuItem::separator(app)?;
    let quit      = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(app, &[&last, &sep1, &polish, &sep2, &settings, &open_log, &devtools, &sep3, &quit])
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
    // Do all native setup synchronously before the window is shown.
    // WebKit may have JS throttled while the window is hidden, so we cannot
    // rely on the TS syncWindow() path to call set_native_opaque first.
    #[cfg(target_os = "macos")]
    {
        use objc::{class, msg_send, sel, sel_impl};
        unsafe {
            let ns_app: *mut objc::runtime::Object =
                msg_send![class!(NSApplication), sharedApplication];

            // Switch to Regular so the window can become key and WebKit renders.
            // NSApplicationActivationPolicyRegular = 0
            let _: () = msg_send![ns_app, setActivationPolicy: 0i64];
            let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];

            // Set every window opaque with the settings background colour
            // so the backing store is correct the moment the window appears.
            let windows: *mut objc::runtime::Object = msg_send![ns_app, windows];
            let count: usize = msg_send![windows, count];
            for i in 0..count {
                let win: *mut objc::runtime::Object =
                    msg_send![windows, objectAtIndex: i];
                let _: () = msg_send![win, setOpaque: true];
                let bg: *mut objc::runtime::Object = msg_send![
                    class!(NSColor),
                    colorWithRed: 0.118f64
                    green: 0.118f64
                    blue: 0.125f64
                    alpha: 1.0f64
                ];
                let _: () = msg_send![win, setBackgroundColor: bg];
                let _: () = msg_send![win, invalidateShadow];
            }
        }
    }

    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
        let _ = win.emit("show-settings", ());
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
