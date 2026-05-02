use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};
use tracing::info;

fn default_provider() -> String {
    "groq".to_string()
}

fn default_polish_enabled() -> bool {
    true
}

fn default_shortcut() -> String {
    #[cfg(target_os = "windows")]
    return "Ctrl+Shift+Space".to_string();
    #[cfg(not(target_os = "windows"))]
    return "Meta+Shift+Space".to_string();
}

fn default_max_history() -> usize {
    crate::history::DEFAULT_MAX_HISTORY
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub provider_configs: HashMap<String, serde_json::Value>,

    #[serde(default = "default_polish_enabled")]
    pub polish_enabled: bool,
    #[serde(default)]
    pub preferred_device: Option<String>,
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    #[serde(default)]
    pub onboarding_completed: bool,
    #[serde(default)]
    pub screenshot_context_enabled: bool,
    #[serde(default)]
    pub show_idle_hud: bool,
    #[serde(default = "default_max_history")]
    pub max_history: usize,

    // Legacy fields — read for migration, never written back.
    #[serde(default, skip_serializing)]
    api_key: String,
    #[serde(default, skip_serializing)]
    gcp_project_id: String,
    #[serde(default, skip_serializing)]
    gcp_location: String,
    #[serde(default, skip_serializing)]
    vertex_model: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            provider: default_provider(),
            provider_configs: HashMap::new(),
            polish_enabled: true,
            preferred_device: None,
            shortcut: default_shortcut(),
            onboarding_completed: false,
            screenshot_context_enabled: false,
            show_idle_hud: false,
            max_history: default_max_history(),
            api_key: String::new(),
            gcp_project_id: String::new(),
            gcp_location: String::new(),
            vertex_model: String::new(),
        }
    }
}

impl AppConfig {
    pub fn load<R: Runtime>(app: &AppHandle<R>) -> Self {
        let path = config_path(app);
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(mut cfg) = serde_json::from_str::<AppConfig>(&data) {
                cfg.migrate_legacy();
                info!("Config loaded: {:?}", path);
                return cfg;
            }
        }
        AppConfig::default()
    }

    pub fn save<R: Runtime>(app: &AppHandle<R>, config: &AppConfig) -> Result<()> {
        let path = config_path(app);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(config)?;
        std::fs::write(&path, data)?;
        info!("Config saved: {:?}", path);
        Ok(())
    }

    /// Migrate v0.2–v0.3.5-alpha legacy top-level fields into provider_configs.
    fn migrate_legacy(&mut self) {
        if !self.api_key.is_empty() && !self.provider_configs.contains_key("groq") {
            self.provider_configs.insert(
                "groq".into(),
                serde_json::json!({ "api_key": self.api_key }),
            );
        }
        self.api_key.clear();

        if !self.gcp_project_id.is_empty() && !self.provider_configs.contains_key("vertex_ai") {
            let loc = if self.gcp_location.is_empty() {
                "us-central1"
            } else {
                &self.gcp_location
            };
            let model = if self.vertex_model.is_empty() {
                "gemini-2.5-flash"
            } else {
                &self.vertex_model
            };
            self.provider_configs.insert(
                "vertex_ai".into(),
                serde_json::json!({
                    "project_id": self.gcp_project_id,
                    "location": loc,
                    "model": model,
                }),
            );
        }
        self.gcp_project_id.clear();
        self.gcp_location.clear();
        self.vertex_model.clear();

        // Migrate enum-serialised provider value (e.g. `"groq"` / `"vertex_ai"`)
        // Already strings, so nothing to do — serde handles it.
    }

    /// Helper: get a provider's config, returning an empty object if absent.
    pub fn get_pcfg(&self, provider: &str) -> serde_json::Value {
        self.provider_configs
            .get(provider)
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}))
    }
}

fn config_path<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("config.json")
}
