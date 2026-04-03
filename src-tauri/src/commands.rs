use crate::{
    audio::{encode_wav, Recorder},
    config::AppConfig,
    input::inject_text,
    state::{AppState, SharedState},
    transcription::{polish, GroqClient},
    tray::set_tray_icon,
};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter as _, Manager, Runtime};
use tracing::{error, info, warn};

pub struct RecorderState(pub Arc<Mutex<Recorder>>);

/// 切换录音状态（Toggle 模式）
/// Idle → Recording → (自动) → Processing → Idle
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
            warn!("正在处理中，请稍候");
        }
        AppState::Error(_) => {
            // 从错误状态恢复，允许重新开始
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
    info!("开始录音...");
    // 先释放 recorder 锁再操作 shared_state，避免嵌套锁
    let result = {
        match recorder_state.lock() {
            Ok(mut recorder) => recorder.start(app),
            Err(e) => {
                error!("recorder 锁中毒: {}", e);
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
            info!("状态 → Recording");
        }
        Err(e) => {
            error!("录音启动失败: {}", e);
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
    // 停止录音
    let audio_data = {
        let mut recorder = recorder_state.lock().unwrap();
        match recorder.stop() {
            Ok(data) => data,
            Err(e) => {
                error!("停止录音失败: {}", e);
                set_error(&app, &shared_state, &e.to_string());
                return;
            }
        }
    };

    // 切换到 Processing 状态
    {
        let mut state = shared_state.lock().unwrap();
        *state = AppState::Processing;
    }
    set_tray_icon(&app, "processing");
    let _ = app.emit("state-change", "processing");
    info!("状态 → Processing");

    // 编码 WAV
    let wav_bytes = match encode_wav(
        &audio_data.samples,
        audio_data.sample_rate,
        audio_data.channels,
    ) {
        Ok(b) => b,
        Err(e) => {
            error!("WAV 编码失败: {}", e);
            set_error(&app, &shared_state, &e.to_string());
            return;
        }
    };

    // 读取 API Key
    let api_key = match get_api_key(&app) {
        Some(k) => {
            info!("API Key 已加载（前8位: {}...）", &k[..k.len().min(8)]);
            k
        }
        None => {
            warn!("未配置 API Key — 请通过托盘菜单「配置 API Key」填入");
            let _ = app.emit("api-key-missing", ());
            set_error(&app, &shared_state, "未配置 Groq API Key");
            return;
        }
    };

    // 调用 Groq API
    let client = GroqClient::new(api_key.clone());
    let raw_text = match client.transcribe(wav_bytes).await {
        Ok(t) => t,
        Err(e) => {
            error!("转录失败: {}", e);
            set_error(&app, &shared_state, &e.to_string());
            return;
        }
    };

    if raw_text.is_empty() {
        warn!("转录结果为空 — 可能静音、麦克风未授权或录音太短");
        reset_to_idle(&app, &shared_state);
        return;
    }

    // 润色（可选）
    let text = {
        let polish_enabled = {
            let config = app.state::<Arc<Mutex<AppConfig>>>();
            let enabled = config.lock().unwrap().polish_enabled;
            enabled
        };
        if polish_enabled {
            info!("润色开关已开启，调用 LLM 润色...");
            polish::polish_text(&raw_text, &api_key).await
        } else {
            raw_text
        }
    };

    // 更新托盘菜单显示最近转录
    crate::tray::set_tray_last_result(&app, &text);

    // 隐藏浮窗（Accessory 激活策略下窗口不持有焦点，无需等待焦点转移）
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }

    // 注入文字
    let _ = app.emit("transcription-result", &text);
    if let Err(e) = inject_text(&text).await {
        error!("文字注入失败: {}", e);
        // 文字已写入剪贴板，通知前端让用户手动粘贴
        if let Some(win) = app.get_webview_window("main") {
            let _ = win.show();
        }
        let _ = app.emit("injection-failed", &text);
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    }

    reset_to_idle(&app, &shared_state);
    info!("状态 → Idle");
}

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

fn get_api_key<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    // 优先环境变量
    if let Ok(key) = std::env::var("GROQ_API_KEY") {
        if !key.is_empty() {
            return Some(key);
        }
    }
    // 其次读持久化配置
    let config = app.state::<Arc<Mutex<AppConfig>>>();
    let cfg = config.lock().unwrap();
    if cfg.api_key.is_empty() {
        None
    } else {
        Some(cfg.api_key.clone())
    }
}

// --- Tauri IPC Commands ---

#[tauri::command]
pub fn open_accessibility_prefs() {
    crate::input::injector::open_accessibility_settings();
}

#[tauri::command]
pub fn get_accessibility_status() -> bool {
    crate::input::injector::check_accessibility_permission()
}

#[tauri::command]
pub async fn save_api_key(
    key: String,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    let updated = {
        let mut cfg = config.lock().unwrap();
        cfg.api_key = key.clone();
        cfg.clone()
    };
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_saved_api_key(
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> String {
    config.lock().unwrap().api_key.clone()
}

#[tauri::command]
pub fn get_app_state(
    shared_state: tauri::State<'_, SharedState>,
) -> String {
    shared_state.lock().unwrap().to_string()
}

// --- Polish commands ---

#[tauri::command]
pub fn get_polish_enabled(
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> bool {
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
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())
}

// --- Audio device commands ---

#[tauri::command]
pub fn list_audio_devices() -> Vec<String> {
    crate::audio::recorder::list_input_devices()
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

// --- Shortcut commands ---

#[tauri::command]
pub fn get_shortcut(
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> String {
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
    // Re-register shortcut
    crate::shortcut::reregister_shortcut(&app, &shortcut).map_err(|e| e.to_string())
}

// --- Autostart commands ---

#[tauri::command]
pub fn get_autostart_enabled(
    app: AppHandle,
) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub async fn save_autostart_enabled(
    enabled: bool,
    app: AppHandle,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    if enabled {
        app.autolaunch().enable().map_err(|e| e.to_string())
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())
    }
}

// --- Onboarding commands ---

#[tauri::command]
pub fn get_onboarding_completed(
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> bool {
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
