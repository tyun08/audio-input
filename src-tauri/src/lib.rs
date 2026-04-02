mod audio;
mod commands;
mod config;
mod input;
mod state;
mod transcription;
mod tray;

use audio::Recorder;
use commands::RecorderState;
use config::AppConfig;
use state::new_shared_state;

use std::sync::{Arc, Mutex};
use tauri::{Emitter as _, Listener as _, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing::info;

pub fn run() {
    // 依次尝试当前目录、父目录（src-tauri 的上级），确保 dev 模式能找到 .env
    if dotenvy::dotenv().is_err() {
        let _ = dotenvy::from_path("../.env");
    }

    // 同时写 stderr 和 ~/Library/Logs/com.audioinput.app/app.log
    // 打包后 stderr 不可见，日志文件是唯一的调试手段
    let log_path = {
        let mut p = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        p.push("com.audioinput.app");
        std::fs::create_dir_all(&p).ok();
        p.push("app.log");
        p
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "audio_input=debug".parse().unwrap());

    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true).write(true).truncate(true)
        .open(&log_path)
    {
        use tracing_subscriber::prelude::*;
        let file_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file));
        let stderr_layer = tracing_subscriber::fmt::layer();
        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .with(stderr_layer)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
    info!("日志文件: {:?}", log_path);

    info!("Audio Input 启动");

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .setup(|app| {
            let handle = app.handle().clone();

            // 初始化持久化配置
            let config = AppConfig::load(&handle);
            app.manage(Arc::new(Mutex::new(config)));

            // 初始化应用状态
            let shared_state = new_shared_state();
            app.manage(shared_state.clone());

            // 初始化录音器
            let recorder = Arc::new(Mutex::new(Recorder::new()));
            app.manage(RecorderState(Arc::clone(&recorder)));

            // 设置系统托盘
            tray::setup_tray(&handle)?;

            // 注册全局快捷键 Cmd+Shift+Space
            {
                let handle2 = handle.clone();
                let shared_state2 = shared_state.clone();
                let recorder2 = Arc::clone(&recorder);

                let shortcut = Shortcut::new(
                    Some(Modifiers::META | Modifiers::SHIFT),
                    Code::Space,
                );

                handle.global_shortcut().on_shortcut(
                    shortcut,
                    move |_app, _shortcut, event| {
                        if event.state() == ShortcutState::Pressed {
                            let app = handle2.clone();
                            let state = shared_state2.clone();
                            let rec = Arc::clone(&recorder2);
                            tauri::async_runtime::spawn(async move {
                                commands::toggle_recording(app, state, rec).await;
                            });
                        }
                    },
                )?;

                info!("全局快捷键 ⌘⇧Space 注册成功");
            }

            // 监听来自托盘点击的 toggle 事件
            {
                let handle3 = handle.clone();
                let shared_state3 = shared_state.clone();
                let recorder3 = Arc::clone(&recorder);

                handle.listen("toggle-recording", move |_event| {
                    let app = handle3.clone();
                    let state = shared_state3.clone();
                    let rec = Arc::clone(&recorder3);
                    tauri::async_runtime::spawn(async move {
                        commands::toggle_recording(app, state, rec).await;
                    });
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::save_api_key,
            commands::get_saved_api_key,
            commands::get_app_state,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}
