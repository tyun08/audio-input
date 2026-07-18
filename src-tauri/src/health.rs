//! Aggregated service-health check surfaced on the tray icon and the health
//! popover: microphone permission, microphone device presence, Accessibility
//! permission, and transcription API readiness.
//!
//! All checks here are cheap/synchronous so they can run from the tray
//! icon-refresh timer and the left-click handler without blocking the UI
//! thread.

use crate::config::AppConfig;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub mic_ok: bool,
    pub mic_found: bool,
    pub ax_ok: bool,
    pub api_ok: bool,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        self.mic_ok && self.mic_found && self.ax_ok && self.api_ok
    }
}

/// Microphone TCC authorization only (does not enumerate devices).
#[cfg(target_os = "macos")]
pub fn mic_permission_ok() -> bool {
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let media_type: *mut objc::runtime::Object =
            msg_send![class!(NSString), stringWithUTF8String: c"soun".as_ptr()];
        let status: i64 =
            msg_send![class!(AVCaptureDevice), authorizationStatusForMediaType: media_type];
        status == 3
    }
}

#[cfg(not(target_os = "macos"))]
pub fn mic_permission_ok() -> bool {
    true
}

/// Whether at least one input device is available. Only safe to call once
/// microphone permission is granted — enumerating devices before that (via
/// cpal's `host.input_devices()`) triggers the macOS microphone TCC prompt,
/// which we deliberately defer until onboarding. When permission is missing
/// we report `false` without probing, since recording cannot work either way.
pub fn mic_device_found(mic_ok: bool) -> bool {
    if !mic_ok {
        return false;
    }
    !crate::audio::recorder::list_input_devices().is_empty()
}

/// Best-effort check that the selected transcription provider has enough
/// configuration to actually be usable (not merely selected).
fn provider_configured(config: &AppConfig) -> bool {
    match config.provider.as_str() {
        // Vertex AI authenticates via Application Default Credentials rather
        // than a stored key, so presence in `provider_configs` isn't required.
        "vertex_ai" => crate::transcription::vertex::check_adc_available(),
        _ => config
            .provider_configs
            .get(&config.provider)
            .map(|v| match v {
                serde_json::Value::Object(m) => m
                    .values()
                    .any(|val| val.as_str().is_some_and(|s| !s.trim().is_empty())),
                _ => false,
            })
            .unwrap_or(false),
    }
}

/// Computes the current health snapshot from live system/config state.
pub fn compute<R: Runtime>(app: &AppHandle<R>) -> HealthStatus {
    let config = {
        let state = app.state::<Arc<Mutex<AppConfig>>>();
        let guard = state.lock().unwrap();
        guard.clone()
    };
    let mic_ok = mic_permission_ok();
    HealthStatus {
        mic_ok,
        mic_found: mic_device_found(mic_ok),
        ax_ok: crate::input::injector::check_accessibility_permission(),
        api_ok: provider_configured(&config),
    }
}
