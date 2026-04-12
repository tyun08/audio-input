use crate::{
    audio::{encode_wav, Recorder},
    config::AppConfig,
    input::inject_text,
    screenshot::capture_primary_screen,
    state::{AppState, ScreenshotState, SharedState},
    transcription::{polish, vertex, GroqClient, VertexClient},
    tray::set_tray_icon,
};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter as _, Manager, Runtime};
use tracing::{error, info, warn};

pub struct RecorderState(pub Arc<Mutex<Recorder>>);

// ---------------------------------------------------------------------------
// Toggle recording (unchanged API)
// ---------------------------------------------------------------------------

pub async fn toggle_recording<R: Runtime>(
    app: AppHandle<R>,
    shared_state: SharedState,
    recorder_state: Arc<Mutex<Recorder>>,
) {
    let current = {
        let state = shared_state.lock().unwrap();
        state.clone()
    };

    match current {
        AppState::Idle => {
            start_recording(&app, shared_state, recorder_state).await;
        }
        AppState::Recording => {
            stop_and_transcribe(app, shared_state, recorder_state).await;
        }
        AppState::Processing => {
            warn!("Still processing, please wait");
        }
        AppState::Error(_) => {
            let mut state = shared_state.lock().unwrap();
            *state = AppState::Idle;
            set_tray_icon(&app, "idle");
            let _ = app.emit("state-change", "idle");
        }
    }
}

async fn start_recording<R: Runtime>(
    app: &AppHandle<R>,
    shared_state: SharedState,
    recorder_state: Arc<Mutex<Recorder>>,
) {
    info!("Recording started...");

    let result = {
        match recorder_state.lock() {
            Ok(mut recorder) => recorder.start(app),
            Err(e) => {
                error!("Recorder lock poisoned: {}", e);
                return;
            }
        }
    };

    match result {
        Ok(()) => {
            let mut state = shared_state.lock().unwrap();
            *state = AppState::Recording;
            drop(state);
            set_tray_icon(app, "recording");
            let _ = app.emit("state-change", "recording");
            info!("State → Recording");

            let screenshot_context_enabled = {
                let config = app.state::<Arc<Mutex<AppConfig>>>();
                let enabled = config.lock().unwrap().screenshot_context_enabled;
                enabled
            };
            let ss = app.state::<ScreenshotState>().inner().clone();
            if screenshot_context_enabled {
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                    tokio::task::spawn_blocking(move || {
                        let shot = capture_primary_screen();
                        if shot.is_some() {
                            info!("Screenshot captured for polish context");
                        } else {
                            info!("Screenshot capture failed, using text-only polish");
                        }
                        *ss.lock().unwrap() = shot;
                    });
                });
            } else {
                *ss.lock().unwrap() = None;
            }
        }
        Err(e) => {
            error!("Recording start failed: {}", e);
            let mut state = shared_state.lock().unwrap();
            *state = AppState::Error(e.to_string());
            drop(state);
            set_tray_icon(app, "error");
            let _ = app.emit("state-change", format!("error:{}", e));
            schedule_error_recovery(app.clone(), shared_state.clone());
        }
    }
}

async fn stop_and_transcribe<R: Runtime>(
    app: AppHandle<R>,
    shared_state: SharedState,
    recorder_state: Arc<Mutex<Recorder>>,
) {
    let audio_data = {
        let mut recorder = recorder_state.lock().unwrap();
        match recorder.stop() {
            Ok(data) => data,
            Err(e) => {
                error!("Recording stop failed: {}", e);
                set_error(&app, &shared_state, &e.to_string());
                return;
            }
        }
    };

    {
        let mut state = shared_state.lock().unwrap();
        *state = AppState::Processing;
    }
    set_tray_icon(&app, "processing");
    let _ = app.emit("state-change", "processing");
    info!("State → Processing");

    let wav_bytes = match encode_wav(
        &audio_data.samples,
        audio_data.sample_rate,
        audio_data.channels,
    ) {
        Ok(b) => b,
        Err(e) => {
            error!("WAV encoding failed: {}", e);
            set_error(&app, &shared_state, &e.to_string());
            return;
        }
    };

    // Read provider + config
    let (provider, pcfg, polish_enabled) = {
        let config = app.state::<Arc<Mutex<AppConfig>>>();
        let cfg = config.lock().unwrap();
        let p = cfg.provider.clone();
        let mut pc = cfg.get_pcfg(&p);
        // Groq: env var override
        if p == "groq" {
            if let Ok(key) = std::env::var("GROQ_API_KEY") {
                if !key.is_empty() {
                    pc["api_key"] = serde_json::Value::String(key);
                }
            }
        }
        (p, pc, cfg.polish_enabled)
    };

    info!("Using provider: {}", provider);

    // Load vocabulary for Whisper context injection
    let vocabulary = load_vocabulary(&app);

    // Transcribe
    let raw_text = match transcribe_with_provider(&provider, &pcfg, wav_bytes, vocabulary.as_deref()).await {
        Ok(t) => t,
        Err(e) => {
            error!("Transcription failed: {}", e);
            let _ = app.emit("show-settings", ());
            set_error(&app, &shared_state, &e.to_string());
            return;
        }
    };

    if raw_text.is_empty() {
        warn!("Transcription empty — possibly silence, mic unauthorized, or recording too short");
        reset_to_idle(&app, &shared_state);
        return;
    }

    // Polish
    let text = if polish_enabled {
        let screenshot = {
            let screenshot_state = app.state::<ScreenshotState>();
            let shot = screenshot_state.lock().unwrap().take();
            shot
        };
        info!(
            "Polish enabled, calling LLM polish (screenshot context: {})...",
            if screenshot.is_some() { "yes" } else { "no" }
        );
        let (polished, failed) =
            polish_with_provider(&provider, &pcfg, &raw_text, screenshot.as_deref()).await;
        if failed {
            let _ = app.emit("polish-failed", ());
        }
        polished
    } else {
        {
            let screenshot_state = app.state::<ScreenshotState>();
            screenshot_state.lock().unwrap().take();
        }
        raw_text
    };

    crate::tray::set_tray_last_result(&app, &text);

    let _ = app.emit("transcription-result", &text);
    if let Err(e) = inject_text(&text).await {
        error!("Text injection failed: {}", e);
        if let Some(win) = app.get_webview_window("main") {
            let _ = win.show();
        }
        let _ = app.emit("injection-failed", &text);
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    }

    reset_to_idle(&app, &shared_state);
    info!("State → Idle");
}

// ---------------------------------------------------------------------------
// Provider dispatch helpers — add a match arm here for each new provider.
// ---------------------------------------------------------------------------

async fn transcribe_with_provider(
    provider: &str,
    config: &serde_json::Value,
    wav_bytes: Vec<u8>,
    vocabulary: Option<&str>,
) -> anyhow::Result<String> {
    match provider {
        "groq" => {
            let api_key = config["api_key"].as_str().unwrap_or("");
            if api_key.is_empty() {
                anyhow::bail!("Groq API Key not configured");
            }
            info!("Groq Key prefix: {}...", &api_key[..api_key.len().min(8)]);
            GroqClient::new(api_key.to_string())
                .transcribe(wav_bytes, vocabulary)
                .await
        }
        "vertex_ai" => {
            let project_id = config["project_id"].as_str().unwrap_or("");
            if project_id.is_empty() {
                anyhow::bail!("Vertex AI project ID not configured");
            }
            let location = config["location"].as_str().unwrap_or("us-central1");
            let model = config["model"].as_str().unwrap_or("gemini-2.5-flash");
            info!(
                "Vertex AI: project={}, region={}, model={}",
                project_id, location, model
            );
            let client =
                VertexClient::new(project_id.into(), location.into(), model.into()).await?;
            client.transcribe(wav_bytes).await
        }
        other => anyhow::bail!("Unsupported provider: {}", other),
    }
}

pub async fn polish_with_provider(
    provider: &str,
    config: &serde_json::Value,
    text: &str,
    screenshot: Option<&str>,
) -> (String, bool) {
    match provider {
        "groq" => {
            let api_key = config["api_key"].as_str().unwrap_or("");
            polish::polish_text(text, api_key, screenshot).await
        }
        "vertex_ai" => {
            let project_id = config["project_id"].as_str().unwrap_or("");
            let location = config["location"].as_str().unwrap_or("us-central1");
            let model = config["model"].as_str().unwrap_or("gemini-2.5-flash");
            vertex::polish_text_vertex(text, project_id, location, model, screenshot).await
        }
        _ => (text.to_string(), true),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn set_error<R: Runtime>(app: &AppHandle<R>, shared_state: &SharedState, msg: &str) {
    {
        let mut state = shared_state.lock().unwrap();
        *state = AppState::Error(msg.to_string());
    }
    set_tray_icon(app, "error");
    let _ = app.emit("state-change", format!("error:{}", msg));
    schedule_error_recovery(app.clone(), shared_state.clone());
}

fn reset_to_idle<R: Runtime>(app: &AppHandle<R>, shared_state: &SharedState) {
    {
        let mut state = shared_state.lock().unwrap();
        *state = AppState::Idle;
    }
    set_tray_icon(app, "idle");
    let _ = app.emit("state-change", "idle");
}

fn schedule_error_recovery<R: Runtime>(app: AppHandle<R>, shared_state: SharedState) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        let mut state = shared_state.lock().unwrap();
        if matches!(*state, AppState::Error(_)) {
            *state = AppState::Idle;
            set_tray_icon(&app, "idle");
            let _ = app.emit("state-change", "idle");
        }
    });
}

// ===========================================================================
// Tauri IPC Commands
// ===========================================================================

#[tauri::command]
pub fn open_accessibility_prefs() {
    crate::input::injector::open_accessibility_settings();
}

#[tauri::command]
pub fn get_accessibility_status() -> bool {
    crate::input::injector::check_accessibility_permission()
}

#[tauri::command]
pub fn get_app_state(shared_state: tauri::State<'_, SharedState>) -> String {
    shared_state.lock().unwrap().to_string()
}

// --- Generic provider commands -----------------------------------------------

#[tauri::command]
pub fn get_provider(config: tauri::State<'_, Arc<Mutex<AppConfig>>>) -> String {
    config.lock().unwrap().provider.clone()
}

#[tauri::command]
pub async fn save_provider(
    provider: String,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    let updated = {
        let mut cfg = config.lock().unwrap();
        cfg.provider = provider;
        cfg.clone()
    };
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_provider_config(
    provider: String,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> serde_json::Value {
    config.lock().unwrap().get_pcfg(&provider)
}

#[tauri::command]
pub async fn save_provider_config(
    provider: String,
    config_values: serde_json::Value,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    let updated = {
        let mut cfg = config.lock().unwrap();
        cfg.provider_configs.insert(provider, config_values);
        cfg.clone()
    };
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_provider_status(provider: String) -> bool {
    match provider.as_str() {
        "vertex_ai" => vertex::check_adc_available(),
        _ => true,
    }
}

// --- Polish ------------------------------------------------------------------

#[tauri::command]
pub fn get_polish_enabled(config: tauri::State<'_, Arc<Mutex<AppConfig>>>) -> bool {
    config.lock().unwrap().polish_enabled
}

#[tauri::command]
pub async fn save_polish_enabled(
    enabled: bool,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    let updated = {
        let mut cfg = config.lock().unwrap();
        cfg.polish_enabled = enabled;
        cfg.clone()
    };
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())?;
    // Sync tray menu checkbox
    if let Some(tray) = app.tray_by_id("main-tray") {
        if let Ok(menu) = crate::tray::build_tray_menu(&app, enabled) {
            let _ = tray.set_menu(Some(menu));
        }
    }
    Ok(())
}

// --- Audio devices -----------------------------------------------------------

#[tauri::command]
pub fn list_audio_devices() -> Vec<String> {
    crate::audio::recorder::list_input_devices()
}

#[tauri::command]
pub fn get_preferred_device(
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Option<String> {
    config.lock().unwrap().preferred_device.clone()
}

#[tauri::command]
pub async fn save_preferred_device(
    device: Option<String>,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    let updated = {
        let mut cfg = config.lock().unwrap();
        cfg.preferred_device = device;
        cfg.clone()
    };
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())
}

// --- Shortcut ----------------------------------------------------------------

#[tauri::command]
pub fn get_shortcut(config: tauri::State<'_, Arc<Mutex<AppConfig>>>) -> String {
    config.lock().unwrap().shortcut.clone()
}

#[tauri::command]
pub async fn save_shortcut(
    shortcut: String,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    let updated = {
        let mut cfg = config.lock().unwrap();
        cfg.shortcut = shortcut.clone();
        cfg.clone()
    };
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())?;
    crate::shortcut::reregister_shortcut(&app, &shortcut).map_err(|e| e.to_string())
}

// --- Autostart ---------------------------------------------------------------

#[tauri::command]
pub fn get_autostart_enabled(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub async fn save_autostart_enabled(enabled: bool, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    if enabled {
        app.autolaunch().enable().map_err(|e| e.to_string())
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())
    }
}

// --- Vocabulary --------------------------------------------------------------

fn vocabulary_path<R: tauri::Runtime>(app: &AppHandle<R>) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("vocabulary.json")
}

/// Loads vocabulary from disk and formats it as a comma-separated string for
/// the Whisper `prompt` parameter.  Returns `None` if the file is absent or empty.
fn load_vocabulary<R: tauri::Runtime>(app: &AppHandle<R>) -> Option<String> {
    let path = vocabulary_path(app);
    let data = std::fs::read_to_string(&path).ok()?;
    let words: Vec<String> = serde_json::from_str(&data).ok()?;
    if words.is_empty() {
        return None;
    }
    Some(words.join(", "))
}

#[tauri::command]
pub fn get_vocabulary(app: AppHandle) -> Vec<String> {
    let path = vocabulary_path(&app);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub async fn save_vocabulary(app: AppHandle, words: Vec<String>) -> Result<(), String> {
    let path = vocabulary_path(&app);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(&words).map_err(|e| e.to_string())?;
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    info!("Vocabulary saved: {} words", words.len());
    Ok(())
}

// --- Screenshot context ------------------------------------------------------

#[tauri::command]
pub fn get_screenshot_context_enabled(config: tauri::State<'_, Arc<Mutex<AppConfig>>>) -> bool {
    config.lock().unwrap().screenshot_context_enabled
}

#[tauri::command]
pub async fn save_screenshot_context_enabled(
    enabled: bool,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    let updated = {
        let mut cfg = config.lock().unwrap();
        cfg.screenshot_context_enabled = enabled;
        cfg.clone()
    };
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())
}

// --- Onboarding --------------------------------------------------------------

#[tauri::command]
pub fn get_onboarding_completed(config: tauri::State<'_, Arc<Mutex<AppConfig>>>) -> bool {
    config.lock().unwrap().onboarding_completed
}

#[tauri::command]
pub async fn save_onboarding_completed(
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    let updated = {
        let mut cfg = config.lock().unwrap();
        cfg.onboarding_completed = true;
        cfg.clone()
    };
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())
}

/// Toggle NSWindow.isOpaque at runtime so WebKit uses the high-quality opaque
/// text rendering path when showing panels, and transparent mode for the HUD.
#[tauri::command]
pub fn set_native_opaque(opaque: bool) {
    #[cfg(target_os = "macos")]
    {
        use objc::{class, msg_send, sel, sel_impl};
        unsafe {
            let app: *mut objc::runtime::Object =
                msg_send![class!(NSApplication), sharedApplication];
            let windows: *mut objc::runtime::Object = msg_send![app, windows];
            let count: usize = msg_send![windows, count];
            for i in 0..count {
                let win: *mut objc::runtime::Object =
                    msg_send![windows, objectAtIndex: i];
                let is_visible: bool = msg_send![win, isVisible];
                if !is_visible {
                    continue;
                }
                let _: () = msg_send![win, setOpaque: opaque];
                let bg: *mut objc::runtime::Object = if opaque {
                    msg_send![
                        class!(NSColor),
                        colorWithRed: 0.118f64
                        green: 0.118f64
                        blue: 0.125f64
                        alpha: 1.0f64
                    ]
                } else {
                    msg_send![class!(NSColor), clearColor]
                };
                let _: () = msg_send![win, setBackgroundColor: bg];

                // Round the window frame view (superview of contentView)
                // so the opaque background is clipped to rounded corners
                let content: *mut objc::runtime::Object = msg_send![win, contentView];
                let frame_view: *mut objc::runtime::Object = msg_send![content, superview];
                if !frame_view.is_null() {
                    let _: () = msg_send![frame_view, setWantsLayer: true];
                    let layer: *mut objc::runtime::Object = msg_send![frame_view, layer];
                    if !layer.is_null() {
                        if opaque {
                            let _: () = msg_send![layer, setCornerRadius: 16.0f64];
                            let _: () = msg_send![layer, setMasksToBounds: true];
                        } else {
                            let _: () = msg_send![layer, setCornerRadius: 0.0f64];
                            let _: () = msg_send![layer, setMasksToBounds: false];
                        }
                    }
                }

                let _: () = msg_send![win, invalidateShadow];
            }
        }
    }
}
