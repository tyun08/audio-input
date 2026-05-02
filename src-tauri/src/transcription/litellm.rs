use anyhow::{bail, Context, Result};
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

use super::polish::{parse_polish_response, SYSTEM_PROMPT_TEXT, SYSTEM_PROMPT_VISION};

pub const DEFAULT_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_POLISH_MODEL: &str = "gpt-4o-mini";

// --- Transcription response structs ---

#[derive(Deserialize, Debug)]
struct TranscriptionResponse {
    text: String,
}

#[derive(Deserialize, Debug)]
struct ApiError {
    error: ApiErrorDetail,
}

#[derive(Deserialize, Debug)]
struct ApiErrorDetail {
    message: String,
}

// --- Chat completions request structs (for polish) ---

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: MessageContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrlContent },
}

#[derive(Serialize)]
struct ImageUrlContent {
    url: String,
}

// --- Client ---

pub struct LiteLLMClient {
    api_base: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl LiteLLMClient {
    pub fn new(api_base: String, api_key: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        LiteLLMClient {
            api_base,
            api_key,
            model,
            client,
        }
    }

    pub async fn transcribe(&self, wav_bytes: Vec<u8>) -> Result<String> {
        // Skip empty/too-short recordings (< 0.1s = < 1600 bytes for 16kHz 16-bit mono)
        if wav_bytes.len() < 1600 {
            warn!("Recording too short, skipping transcription");
            return Ok(String::new());
        }

        info!(
            "Sending {} bytes to LiteLLM ({}) for transcription",
            wav_bytes.len(),
            self.model
        );

        let file_part = multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .context("Failed to set MIME type")?;

        let form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("temperature", "0")
            .text("response_format", "json");

        let url = format!(
            "{}/audio/transcriptions",
            self.api_base.trim_end_matches('/')
        );

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .context("LiteLLM API request failed")?;

        let status = resp.status();
        let body = resp.text().await.context("Failed to read response")?;

        if !status.is_success() {
            let err_msg = serde_json::from_str::<ApiError>(&body)
                .map(|e| e.error.message)
                .unwrap_or_else(|_| format!("HTTP {}: {}", status, body));
            bail!("LiteLLM API error: {}", err_msg);
        }

        let result: TranscriptionResponse =
            serde_json::from_str(&body).context("Failed to parse transcription response")?;

        let text = result.text.trim().to_string();
        info!("Transcription result: {:?}", text);
        Ok(text)
    }
}

// --- Polish via LiteLLM chat completions ---

/// Returns `(text, polish_failed)`.
/// Uses the OpenAI-compatible `/chat/completions` endpoint at `api_base`.
/// If `screenshot` is Some, attempts vision-assisted polish first.
pub async fn polish_text_litellm(
    text: &str,
    api_base: &str,
    api_key: &str,
    screenshot: Option<&str>,
) -> (String, bool) {
    let base = api_base.trim_end_matches('/');
    let url = format!("{}/chat/completions", base);

    let original_len = text.chars().count();
    let threshold = (original_len as f64 * 0.8) as usize;
    let max_tokens = ((original_len as u32 * 3 / 2) + 256).clamp(512, 65_536);

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to create HTTP client for LiteLLM polish: {}", e);
            return (text.to_string(), true);
        }
    };

    // Try vision-assisted polish if a screenshot is available
    if let Some(img_data) = screenshot {
        info!("Using vision model for LiteLLM polish (screenshot context attached)");
        match try_polish_vision(
            &client, &url, api_key, text, img_data, 0.1, false, max_tokens,
        )
        .await
        {
            Ok(polished) if polished.chars().count() >= threshold => {
                info!("LiteLLM vision polish complete");
                return (polished, false);
            }
            Ok(short) => {
                warn!(
                    "LiteLLM vision polish result too short ({}/{} chars), retrying",
                    short.chars().count(),
                    threshold
                );
                if let Ok(p) = try_polish_vision(
                    &client, &url, api_key, text, img_data, 0.3, true, max_tokens,
                )
                .await
                {
                    if p.chars().count() >= threshold {
                        return (p, false);
                    }
                }
                warn!("LiteLLM vision polish retry failed, falling back to text-only");
            }
            Err(e) => {
                warn!(
                    "LiteLLM vision polish failed: {}, falling back to text-only",
                    e
                );
            }
        }
    }

    // Text-only path
    match try_polish_text(&client, &url, api_key, text, 0.1, false, max_tokens).await {
        Ok(polished) if polished.chars().count() >= threshold => {
            info!("LiteLLM text polish complete");
            (polished, false)
        }
        Ok(short) => {
            warn!(
                "LiteLLM polish result too short ({}/{} chars), retrying",
                short.chars().count(),
                threshold
            );
            match try_polish_text(&client, &url, api_key, text, 0.3, true, max_tokens).await {
                Ok(p) if p.chars().count() >= threshold => (p, false),
                Ok(_) | Err(_) => {
                    warn!("LiteLLM polish retry still failed, returning original text");
                    (text.to_string(), true)
                }
            }
        }
        Err(e) => {
            warn!("LiteLLM polish failed: {}, retrying", e);
            match try_polish_text(&client, &url, api_key, text, 0.3, true, max_tokens).await {
                Ok(p) if p.chars().count() >= threshold => (p, false),
                Ok(_) | Err(_) => {
                    warn!("LiteLLM polish retry failed, returning original text");
                    (text.to_string(), true)
                }
            }
        }
    }
}

// --- Internal helpers ---

#[allow(clippy::too_many_arguments)]
async fn try_polish_vision(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    text: &str,
    screenshot_data_url: &str,
    temperature: f32,
    with_completeness_hint: bool,
    max_tokens: u32,
) -> anyhow::Result<String> {
    let transcription_text = if with_completeness_hint {
        format!(
            "{}\n\n(Please ensure the output is complete and covers all content without truncation.)",
            text
        )
    } else {
        text.to_string()
    };

    let user_content = MessageContent::Parts(vec![
        ContentPart::ImageUrl {
            image_url: ImageUrlContent {
                url: screenshot_data_url.to_string(),
            },
        },
        ContentPart::Text {
            text: format!(
                "The above screenshot shows the user's current screen (context only).\n\nTranscription to clean up:\n{}",
                transcription_text
            ),
        },
    ]);

    let request = ChatRequest {
        model: DEFAULT_POLISH_MODEL.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: MessageContent::Text(SYSTEM_PROMPT_VISION.to_string()),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_content,
            },
        ],
        temperature,
        max_tokens,
    };

    send_chat_request(client, url, api_key, &request).await
}

async fn try_polish_text(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    text: &str,
    temperature: f32,
    with_completeness_hint: bool,
    max_tokens: u32,
) -> anyhow::Result<String> {
    let user_content = if with_completeness_hint {
        format!(
            "{}\n\n(Please ensure the output is complete and covers all content without truncation.)",
            text
        )
    } else {
        text.to_string()
    };

    let request = ChatRequest {
        model: DEFAULT_POLISH_MODEL.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: MessageContent::Text(SYSTEM_PROMPT_TEXT.to_string()),
            },
            ChatMessage {
                role: "user".to_string(),
                content: MessageContent::Text(user_content),
            },
        ],
        temperature,
        max_tokens,
    };

    send_chat_request(client, url, api_key, &request).await
}

async fn send_chat_request(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    request: &ChatRequest,
) -> anyhow::Result<String> {
    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(request)
        .send()
        .await
        .context("LiteLLM chat request failed")?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .context("Failed to read LiteLLM response")?;

    if !status.is_success() {
        bail!("LiteLLM chat API error: HTTP {} {}", status, body);
    }

    parse_polish_response(&body)
}
