use std::sync::{Arc, Mutex};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter as _, Manager, Runtime,
};
use tracing::{info, warn};

/// Prefix used on menu item IDs in the "Recent transcriptions" submenu.
/// The suffix is the history entry id, which the event handler looks up.
const RECENT_ID_PREFIX: &str = "history-copy:";

/// Number of recent completed transcriptions to surface in the tray submenu.
const RECENT_MAX: usize = 8;

/// Max chars to display per submenu item (full text remains on the clipboard).
const RECENT_PREVIEW_CHARS: usize = 48;

struct TrayStrings {
    no_transcription_yet: &'static str,
    recent: &'static str,
    no_history_yet: &'static str,
    ai_polish: &'static str,
    smart_compose: &'static str,
    settings: &'static str,
    open_log: &'static str,
    check_updates: &'static str,
    quit: &'static str,
    tooltip_idle: &'static str,
    last_prefix: &'static str,
    #[cfg(debug_assertions)]
    devtools: &'static str,
}

const STRINGS_EN: TrayStrings = TrayStrings {
    no_transcription_yet: "No transcription yet",
    recent: "Recent",
    no_history_yet: "(no history yet)",
    ai_polish: "AI Polish",
    smart_compose: "Smart Compose",
    settings: "Settings…",
    open_log: "Open Log File",
    check_updates: "Check for Updates…",
    quit: "Quit",
    tooltip_idle: "Audio Input — Click or ⌘⇧Space to record",
    last_prefix: "Last: ",
    #[cfg(debug_assertions)]
    devtools: "Open DevTools",
};

const STRINGS_ZH: TrayStrings = TrayStrings {
    no_transcription_yet: "暂无转录内容",
    recent: "最近转录",
    no_history_yet: "（暂无历史记录）",
    ai_polish: "AI 润色",
    smart_compose: "智能撰写",
    settings: "设置…",
    open_log: "打开日志文件",
    check_updates: "检查更新…",
    quit: "退出",
    tooltip_idle: "Audio Input — 点击或 ⌘⇧Space 录音",
    last_prefix: "最后：",
    #[cfg(debug_assertions)]
    devtools: "打开开发工具",
};

fn strings_for_locale(locale: &str) -> &'static TrayStrings {
    if locale == "zh" { &STRINGS_ZH } else { &STRINGS_EN }
}


pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let (polish_enabled, smart_compose_active, locale) = {
        let config_state = app.state::<Arc<Mutex<crate::config::AppConfig>>>();
        let cfg = config_state.lock().unwrap();
        let sc_active = cfg.transcription_mode == crate::state::TranscriptionMode::SmartCompose;
        (cfg.polish_enabled, sc_active, cfg.locale.clone())
    };

    let menu = build_tray_menu(app, polish_enabled, smart_compose_active, &locale)?;

    let s = strings_for_locale(&locale);
    TrayIconBuilder::with_id("main-tray")
        .icon(idle_icon())
        .icon_as_template(true)
        .tooltip(s.tooltip_idle)
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
            "check-updates" => {
                // Frontend owns the updater UX (dialog → download → relaunch).
                // Make sure the main window is visible so the dialog has a host.
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
                let _ = app.emit("check-for-updates", ());
            }
            #[cfg(debug_assertions)]
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
                refresh_tray_menu(app);
            }
            "toggle-mode" => {
                use crate::state::{SharedMode, TranscriptionMode};
                let config_state = app.state::<Arc<Mutex<crate::config::AppConfig>>>();
                let new_mode = if let Some(mode_state) = app.try_state::<SharedMode>() {
                    let mut m = mode_state.lock().unwrap();
                    *m = m.toggle();
                    m.clone()
                } else {
                    TranscriptionMode::default()
                };
                {
                    let mut cfg = config_state.lock().unwrap();
                    cfg.transcription_mode = new_mode.clone();
                    let _ = crate::config::AppConfig::save(app, &cfg);
                }
                info!("Transcription mode via tray: {}", new_mode);
                let _ = app.emit("mode-changed", new_mode.to_string());
                refresh_tray_menu(app);
            }
            other if other.starts_with(RECENT_ID_PREFIX) => {
                let id = &other[RECENT_ID_PREFIX.len()..];
                let ok = copy_history_entry_to_clipboard(app, id);
                if ok {
                    let _ = app.emit("history-copied", id);
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

pub fn build_tray_menu<R: Runtime>(
    app: &AppHandle<R>,
    polish_enabled: bool,
    smart_compose_active: bool,
    locale: &str,
) -> tauri::Result<Menu<R>> {
    let s = strings_for_locale(locale);

    let last = MenuItem::with_id(
        app,
        "last-result",
        s.no_transcription_yet,
        false,
        None::<&str>,
    )?;
    let sep1 = PredefinedMenuItem::separator(app)?;

    // "Recent transcriptions" submenu — click to copy that entry's text back
    // to the clipboard. Useful when an auto-paste silently dropped the text.
    let recent = build_recent_submenu(app, s)?;

    let sep2 = PredefinedMenuItem::separator(app)?;
    let polish = CheckMenuItem::with_id(
        app,
        "toggle-polish",
        s.ai_polish,
        true,
        polish_enabled,
        None::<&str>,
    )?;
    let smart_compose = CheckMenuItem::with_id(
        app,
        "toggle-mode",
        s.smart_compose,
        true,
        smart_compose_active,
        None::<&str>,
    )?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", s.settings, true, None::<&str>)?;
    let open_log = MenuItem::with_id(app, "open-log", s.open_log, true, None::<&str>)?;
    let check_updates =
        MenuItem::with_id(app, "check-updates", s.check_updates, true, None::<&str>)?;
    let sep4 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", s.quit, true, None::<&str>)?;

    // Disabled info row at the top — version is pulled from CARGO_PKG_VERSION
    // at compile time so it's always accurate. Lets the user confirm which
    // build is installed after an in-app update or brew upgrade. (#75)
    let version_label = format!("Audio Input v{}", env!("CARGO_PKG_VERSION"));
    let version_item =
        MenuItem::with_id(app, "version-info", &version_label, false, None::<&str>)?;
    let sep_top = PredefinedMenuItem::separator(app)?;

    #[cfg(debug_assertions)]
    let devtools = MenuItem::with_id(app, "devtools", s.devtools, true, None::<&str>)?;

    let mut items: Vec<&dyn IsMenuItem<R>> = vec![
        &version_item,
        &sep_top,
        &last,
        &sep1,
        &recent,
        &sep2,
        &polish,
        &smart_compose,
        &sep3,
        &settings,
        &open_log,
        &check_updates,
    ];
    #[cfg(debug_assertions)]
    items.push(&devtools);
    items.push(&sep4);
    items.push(&quit);

    Menu::with_items(app, &items)
}

/// Builds the "Recent ▸" submenu. Picks the most recent N successfully-
/// completed history entries, with the newest first. Each item's ID encodes
/// the history id so the menu-event handler can resolve back to the text.
fn build_recent_submenu<R: Runtime>(app: &AppHandle<R>, s: &TrayStrings) -> tauri::Result<Submenu<R>> {
    use crate::history::{HistoryState, HistoryStatus};

    let mut recent: Vec<(String, String)> = Vec::new(); // (id, preview)
    if let Some(state) = app.try_state::<HistoryState>() {
        if let Ok(store) = state.lock() {
            // entries are stored chronologically; iterate newest-first.
            for entry in store.entries().iter().rev() {
                if entry.status != HistoryStatus::Completed {
                    continue;
                }
                let text = if !entry.polished_text.is_empty() {
                    &entry.polished_text
                } else {
                    &entry.raw_text
                };
                if text.is_empty() {
                    continue;
                }
                recent.push((entry.id.clone(), preview_text(text)));
                if recent.len() >= RECENT_MAX {
                    break;
                }
            }
        }
    }

    let builder = tauri::menu::SubmenuBuilder::with_id(app, "recent-submenu", s.recent);
    if recent.is_empty() {
        let empty = MenuItem::with_id(app, "recent-empty", s.no_history_yet, false, None::<&str>)?;
        builder.item(&empty).build()
    } else {
        let mut items: Vec<MenuItem<R>> = Vec::with_capacity(recent.len());
        for (id, preview) in &recent {
            let item_id = format!("{RECENT_ID_PREFIX}{id}");
            items.push(MenuItem::with_id(app, &item_id, preview, true, None::<&str>)?);
        }
        let mut b = builder;
        for it in &items {
            b = b.item(it);
        }
        b.build()
    }
}

/// Truncate transcription text for tray display: collapse whitespace, cap to
/// RECENT_PREVIEW_CHARS, append "…" when cut.
fn preview_text(s: &str) -> String {
    let collapsed: String = s
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let chars: Vec<char> = collapsed.chars().collect();
    if chars.len() > RECENT_PREVIEW_CHARS {
        let mut out: String = chars[..RECENT_PREVIEW_CHARS].iter().collect();
        out.push('…');
        out
    } else {
        collapsed
    }
}

/// Look up a history entry by id and copy its text to the system clipboard.
/// Returns true on success. Called from the tray menu event handler when the
/// user clicks an item in the "Recent" submenu.
pub fn copy_history_entry_to_clipboard<R: Runtime>(app: &AppHandle<R>, id: &str) -> bool {
    use crate::history::{HistoryState, HistoryStatus};

    let state = match app.try_state::<HistoryState>() {
        Some(s) => s,
        None => {
            warn!("history state not available");
            return false;
        }
    };
    let store = match state.lock() {
        Ok(s) => s,
        Err(_) => {
            warn!("history state lock poisoned");
            return false;
        }
    };
    let entry = match store.get(id) {
        Some(e) if e.status == HistoryStatus::Completed => e,
        _ => {
            warn!("history entry {id} not found or not completed");
            return false;
        }
    };
    let text = if !entry.polished_text.is_empty() {
        entry.polished_text.clone()
    } else {
        entry.raw_text.clone()
    };
    drop(store);

    match arboard::Clipboard::new().and_then(|mut c| c.set_text(text)) {
        Ok(()) => {
            info!("Copied history entry {id} to clipboard via tray");
            true
        }
        Err(e) => {
            warn!("clipboard copy failed: {e}");
            false
        }
    }
}

/// Rebuilds the tray menu (and tooltip) so the "Recent" submenu picks up
/// newly-added history entries, and menu labels reflect the current locale.
/// Cheap — call after each successful transcription or locale change.
pub fn refresh_tray_menu<R: Runtime>(app: &AppHandle<R>) {
    let Some(tray) = app.tray_by_id("main-tray") else {
        return;
    };
    let (polish_enabled, smart_compose_active, locale) = app
        .state::<Arc<Mutex<crate::config::AppConfig>>>()
        .lock()
        .map(|cfg| {
            let sc_active = cfg.transcription_mode == crate::state::TranscriptionMode::SmartCompose;
            (cfg.polish_enabled, sc_active, cfg.locale.clone())
        })
        .unwrap_or((true, false, "en".to_string()));
    let s = strings_for_locale(&locale);
    if let Ok(menu) = build_tray_menu(app, polish_enabled, smart_compose_active, &locale) {
        let _ = tray.set_menu(Some(menu));
    }
    let _ = tray.set_tooltip(Some(s.tooltip_idle));
}

pub fn set_tray_icon<R: Runtime>(app: &AppHandle<R>, state: &str) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let (icon, as_template) = match state {
            "recording" => (recording_icon(), false),
            "processing" => (processing_icon(), false),
            "error" => (error_icon(), false),
            _ => (idle_icon(), true),
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
        let locale = app
            .state::<Arc<Mutex<crate::config::AppConfig>>>()
            .lock()
            .map(|cfg| cfg.locale.clone())
            .unwrap_or_else(|_| "en".to_string());
        let s = strings_for_locale(&locale);
        let _ = tray.set_tooltip(Some(format!("{}{}", s.last_prefix, display)));
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
                let win: *mut objc::runtime::Object = msg_send![windows, objectAtIndex: i];
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
    Image::from_bytes(include_bytes!("../icons/tray-recording.png"))
        .expect("tray-recording.png corrupted")
}
fn processing_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/tray-processing.png"))
        .expect("tray-processing.png corrupted")
}
fn error_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/tray-error.png")).expect("tray-error.png corrupted")
}
