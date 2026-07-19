use crate::config::{AppConfig, UpdateChannel};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

const STABLE_ENDPOINT: &str =
    "https://github.com/tyun08/audio-input/releases/latest/download/latest.json";
const BETA_ENDPOINT: &str =
    "https://github.com/tyun08/audio-input/releases/download/beta/latest-beta.json";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    version: String,
    body: Option<String>,
}

fn endpoint(channel: UpdateChannel) -> &'static str {
    match channel {
        UpdateChannel::Stable => STABLE_ENDPOINT,
        UpdateChannel::Beta => BETA_ENDPOINT,
    }
}

async fn find_update(app: &AppHandle, channel: UpdateChannel) -> Result<Option<Update>, String> {
    let url = Url::parse(endpoint(channel)).map_err(|e| e.to_string())?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    updater.check().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_update_channel(config: tauri::State<'_, Arc<Mutex<AppConfig>>>) -> UpdateChannel {
    config.lock().unwrap().update_channel
}

#[tauri::command]
pub async fn save_update_channel(
    channel: UpdateChannel,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    let updated = {
        let mut cfg = config.lock().unwrap();
        cfg.update_channel = channel;
        cfg.clone()
    };
    AppConfig::save(&app, &updated).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<Option<AvailableUpdate>, String> {
    let channel = config.lock().unwrap().update_channel;
    Ok(find_update(&app, channel)
        .await?
        .map(|update| AvailableUpdate {
            version: update.version,
            body: update.body,
        }))
}

#[tauri::command]
pub async fn install_update(
    version: String,
    app: AppHandle,
    config: tauri::State<'_, Arc<Mutex<AppConfig>>>,
) -> Result<(), String> {
    let channel = config.lock().unwrap().update_channel;
    let update = find_update(&app, channel)
        .await?
        .ok_or_else(|| "The selected update is no longer available.".to_string())?;
    if update.version != version {
        return Err(format!(
            "The available update changed from {version} to {}. Check again before installing.",
            update.version
        ));
    }

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_use_independent_manifests() {
        assert!(endpoint(UpdateChannel::Stable).ends_with("/latest/download/latest.json"));
        assert!(endpoint(UpdateChannel::Beta).ends_with("/beta/latest-beta.json"));
    }
}
