use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{info, warn};

use super::polish::{SYSTEM_PROMPT_TEXT, SYSTEM_PROMPT_VISION};

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

pub struct VertexClient {
    project_id: String,
    location: String,
    model: String,
    access_token: String,
    client: Client,
}

impl VertexClient {
    pub async fn new(project_id: String, location: String, model: String) -> Result<Self> {
        let access_token = get_access_token().await?;
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(VertexClient {
            project_id,
            location,
            model,
            access_token,
            client,
        })
    }

    fn gemini_url(&self, model: &str) -> String {
        format!(
            "https://{loc}-aiplatform.googleapis.com/v1/projects/{proj}/locations/{loc}/publishers/google/models/{model}:generateContent",
            loc = self.location,
            proj = self.project_id,
        )
    }

    pub async fn transcribe(&self, wav_bytes: Vec<u8>) -> Result<String> {
        if wav_bytes.len() < 1600 {
            warn!("Recording too short, skipping transcription");
            return Ok(String::new());
        }

        info!(
            "Sending {} bytes to Vertex AI Gemini ({}) for transcription",
            wav_bytes.len(),
            self.model
        );

        let audio_base64 = BASE64.encode(&wav_bytes);

        let body = serde_json::json!({
            "contents": [{
                "role": "user",
                "parts": [
                    {
                        "inlineData": {
                            "mimeType": "audio/wav",
                            "data": audio_base64
                        }
                    },
                    {
                        "text": "Transcribe this audio exactly as spoken. Output only the transcribed text, nothing else. Respond in the same language as the audio."
                    }
                ]
            }],
            "generationConfig": {
                "temperature": 0.0,
                "maxOutputTokens": 8192
            }
        });

        let text =
            send_gemini_request(&self.client, &self.gemini_url(&self.model), &self.access_token, &body).await?;

        info!("Vertex AI transcription result: {:?}", text);
        Ok(text)
    }
}

// ---------------------------------------------------------------------------
// Polish via Vertex AI Gemini
// ---------------------------------------------------------------------------

pub async fn polish_text_vertex(
    text: &str,
    project_id: &str,
    location: &str,
    model: &str,
    screenshot: Option<&str>,
) -> (String, bool) {
    let access_token = match get_access_token().await {
        Ok(t) => t,
        Err(e) => {
            warn!("Failed to get Vertex AI access token: {}", e);
            return (text.to_string(), true);
        }
    };

    let original_len = text.chars().count();
    let threshold = (original_len as f64 * 0.8) as usize;
    let max_tokens = ((original_len as u32 * 3 / 2) + 256).max(512).min(65_536);

    let url = format!(
        "https://{loc}-aiplatform.googleapis.com/v1/projects/{proj}/locations/{loc}/publishers/google/models/{model}:generateContent",
        loc = location, proj = project_id,
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap();

    if let Some(img_data) = screenshot {
        info!("Using Vertex AI vision model for polish (screenshot attached)");
        match try_vision(&client, &url, &access_token, text, img_data, 0.1, false, max_tokens)
            .await
        {
            Ok(p) if p.chars().count() >= threshold => {
                info!("Vertex AI vision polish complete");
                return (p, false);
            }
            Ok(short) => {
                warn!(
                    "Vision polish result too short ({}/{}), retrying",
                    short.chars().count(),
                    threshold
                );
                if let Ok(p) =
                    try_vision(&client, &url, &access_token, text, img_data, 0.3, true, max_tokens)
                        .await
                {
                    if p.chars().count() >= threshold {
                        return (p, false);
                    }
                }
                warn!("Vision polish retry failed, falling back to text-only");
            }
            Err(e) => {
                warn!("Vertex AI vision model failed: {}, falling back to text-only", e);
            }
        }
    }

    match try_text(&client, &url, &access_token, text, 0.1, false, max_tokens).await {
        Ok(p) if p.chars().count() >= threshold => {
            info!("Vertex AI polish complete");
            (p, false)
        }
        Ok(_) => {
            match try_text(&client, &url, &access_token, text, 0.3, true, max_tokens).await {
                Ok(p) if p.chars().count() >= threshold => (p, false),
                _ => {
                    warn!("Vertex AI polish retry failed, returning original text");
                    (text.to_string(), true)
                }
            }
        }
        Err(e) => {
            warn!("Vertex AI polish failed: {}", e);
            (text.to_string(), true)
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async fn try_vision(
    client: &Client,
    url: &str,
    token: &str,
    text: &str,
    screenshot_data_url: &str,
    temperature: f32,
    with_hint: bool,
    max_tokens: u32,
) -> Result<String> {
    let user_text = if with_hint {
        format!(
            "The above screenshot shows the user's current screen (context only).\n\nTranscription to clean up:\n{}\n\n(Please ensure the output is complete and covers all content without truncation.)",
            text
        )
    } else {
        format!(
            "The above screenshot shows the user's current screen (context only).\n\nTranscription to clean up:\n{}",
            text
        )
    };

    let mut parts = Vec::new();

    if let Some((mime, b64)) = parse_data_url(screenshot_data_url) {
        parts.push(serde_json::json!({
            "inlineData": { "mimeType": mime, "data": b64 }
        }));
    }
    parts.push(serde_json::json!({ "text": user_text }));

    let body = serde_json::json!({
        "contents": [{ "role": "user", "parts": parts }],
        "systemInstruction": { "parts": [{ "text": SYSTEM_PROMPT_VISION }] },
        "generationConfig": { "temperature": temperature, "maxOutputTokens": max_tokens }
    });

    send_gemini_request(client, url, token, &body).await
}

async fn try_text(
    client: &Client,
    url: &str,
    token: &str,
    text: &str,
    temperature: f32,
    with_hint: bool,
    max_tokens: u32,
) -> Result<String> {
    let user_text = if with_hint {
        format!(
            "{}\n\n(Please ensure the output is complete and covers all content without truncation.)",
            text
        )
    } else {
        text.to_string()
    };

    let body = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": user_text }] }],
        "systemInstruction": { "parts": [{ "text": SYSTEM_PROMPT_TEXT }] },
        "generationConfig": { "temperature": temperature, "maxOutputTokens": max_tokens }
    });

    send_gemini_request(client, url, token, &body).await
}

fn parse_data_url(data_url: &str) -> Option<(&str, &str)> {
    let stripped = data_url.strip_prefix("data:")?;
    let semicolon = stripped.find(';')?;
    let mime = &stripped[..semicolon];
    let base64_data = stripped[semicolon + 1..].strip_prefix("base64,")?;
    Some((mime, base64_data))
}

async fn send_gemini_request(
    client: &Client,
    url: &str,
    access_token: &str,
    body: &serde_json::Value,
) -> Result<String> {
    let resp = client
        .post(url)
        .bearer_auth(access_token)
        .json(body)
        .send()
        .await?;

    let status = resp.status();
    let resp_body = resp.text().await?;

    if !status.is_success() {
        bail!("Vertex AI API error: HTTP {} — {}", status, resp_body);
    }

    let response: serde_json::Value =
        serde_json::from_str(&resp_body).context("Failed to parse Vertex AI response")?;
    let text = response["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    if text.is_empty() {
        bail!("Vertex AI returned empty content");
    }

    Ok(text)
}

// ---------------------------------------------------------------------------
// ADC (Application Default Credentials) token management
// ---------------------------------------------------------------------------

pub fn check_adc_available() -> bool {
    get_adc_path()
        .ok()
        .and_then(|p| std::fs::metadata(&p).ok())
        .map(|m| m.is_file())
        .unwrap_or(false)
}

async fn get_access_token() -> Result<String> {
    let adc_path = get_adc_path()?;
    let data = std::fs::read_to_string(&adc_path).with_context(|| {
        format!(
            "Cannot read ADC credentials: {:?}\nRun: gcloud auth application-default login",
            adc_path
        )
    })?;

    let creds: serde_json::Value =
        serde_json::from_str(&data).context("Failed to parse ADC credentials")?;

    let cred_type = creds["type"].as_str().unwrap_or("");
    if cred_type != "authorized_user" {
        bail!(
            "Only authorized_user credential type is supported (current: {})\nRun: gcloud auth application-default login",
            cred_type
        );
    }

    let client_id = creds["client_id"].as_str().context("ADC missing client_id")?;
    let client_secret = creds["client_secret"]
        .as_str()
        .context("ADC missing client_secret")?;
    let refresh_token = creds["refresh_token"]
        .as_str()
        .context("ADC missing refresh_token")?;

    let client = Client::new();
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .context("Failed to request Google OAuth token")?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        bail!("Failed to get access token: {}", body);
    }

    let token: TokenResponse = resp.json().await.context("Failed to parse token response")?;
    Ok(token.access_token)
}

fn get_adc_path() -> Result<std::path::PathBuf> {
    if let Ok(path) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
        return Ok(std::path::PathBuf::from(path));
    }

    let home = dirs::home_dir().context("Cannot determine home directory")?;

    #[cfg(target_os = "windows")]
    {
        Ok(home
            .join("AppData")
            .join("Roaming")
            .join("gcloud")
            .join("application_default_credentials.json"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(home
            .join(".config")
            .join("gcloud")
            .join("application_default_credentials.json"))
    }
}
