use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Runtime};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageStats {
    /// Cumulative seconds the microphone was actively recording.
    pub total_recording_secs: u64,
    /// Number of completed recording sessions.
    pub recording_count: u64,
}

pub type SharedUsage = Arc<Mutex<UsageStats>>;

pub fn new_shared_usage() -> SharedUsage {
    Arc::new(Mutex::new(UsageStats::default()))
}

impl UsageStats {
    pub fn load<R: Runtime>(app: &AppHandle<R>) -> Self {
        let path = usage_path(app);
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(stats) = serde_json::from_str::<UsageStats>(&data) {
                info!("Usage stats loaded: {:?}", path);
                return stats;
            }
        }
        UsageStats::default()
    }

    pub fn save<R: Runtime>(&self, app: &AppHandle<R>) -> Result<()> {
        let path = usage_path(app);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, data)?;
        Ok(())
    }

    /// Record a completed session of `duration_secs` seconds.
    pub fn add_session(&mut self, duration_secs: u64) {
        self.total_recording_secs += duration_secs;
        self.recording_count += 1;
    }
}

fn usage_path<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("usage.json")
}
