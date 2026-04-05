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
        .tooltip("Audio Input — 点击或 ⌘⇧Space 开始录音")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                info!("用户退出");
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
                let _ = std::process::Command::new("open").arg(&log_path).spawn();
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
                info!("AI 润色: {}", if new_enabled { "开启" } else { "关闭" });
                // 重建菜单以刷新勾选状态
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

fn build_tray_menu<R: Runtime>(app: &AppHandle<R>, polish_enabled: bool) -> tauri::Result<Menu<R>> {
    let last     = MenuItem::with_id(app, "last-result", "尚无转录结果", false, None::<&str>)?;
    let sep1     = PredefinedMenuItem::separator(app)?;
    let polish   = CheckMenuItem::with_id(app, "toggle-polish", "AI 润色", true, polish_enabled, None::<&str>)?;
    let sep2     = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "配置 API Key...", true, None::<&str>)?;
    let open_log = MenuItem::with_id(app, "open-log", "打开日志文件", true, None::<&str>)?;
    let sep3     = PredefinedMenuItem::separator(app)?;
    let quit     = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

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
        let _ = tray.set_tooltip(Some(format!("最近转录: {}", display)));
    }
}

fn show_settings_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
        let win2 = win.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = win2.emit("show-settings", ());
        });
    }
}

// --- 内嵌图标 ---

fn idle_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/tray-idle.png")).expect("tray-idle.png 损坏")
}
fn recording_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/tray-recording.png")).expect("tray-recording.png 损坏")
}
fn processing_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/tray-processing.png")).expect("tray-processing.png 损坏")
}
fn error_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/tray-error.png")).expect("tray-error.png 损坏")
}
