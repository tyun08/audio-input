use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub api_key: String,
    #[serde(default = "default_polish_enabled")]
    pub polish_enabled: bool,
    #[serde(default)]
    pub preferred_device: Option<String>,
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    #[serde(default)]
    pub onboarding_completed: bool,
}

fn default_polish_enabled() -> bool {
    true
}

fn default_shortcut() -> String {
    "Meta+Shift+Space".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            api_key: String::new(),
            polish_enabled: true,
            preferred_device: None,
            shortcut: "Meta+Shift+Space".to_string(),
            onboarding_completed: false,
        }
    }
}

impl AppConfig {
    pub fn load<R: Runtime>(app: &AppHandle<R>) -> Self {
        let path = config_path(app);
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<AppConfig>(&data) {
                info!("加载配置: {:?}", path);
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
        info!("保存配置: {:?}", path);
        Ok(())
    }
}

fn config_path<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("config.json")
}
