/// Holds the active paste-key observer so it can be stopped from any context.
static PASTE_MONITOR: std::sync::Mutex<Option<crate::paste_monitor::PasteMonitorHandle>> =
    std::sync::Mutex::new(None);

use crate::{
    audio::{encode_wav, Recorder},
    config::AppConfig,
    history::{HistoryEntry, HistoryState},
    input::inject_text,
    screenshot::capture_primary_screen,
    state::{AppState, ScreenshotState, SharedState},
    transcription::{
        gemini, polish, vertex, GeminiClient, GroqClient, LiteLLMClient, VertexClient,
    },
    tray::set_tray_icon,
};
use serde::Serialize;
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

            // Spawn a task that periodically samples the audio buffer and
            // emits an audio-level event so the frontend can animate a live
            // waveform while recording.
            let (buffer_ref, sample_rate) = {
                let recorder = recorder_state.lock().unwrap();
                (recorder.get_buffer_ref(), recorder.sample_rate())
            };
            let app_monitor = app.clone();
            let state_monitor = shared_state.clone();
            tokio::spawn(async move {
                // Window of samples to compute RMS over — sample_rate / 10 = ~100 ms
                let window_size = ((sample_rate as usize) / 10).max(1);
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                    {
                        let state = state_monitor.lock().unwrap();
                        if !matches!(*state, AppState::Recording) {
                            break;
                        }
                    }
                    let level: f32 = {
                        let buf = buffer_ref.lock().unwrap();
                        let len = buf.len();
                        let start = len.saturating_sub(window_size);
                        let recent = &buf[start..];
                        if recent.is_empty() {
                            0.0
                        } else {
                            let sum_sq: f32 = recent.iter().map(|&s| s * s).sum();
                            (sum_sq / recent.len() as f32).sqrt()
                        }
                    };
                    if let Err(e) = app_monitor.emit("audio-level", level) {
                        error!("Failed to emit audio-level: {}", e);
                    }
                }
            });
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

    // Early return: if the microphone captured only silence or background
    // noise, skip encoding and the API call entirely to avoid Whisper
    // hallucinations (e.g. "Thank you.", "You're welcome.").
    if crate::audio::is_silent(&audio_data.samples) {
        info!("No speech detected — skipping transcription (silence)");
        reset_to_idle(&app, &shared_state);
        return;
    }

    {
        let mut state = shared_state.lock().unwrap();
        *state = AppState::Processing;
    }
    set_tray_icon(&app, "processing");
    let _ = app.emit("state-change", "processing");
    info!("State → Processing");

    let sample_rate = audio_data.sample_rate;
    let total_samples = audio_data.samples.len();
    let channels = audio_data.channels.max(1);
    let duration_s = if sample_rate > 0 {
        (total_samples as f32 / channels as f32) / sample_rate as f32
    } else {
        0.0
    };

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

    let provider_name = {
        let config = app.state::<Arc<Mutex<AppConfig>>>();
        let cfg = config.lock().unwrap();
        cfg.provider.clone()
    };

    // Persist audio + pending metadata BEFORE the API call so a crash or
    // failure never loses the user's speech.
    let history_state = app.state::<HistoryState>().inner().clone();
    let session_id = match history_state
        .lock()
        .unwrap()
        .insert_pending(provider_name.clone(), duration_s, &wav_bytes)
    {
        Ok(id) => id,
        Err(e) => {
            warn!("Failed to persist recording to history: {}", e);
            // We still continue — in-memory retry is lost, but the transcription
            // pipeline works. Fall back to a sentinel id.
            String::new()
        }
    };

    run_transcription_pipeline(&app, &shared_state, &history_state, wav_bytes, session_id).await;
}

/// Shared transcription pipeline: provider call → polish → inject.
/// Used by the live stop flow AND the retry flow.
async fn run_transcription_pipeline<R: Runtime>(
    app: &AppHandle<R>,
    shared_state: &SharedState,
    history_state: &HistoryState,
    wav_bytes: Vec<u8>,
    session_id: String,
) {
    let (provider, pcfg, polish_enabled) = read_provider_config(app);

    info!("Using provider: {}", provider);

    // Transcribe
    let raw_text = match transcribe_with_provider(&provider, &pcfg, wav_bytes).await {
        Ok(t) => t,
        Err(e) => {
            let mut msg = e.to_string();
            error!("Transcription failed: {}", msg);
            if !session_id.is_empty() {
                if let Err(err) = history_state
                    .lock()
                    .unwrap()
                    .mark_failed(&session_id, msg.clone())
                {
                    warn!("Failed to update history entry {}: {}", session_id, err);
                }
            } else {
                msg.push_str(
                    "\n\nRecording was not saved for retry (history write failed). Check disk permissions.",
                );
            }
            set_transcription_error(app, shared_state, &session_id, &msg);
            return;
        }
    };

    if raw_text.is_empty() {
        warn!("Transcription empty — possibly silence, mic unauthorized, or recording too short");
        if !session_id.is_empty() {
            let _ = history_state
                .lock()
                .unwrap()
                .mark_completed(&session_id, String::new(), String::new(), false);
        }
        reset_to_idle(app, shared_state);
        return;
    }

    // Polish
    let mut polish_failed = false;
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
        polish_failed = failed;
        if failed {
            let _ = app.emit("polish-failed", ());
        }
        polished
    } else {
        {
            let screenshot_state = app.state::<ScreenshotState>();
            screenshot_state.lock().unwrap().take();
        }
        raw_text.clone()
    };

    if !session_id.is_empty() {
        if let Err(err) = history_state.lock().unwrap().mark_completed(
            &session_id,
            raw_text.clone(),
            text.clone(),
            polish_failed,
        ) {
            warn!("Failed to update history entry {}: {}", session_id, err);
        }
    }

    crate::tray::set_tray_last_result(app, &text);

    let _ = app.emit("transcription-result", &text);
    match inject_text(&text).await {
        Ok(()) => {
            let _ = app.emit("transcription-success", ());
        }
        Err(e) => {
            error!("Text injection failed: {}", e);
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
            }
            // Transition to idle BEFORE emitting injection-failed so the HUD
            // leaves "Transcribing…" immediately and shows the clipboard panel.
            reset_to_idle(app, shared_state);
            let _ = app.emit("injection-failed", &text);
            // Start a passive ⌘V observer so the HUD auto-dismisses when the
            // user manually pastes.
            *PASTE_MONITOR.lock().unwrap() = Some(crate::paste_monitor::start(app.clone()));
            return;
        }
    }

    reset_to_idle(app, shared_state);
    info!("State → Idle");
}

fn read_provider_config<R: Runtime>(
    app: &AppHandle<R>,
) -> (String, serde_json::Value, bool) {
    let config = app.state::<Arc<Mutex<AppConfig>>>();
    let cfg = config.lock().unwrap();
    let p = cfg.provider.clone();
    let mut pc = cfg.get_pcfg(&p);
    if p == "groq" {
        if let Ok(key) = std::env::var("GROQ_API_KEY") {
            if !key.is_empty() {
                pc["api_key"] = serde_json::Value::String(key);
            }
        }
    }
    (p, pc, cfg.polish_enabled)
}

// ---------------------------------------------------------------------------
// Provider dispatch helpers — add a match arm here for each new provider.
// ---------------------------------------------------------------------------

async fn transcribe_with_provider(
    provider: &str,
    config: &serde_json::Value,
    wav_bytes: Vec<u8>,
) -> anyhow::Result<String> {
    match provider {
        "groq" => {
            let api_key = config["api_key"].as_str().unwrap_or("");
            if api_key.is_empty() {
                anyhow::bail!("Groq API Key not configured");
            }
            let model = config["model"]
                .as_str()
                .unwrap_or("whisper-large-v3-turbo")
                .to_string();
            info!("Groq Key prefix: {}...", &api_key[..api_key.len().min(8)]);
            info!("Groq model: {}", model);
            GroqClient::new(api_key.to_string(), model)
                .transcribe(wav_bytes)
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
        "openai" => {
            let api_key = config["api_key"].as_str().unwrap_or("");
            if api_key.is_empty() {
                anyhow::bail!("OpenAI API key not configured");
            }
            let model = config["model"]
                .as_str()
                .unwrap_or("gpt-4o-mini-transcribe")
                .to_string();
            info!("OpenAI model: {}", model);
            LiteLLMClient::new(
                crate::transcription::litellm::DEFAULT_API_BASE.to_string(),
                api_key.to_string(),
                model,
            )
            .transcribe(wav_bytes)
            .await
        }
        "gemini" => {
            let api_key = config["api_key"].as_str().unwrap_or("");
            if api_key.is_empty() {
                anyhow::bail!("Gemini API key not configured");
            }
            let model = config["model"]
                .as_str()
                .unwrap_or("gemini-2.5-flash")
                .to_string();
            info!("Gemini model: {}", model);
            GeminiClient::new(api_key.to_string(), model)?
                .transcribe(wav_bytes)
                .await
        }
        "litellm" => {
            let api_key = config["api_key"].as_str().unwrap_or("");
            if api_key.is_empty() {
                anyhow::bail!("LiteLLM API key not configured");
            }
            let api_base = config["api_base"]
                .as_str()
                .unwrap_or(crate::transcription::litellm::DEFAULT_API_BASE);
            let model = config["model"].as_str().unwrap_or("whisper-1").to_string();
            info!("LiteLLM api_base: {}, model: {}", api_base, model);
            LiteLLMClient::new(api_base.to_string(), api_key.to_string(), model)
                .transcribe(wav_bytes)
                .await
        }
        other => anyhow::bail!("Unsupported provider: {}", other),
    }
}

async fn polish_with_provider(
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
        "openai" => {
            let api_key = config["api_key"].as_str().unwrap_or("");
            if api_key.is_empty() {
                return (text.to_string(), true);
            }
            crate::transcription::litellm::polish_text_litellm(
                text,
                crate::transcription::litellm::DEFAULT_API_BASE,
                api_key,
                screenshot,
            )
            .await
        }
        "gemini" => {
            let api_key = config["api_key"].as_str().unwrap_or("");
            if api_key.is_empty() {
                return (text.to_string(), true);
            }
            let model = config["model"].as_str().unwrap_or("gemini-2.5-flash");
            gemini::polish_text_gemini(text, api_key, model, screenshot).await
        }
        "litellm" => {
            let api_key = config["api_key"].as_str().unwrap_or("");
            if api_key.is_empty() {
                return (text.to_string(), true);
            }
            let api_base = config["api_base"]
                .as_str()
                .unwrap_or(crate::transcription::litellm::DEFAULT_API_BASE);
            crate::transcription::litellm::polish_text_litellm(text, api_base, api_key, screenshot)
                .await
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

#[derive(Serialize, Clone)]
struct TranscriptionError<'a> {
    #[serde(rename = "sessionId")]
    session_id: &'a str,
    message: &'a str,
}

/// Transcription failure: keep the HUD pinned in the error state, emit a
/// retryable event carrying the history session id, and skip auto-recovery.
/// The user reclaims control via the retry button, dismiss button, or the
/// global shortcut.
fn set_transcription_error<R: Runtime>(
    app: &AppHandle<R>,
    shared_state: &SharedState,
    session_id: &str,
    msg: &str,
) {
    {
        let mut state = shared_state.lock().unwrap();
        *state = AppState::Error(msg.to_string());
    }
    set_tray_icon(app, "error");
    // Emit the retryable payload first so the UI sees the session id before
    // reacting to the state-change — otherwise the HUD briefly renders as the
    // small error pill and then resizes into the retry panel.
    let _ = app.emit(
        "transcription-error",
        TranscriptionError {
            session_id,
            message: msg,
        },
    );
    let _ = app.emit("state-change", format!("error:{}", msg));
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

// --- Show idle HUD -----------------------------------------------------------

#[tauri::command]
pub fn get_show_idle_hud(config: tauri::State<'_, Arc<Mutex<AppConfig>>>) -> bool {
    config.lock().unwrap().show_idle_hud
}

#[tauri::command]
pub async fn save_show_idle_hud(
    enabled: bool,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    let updated = {
        let mut cfg = config.lock().unwrap();
        cfg.show_idle_hud = enabled;
        cfg.clone()
    };
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())
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
pub fn set_native_opaque(opaque: bool, visible: bool) {
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

                // Visibility via alphaValue instead of show/hide or offscreen parking.
                // alphaValue=0 keeps the window on-screen so macOS CVDisplayLink keeps
                // firing and the WKWebView compositor never suspends (the root cause of
                // the all-black settings window). setIgnoresMouseEvents prevents an
                // invisible window from eating clicks.
                let alpha: f64 = if visible { 1.0 } else { 0.0 };
                let _: () = msg_send![win, setAlphaValue: alpha];
                let _: () = msg_send![win, setIgnoresMouseEvents: !visible];
                // Apply to all windows, including hidden ones, so the background
                // state is correct the moment the window becomes visible. Skipping
                // hidden windows caused a black-settings-window bug: the call was a
                // no-op while the window was hidden, then show() rendered it with
                // the old transparent/clear background still in place.
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

                // [win contentView] returns WryWebViewParent (a wry container view),
                // not the WKWebView. The actual WKWebView is WryWebViewParent's
                // first subview. Toggle drawsBackground on it so the compositor
                // restarts when switching to opaque/settings mode.
                // transparent:true sets drawsBackground=NO at startup; leaving it NO
                // while the window is hidden suspends the compositor — which is why
                // the window appears all-black even though JS is running fine.
                let content: *mut objc::runtime::Object = msg_send![win, contentView];
                if !content.is_null() {
                    let subviews: *mut objc::runtime::Object = msg_send![content, subviews];
                    let sub_count: usize = msg_send![subviews, count];
                    if sub_count > 0 {
                        let webview: *mut objc::runtime::Object =
                            msg_send![subviews, objectAtIndex: 0usize];
                        let sel = objc::sel!(setDrawsBackground:);
                        let responds: bool = msg_send![webview, respondsToSelector: sel];
                        if responds {
                            let _: () = msg_send![webview, setDrawsBackground: opaque];
                        }
                    }
                }

                // Round the window frame view (superview of contentView)
                // so the opaque background is clipped to rounded corners
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

            // Switch activation policy based on mode:
            // - opaque (settings/onboarding): Regular so the window becomes key,
            //   the app appears in the menu bar, and WebKit renders properly.
            // - transparent (HUD): Accessory so the overlay floats without
            //   stealing focus from the user's current app.
            // NSApplicationActivationPolicyRegular  = 0
            // NSApplicationActivationPolicyAccessory = 1
            if opaque {
                let _: () = msg_send![app, setActivationPolicy: 0i64];
                let _: () = msg_send![app, activateIgnoringOtherApps: true];
            } else {
                let _: () = msg_send![app, setActivationPolicy: 1i64];
            }
        }
    }
}

// ===========================================================================
// History / Retry
// ===========================================================================

#[tauri::command]
pub async fn retry_transcription(
    session_id: String,
    app: AppHandle,
    shared_state: tauri::State<'_, SharedState>,
    history: tauri::State<'_, HistoryState>,
) -> Result<(), String> {
    let shared_state: SharedState = shared_state.inner().clone();
    let history_state: HistoryState = history.inner().clone();

    // Block concurrent retries while recording or already processing.
    {
        let state = shared_state.lock().unwrap();
        if matches!(*state, AppState::Recording | AppState::Processing) {
            return Err("Busy — finish current action before retrying".into());
        }
    }

    let wav_bytes = {
        let store = history_state.lock().unwrap();
        if store.get(&session_id).is_none() {
            return Err(format!("History entry {} not found", session_id));
        }
        store
            .load_wav(&session_id)
            .map_err(|e| format!("Cannot load recording: {}", e))?
    };

    if let Err(e) = history_state.lock().unwrap().mark_pending(&session_id) {
        warn!("Failed to mark history entry pending: {}", e);
    }

    {
        let mut state = shared_state.lock().unwrap();
        *state = AppState::Processing;
    }
    set_tray_icon(&app, "processing");
    let _ = app.emit("state-change", "processing");
    info!("Retrying transcription for session {}", session_id);

    tauri::async_runtime::spawn(async move {
        run_transcription_pipeline(&app, &shared_state, &history_state, wav_bytes, session_id)
            .await;
    });

    Ok(())
}

#[tauri::command]
pub fn dismiss_error(
    app: AppHandle,
    shared_state: tauri::State<'_, SharedState>,
) -> Result<(), String> {
    reset_to_idle(&app, shared_state.inner());
    Ok(())
}

#[derive(Serialize)]
pub struct HistoryEntryDto {
    pub id: String,
    #[serde(rename = "createdAtMs")]
    pub created_at_ms: u64,
    #[serde(rename = "durationS")]
    pub duration_s: f32,
    pub provider: String,
    #[serde(rename = "rawText")]
    pub raw_text: String,
    #[serde(rename = "polishedText")]
    pub polished_text: String,
    pub status: String,
    pub error: Option<String>,
    #[serde(rename = "polishFailed")]
    pub polish_failed: bool,
}

impl From<&HistoryEntry> for HistoryEntryDto {
    fn from(e: &HistoryEntry) -> Self {
        HistoryEntryDto {
            id: e.id.clone(),
            created_at_ms: e.created_at_ms,
            duration_s: e.duration_s,
            provider: e.provider.clone(),
            raw_text: e.raw_text.clone(),
            polished_text: e.polished_text.clone(),
            status: match e.status {
                crate::history::HistoryStatus::Pending => "pending",
                crate::history::HistoryStatus::Completed => "completed",
                crate::history::HistoryStatus::Failed => "failed",
            }
            .to_string(),
            error: e.error.clone(),
            polish_failed: e.polish_failed,
        }
    }
}

#[tauri::command]
pub fn list_history(history: tauri::State<'_, HistoryState>) -> Vec<HistoryEntryDto> {
    let store = history.lock().unwrap();
    // Most recent first for UI convenience.
    store
        .entries()
        .iter()
        .rev()
        .map(HistoryEntryDto::from)
        .collect()
}

#[tauri::command]
pub fn delete_history_entry(
    session_id: String,
    history: tauri::State<'_, HistoryState>,
) -> Result<(), String> {
    history
        .lock()
        .unwrap()
        .delete(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_max_history(config: tauri::State<'_, Arc<Mutex<AppConfig>>>) -> usize {
    config.lock().unwrap().max_history
}

#[tauri::command]
pub async fn save_max_history(
    max: usize,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
    history: tauri::State<'_, HistoryState>,
) -> Result<(), String> {
    let max = max.max(1);
    let updated = {
        let mut cfg = config.lock().unwrap();
        cfg.max_history = max;
        cfg.clone()
    };
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())?;
    history
        .lock()
        .unwrap()
        .set_max_entries(max)
        .map_err(|e| e.to_string())
}


#[tauri::command]
pub fn get_sent_hud_timeout_secs(config: tauri::State<'_, Arc<Mutex<AppConfig>>>) -> u32 {
    config.lock().unwrap().sent_hud_timeout_secs
}

#[tauri::command]
pub async fn save_sent_hud_timeout_secs(
    secs: u32,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    let secs = secs.max(1).min(30);
    let updated = {
        let mut cfg = config.lock().unwrap();
        cfg.sent_hud_timeout_secs = secs;
        cfg.clone()
    };
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())
}


/// Stop the passive ⌘V observer that auto-dismisses the injection-failed HUD.
/// Called from the frontend whenever the HUD is dismissed (Copy Again / Dismiss,
/// or the `paste-detected` event handler).
#[tauri::command]
pub fn stop_paste_monitor() {
    let _ = PASTE_MONITOR.lock().unwrap().take();
}
