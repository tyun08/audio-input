//! Aggregated service-health check surfaced on the tray icon and the health
//! popover: microphone permission, microphone device presence, Accessibility
//! permission, and transcription API readiness.
//!
//! Permission/config checks are cheap, but input-device enumeration can take
//! tens of milliseconds on macOS. Full probes therefore run on the idle tray
//! timer; shortcut handlers consume the cached snapshot.

use crate::config::AppConfig;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Runtime};

/// Last health snapshot, refreshed while the app is idle. Hotkey handling reads
/// this in-memory value instead of enumerating audio devices synchronously.
pub type SharedHealth = Arc<Mutex<HealthStatus>>;

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

pub fn new_shared_health(initial: HealthStatus) -> SharedHealth {
    Arc::new(Mutex::new(initial))
}

pub fn cached<R: Runtime>(app: &AppHandle<R>) -> HealthStatus {
    app.state::<SharedHealth>().lock().unwrap().to_owned()
}

pub fn compute_and_cache<R: Runtime>(app: &AppHandle<R>) -> HealthStatus {
    let health = compute(app);
    *app.state::<SharedHealth>().lock().unwrap() = health;
    health
}

/// AVMediaTypeAudio ("soun") — the four-char code AVFoundation uses to
/// identify the microphone media type in authorization checks, expressed as
/// a NUL-terminated C string for the Objective-C `stringWithUTF8String:` call.
#[cfg(target_os = "macos")]
const AV_MEDIA_TYPE_AUDIO: &std::ffi::CStr = c"soun";

/// AVAuthorizationStatusAuthorized — the user has explicitly granted access.
#[cfg(target_os = "macos")]
const AV_AUTHORIZATION_STATUS_AUTHORIZED: i64 = 3;

/// Microphone TCC authorization only (does not enumerate devices).
#[cfg(target_os = "macos")]
pub fn mic_permission_ok() -> bool {
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let media_type: *mut objc::runtime::Object =
            msg_send![class!(NSString), stringWithUTF8String: AV_MEDIA_TYPE_AUDIO.as_ptr()];
        let status: i64 =
            msg_send![class!(AVCaptureDevice), authorizationStatusForMediaType: media_type];
        status == AV_AUTHORIZATION_STATUS_AUTHORIZED
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- HealthStatus::is_healthy ---

    #[test]
    fn is_healthy_when_all_checks_pass() {
        let status = HealthStatus {
            mic_ok: true,
            mic_found: true,
            ax_ok: true,
            api_ok: true,
        };
        assert!(status.is_healthy());
    }

    #[test]
    fn is_unhealthy_when_mic_permission_missing() {
        let status = HealthStatus {
            mic_ok: false,
            mic_found: true,
            ax_ok: true,
            api_ok: true,
        };
        assert!(!status.is_healthy());
    }

    #[test]
    fn is_unhealthy_when_no_mic_device_found() {
        let status = HealthStatus {
            mic_ok: true,
            mic_found: false,
            ax_ok: true,
            api_ok: true,
        };
        assert!(!status.is_healthy());
    }

    #[test]
    fn is_unhealthy_when_accessibility_permission_missing() {
        let status = HealthStatus {
            mic_ok: true,
            mic_found: true,
            ax_ok: false,
            api_ok: true,
        };
        assert!(!status.is_healthy());
    }

    #[test]
    fn is_unhealthy_when_api_not_configured() {
        let status = HealthStatus {
            mic_ok: true,
            mic_found: true,
            ax_ok: true,
            api_ok: false,
        };
        assert!(!status.is_healthy());
    }

    #[test]
    fn shared_health_updates_without_reprobing_devices() {
        let initial = HealthStatus {
            mic_ok: true,
            mic_found: true,
            ax_ok: true,
            api_ok: true,
        };
        let shared = new_shared_health(initial);
        let updated = HealthStatus {
            mic_found: false,
            ..initial
        };

        *shared.lock().unwrap() = updated;

        assert_eq!(*shared.lock().unwrap(), updated);
    }

    // --- mic_device_found ---

    #[test]
    fn mic_device_found_short_circuits_without_permission() {
        // With mic_ok=false this must return false without probing devices
        // (probing before permission is granted triggers the macOS TCC
        // prompt, which would defeat passive health polling).
        assert!(!mic_device_found(false));
    }

    // --- provider_configured ---

    fn config_with(provider: &str, provider_configs: serde_json::Value) -> AppConfig {
        let mut config = AppConfig::default();
        config.provider = provider.to_string();
        config.provider_configs = serde_json::from_value(provider_configs).unwrap();
        config
    }

    #[test]
    fn provider_not_configured_when_no_entry_present() {
        let config = config_with("groq", json!({}));
        assert!(!provider_configured(&config));
    }

    #[test]
    fn provider_not_configured_when_entry_has_only_blank_values() {
        let config = config_with("groq", json!({ "groq": { "apiKey": "   " } }));
        assert!(!provider_configured(&config));
    }

    #[test]
    fn provider_not_configured_when_entry_is_not_an_object() {
        let config = config_with("groq", json!({ "groq": "not-an-object" }));
        assert!(!provider_configured(&config));
    }

    #[test]
    fn provider_configured_when_entry_has_a_non_blank_value() {
        let config = config_with("groq", json!({ "groq": { "apiKey": "sk-live-123" } }));
        assert!(provider_configured(&config));
    }

    #[test]
    fn vertex_ai_provider_ignores_provider_configs() {
        // Vertex AI authenticates via ADC, so an empty provider_configs entry
        // must not affect the result — it should match check_adc_available()
        // regardless of what (if anything) is stored for it.
        let config = config_with("vertex_ai", json!({}));
        assert_eq!(
            provider_configured(&config),
            crate::transcription::vertex::check_adc_available()
        );
    }
}
