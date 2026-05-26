#[cfg(target_os = "macos")]
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

pub mod audio;
mod commands;
mod config;
mod history;
mod input;
#[cfg(target_os = "macos")]
mod macos_shortcut;
mod paste_monitor;
mod screenshot;
mod shortcut;
mod state;
mod transcription;
mod tray;

use audio::Recorder;
use commands::RecorderState;
use config::AppConfig;
use history::{history_dir, new_history_state};
use state::{new_screenshot_state, new_shared_mode, new_shared_state};

use std::sync::{Arc, Mutex};
use tauri::{Listener as _, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tracing::{info, warn};

pub fn run() {
    // Try current dir, then parent dir, to find .env in dev mode
    if dotenvy::dotenv().is_err() {
        let _ = dotenvy::from_path("../.env");
    }

    // Write to both stderr and a log file
    // In packaged builds stderr is invisible; the log file is the only debug channel
    let log_path = {
        let mut p = dirs::cache_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        p.push("com.audioinput.app");
        std::fs::create_dir_all(&p).ok();
        p.push("app.log");
        p
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "audio_input=debug".parse().unwrap());

    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
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
    info!("Log file: {:?}", log_path);

    info!("Audio Input starting");

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .setup(|app| {
            let handle = app.handle().clone();

            // macOS: Set activation policy to Accessory (tray-only app, windows don't steal focus)
            #[cfg(target_os = "macos")]
            {
                use objc::{class, msg_send, sel, sel_impl};
                unsafe {
                    let ns_app: *mut objc::runtime::Object =
                        msg_send![class!(NSApplication), sharedApplication];
                    // NSApplicationActivationPolicyAccessory = 1
                    let _: () = msg_send![ns_app, setActivationPolicy: 1i64];

                    // Start the window fully transparent and input-passthrough.
                    // The TS side calls show() right away, which puts the window
                    // on-screen and starts the CVDisplayLink — keeping the WKWebView
                    // compositor warm even while the HUD is "hidden". When the user
                    // opens settings, setNativeOpaque(opaque=true, visible=true) will
                    // restore alphaValue=1 and disable ignoresMouseEvents.
                    let windows: *mut objc::runtime::Object = msg_send![ns_app, windows];
                    let count: usize = msg_send![windows, count];
                    for i in 0..count {
                        let win: *mut objc::runtime::Object =
                            msg_send![windows, objectAtIndex: i];
                        let _: () = msg_send![win, setAlphaValue: 0.0f64];
                        let _: () = msg_send![win, setIgnoresMouseEvents: true];
                    }
                }
            }

            // macOS: pre-load AVFoundation so AVCaptureDevice is available when
            // request_microphone_permission is called from the frontend (after run loop starts).
            #[cfg(target_os = "macos")]
            {
                use objc::runtime::Class;
                unsafe {
                    extern "C" {
                        fn dlopen(
                            filename: *const std::ffi::c_char,
                            flag: std::ffi::c_int,
                        ) -> *mut std::ffi::c_void;
                    }
                    const RTLD_LAZY: std::ffi::c_int = 0x1;
                    const RTLD_GLOBAL: std::ffi::c_int = 0x8;
                    let av_lib = dlopen(
                        c"/System/Library/Frameworks/AVFoundation.framework/AVFoundation".as_ptr(),
                        RTLD_LAZY | RTLD_GLOBAL,
                    );
                    if av_lib.is_null() {
                        tracing::warn!("dlopen AVFoundation failed");
                    } else {
                        tracing::info!("AVFoundation loaded via dlopen");
                        if Class::get("AVCaptureDevice").is_some() {
                            tracing::info!("AVCaptureDevice class ready");
                        }
                    }
                }
            }

            // Load persisted config
            let config = AppConfig::load(&handle);
            let shortcut_str = config.shortcut.clone();
            let max_history = config.max_history;
            let initial_mode = config.transcription_mode.clone();
            app.manage(Arc::new(Mutex::new(config)));

            // Init transcription mode
            app.manage(new_shared_mode(initial_mode));

            // Init shared state
            let shared_state = new_shared_state();
            app.manage(shared_state.clone());

            // Init screenshot context state
            app.manage(new_screenshot_state());

            // Init history store (audio + metadata persistence for retry)
            let app_data = handle
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let hist = new_history_state(history_dir(&app_data), max_history);
            app.manage(hist);

            // Init recorder
            let recorder = Arc::new(Mutex::new(Recorder::new()));
            app.manage(RecorderState(Arc::clone(&recorder)));

            // Setup system tray
            tray::setup_tray(&handle)?;

            // Diagnostics: binary path and accessibility status
            info!("Binary path: {:?}", std::env::current_exe().unwrap_or_default());
            if input::injector::check_accessibility_permission() {
                info!("Accessibility permission: granted ✓");
            } else {
                warn!("Accessibility permission: denied (AXIsProcessTrusted=false)");
            }

            // Register global shortcut (from config, default Meta+Shift+Space).
            //
            // On macOS we use a CGEventTap at the HID level so our shortcut fires
            // even when another app (e.g. 1Password) has registered the same combo
            // via a session-level event tap.  Falls back to Tauri's Carbon-based
            // global-shortcut plugin if CGEventTap is unavailable.
            {
                use tauri::Emitter as _;

                #[cfg(target_os = "macos")]
                let hid_ok = {
                    // CGEventTap requires Accessibility permission. Calling it
                    // before the user has granted Accessibility causes macOS to
                    // silently add the app to the Accessibility list, which is
                    // an unwanted UX side-effect on first launch. Skip the
                    // HID-level tap until permission is in place — Carbon
                    // hotkey works fine as a baseline.
                    if !input::injector::check_accessibility_permission() {
                        info!(
                            "Accessibility not granted yet — using Carbon hotkey only (CGEventTap will be installed after permission is granted)"
                        );
                        false
                    } else {
                        let h = handle.clone();
                        let ss = shared_state.clone();
                        let rec = Arc::clone(&recorder);
                        match macos_shortcut::install(&shortcut_str, move || {
                            let app = h.clone();
                            let state = ss.clone();
                            let r = Arc::clone(&rec);
                            tauri::async_runtime::spawn(async move {
                                commands::toggle_recording(app, state, r).await;
                            });
                        }) {
                            Ok(sh) => {
                                app.manage(sh);
                                info!(
                                    "Global shortcut {} registered via CGEventTap (HID-level, overrides other apps)",
                                    shortcut_str
                                );
                                true
                            }
                            Err(e) => {
                                warn!(
                                    "CGEventTap shortcut failed: {} — falling back to Carbon hotkey",
                                    e
                                );
                                false
                            }
                        }
                    }
                };

                #[cfg(not(target_os = "macos"))]
                let hid_ok = false;

                if !hid_ok {
                    let _ = handle.global_shortcut().unregister_all();

                    let handle2 = handle.clone();
                    let shared_state2 = shared_state.clone();
                    let recorder2 = Arc::clone(&recorder);

                    let sc = shortcut::parse_shortcut(&shortcut_str)
                        .unwrap_or_else(|_| {
                            use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};
                            Shortcut::new(Some(Modifiers::META | Modifiers::SHIFT), Code::Space)
                        });

                    match handle.global_shortcut().on_shortcut(
                        sc,
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
                    ) {
                        Ok(_) => info!("Global shortcut {} registered (Carbon hotkey)", shortcut_str),
                        Err(e) => {
                            warn!("Global shortcut {} registration failed ({}), change it in Settings", shortcut_str, e);
                            let _ = handle.emit("shortcut-conflict", shortcut_str.clone());
                        }
                    }
                }
            }

            // Listen for toggle event from tray click
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
            commands::get_app_state,
            commands::open_accessibility_prefs,
            commands::get_accessibility_status,
            commands::get_provider,
            commands::save_provider,
            commands::get_provider_config,
            commands::save_provider_config,
            commands::check_provider_status,
            commands::get_polish_enabled,
            commands::save_polish_enabled,
            commands::get_ai_action_config,
            commands::get_ai_action_defaults,
            commands::save_ai_action_config,
            commands::list_audio_devices,
            commands::get_preferred_device,
            commands::save_preferred_device,
            commands::get_shortcut,
            commands::save_shortcut,
            commands::get_autostart_enabled,
            commands::save_autostart_enabled,
            commands::get_onboarding_completed,
            commands::save_onboarding_completed,
            commands::get_screenshot_context_enabled,
            commands::save_screenshot_context_enabled,
            commands::get_show_idle_hud,
            commands::save_show_idle_hud,
            commands::set_native_opaque,
            commands::retry_transcription,
            commands::dismiss_error,
            commands::list_history,
            commands::delete_history_entry,
            commands::get_max_history,
            commands::save_max_history,
            commands::get_sent_hud_timeout_secs,
            commands::save_sent_hud_timeout_secs,
            commands::stop_paste_monitor,
            commands::get_microphone_status,
            commands::get_locale,
            commands::save_locale,
            commands::open_microphone_prefs,
            commands::request_microphone_permission,
            commands::get_transcription_mode,
            commands::toggle_transcription_mode,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start Tauri application");
}
