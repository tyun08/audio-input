use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use reqwest::Client;
use std::time::Duration;
use tracing::{info, warn};

use super::polish::{SYSTEM_PROMPT_TEXT, SYSTEM_PROMPT_VISION};

pub const DEFAULT_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

pub struct GeminiClient {
    api_key: String,
    model: String,
    client: Client,
}

impl GeminiClient {
    pub fn new(api_key: String, model: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(GeminiClient {
            api_key,
            model,
            client,
        })
    }

    fn generate_content_url(&self) -> String {
        generate_content_url(&self.model)
    }

    pub async fn transcribe(&self, wav_bytes: Vec<u8>) -> Result<String> {
        if wav_bytes.len() < 1600 {
            warn!("Recording too short, skipping transcription");
            return Ok(String::new());
        }

        info!(
            "Sending {} bytes to Gemini ({}) for transcription",
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

        let text = send_gemini_request(
            &self.client,
            &self.generate_content_url(),
            &self.api_key,
            &body,
        )
        .await?;

        info!("Gemini transcription result: {:?}", text);
        Ok(text)
    }
}

pub async fn polish_text_gemini(
    text: &str,
    api_key: &str,
    model: &str,
    screenshot: Option<&str>,
) -> (String, bool) {
    let original_len = text.chars().count();
    let threshold = (original_len as f64 * 0.8) as usize;
    let max_tokens = ((original_len as u32 * 3 / 2) + 256).clamp(512, 65_536);
    let url = generate_content_url(model);

    let client = match Client::builder().timeout(Duration::from_secs(15)).build() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to create HTTP client for Gemini polish: {}", e);
            return (text.to_string(), true);
        }
    };

    if let Some(img_data) = screenshot {
        info!("Using Gemini vision model for polish (screenshot attached)");
        match try_vision(
            &client, &url, api_key, text, img_data, 0.1, false, max_tokens,
        )
        .await
        {
            Ok(p) if p.chars().count() >= threshold => {
                info!("Gemini vision polish complete");
                return (p, false);
            }
            Ok(short) => {
                warn!(
                    "Gemini vision polish result too short ({}/{}), retrying",
                    short.chars().count(),
                    threshold
                );
                if let Ok(p) = try_vision(
                    &client, &url, api_key, text, img_data, 0.3, true, max_tokens,
                )
                .await
                {
                    if p.chars().count() >= threshold {
                        return (p, false);
                    }
                }
                warn!("Gemini vision polish retry failed, falling back to text-only");
            }
            Err(e) => {
                warn!(
                    "Gemini vision polish failed: {}, falling back to text-only",
                    e
                );
            }
        }
    }

    match try_text(&client, &url, api_key, text, 0.1, false, max_tokens).await {
        Ok(p) if p.chars().count() >= threshold => {
            info!("Gemini polish complete");
            (p, false)
        }
        Ok(_) => match try_text(&client, &url, api_key, text, 0.3, true, max_tokens).await {
            Ok(p) if p.chars().count() >= threshold => (p, false),
            _ => {
                warn!("Gemini polish retry failed, returning original text");
                (text.to_string(), true)
            }
        },
        Err(e) => {
            warn!("Gemini polish failed: {}", e);
            (text.to_string(), true)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn try_vision(
    client: &Client,
    url: &str,
    api_key: &str,
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

    send_gemini_request(client, url, api_key, &body).await
}

async fn try_text(
    client: &Client,
    url: &str,
    api_key: &str,
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

    send_gemini_request(client, url, api_key, &body).await
}

fn generate_content_url(model: &str) -> String {
    format!("{}/models/{}:generateContent", DEFAULT_API_BASE, model)
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
    api_key: &str,
    body: &serde_json::Value,
) -> Result<String> {
    let resp = client
        .post(url)
        .header("x-goog-api-key", api_key)
        .json(body)
        .send()
        .await?;

    let status = resp.status();
    let resp_body = resp.text().await?;

    if !status.is_success() {
        bail!("Gemini API error: HTTP {} - {}", status, resp_body);
    }

    let response: serde_json::Value =
        serde_json::from_str(&resp_body).context("Failed to parse Gemini response")?;
    let text = response["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    if text.is_empty() {
        bail!("Gemini returned empty content");
    }

    Ok(text)
}
