use crate::{
    audio::{encode_wav, Recorder},
    config::AppConfig,
    input::inject_text,
    state::{AppState, SharedState},
    transcription::GroqClient,
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
    let mut recorder = recorder_state.lock().unwrap();
    match recorder.start() {
        Ok(()) => {
            let mut state = shared_state.lock().unwrap();
            *state = AppState::Recording;
            set_tray_icon(app, "recording");
            let _ = app.emit("state-change", "recording");
            info!("状态 → Recording");
        }
        Err(e) => {
            error!("录音启动失败: {}", e);
            let mut state = shared_state.lock().unwrap();
            *state = AppState::Error(e.to_string());
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
    let client = GroqClient::new(api_key);
    let text = match client.transcribe(wav_bytes).await {
        Ok(t) => t,
        Err(e) => {
            error!("转录失败: {}", e);
            set_error(&app, &shared_state, &e.to_string());
            return;
        }
    };

    if text.is_empty() {
        warn!("转录结果为空 — 可能静音、麦克风未授权或录音太短");
        reset_to_idle(&app, &shared_state);
        return;
    }

    // 注入前先隐藏浮窗，让焦点还给用户的目标输入框
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    // 等待焦点切换完成
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // 注入文字
    let _ = app.emit("transcription-result", &text);
    if let Err(e) = inject_text(&text).await {
        error!("文字注入失败: {}", e);
        // 检查是否是权限问题，引导用户去开启
        if !crate::input::injector::check_accessibility_permission() {
            crate::input::injector::open_accessibility_settings();
            let _ = app.emit("accessibility-missing", ());
        }
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
pub async fn save_api_key(
    key: String,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    {
        let mut cfg = config.lock().unwrap();
        cfg.api_key = key.clone();
    }
    AppConfig::save(&app, &AppConfig { api_key: key })
        .map_err(|e| e.to_string())
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
