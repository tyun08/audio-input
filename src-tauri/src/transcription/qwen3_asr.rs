use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tauri::{AppHandle, Emitter as _, Runtime};
use tracing::{info, warn};

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Delta,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

pub struct Qwen3AsrClient {
    host: String,
    port: u16,
    model: String,
    language: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl Qwen3AsrClient {
    pub fn new(host: String, port: u16, model: String, language: String, api_key: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");
        Qwen3AsrClient { host, port, model, language, api_key, client }
    }

    pub async fn transcribe<R: Runtime>(
        &self,
        wav_bytes: Vec<u8>,
        app: &AppHandle<R>,
    ) -> Result<String> {
        if wav_bytes.len() < 1600 {
            warn!("Recording too short, skipping transcription");
            return Ok(String::new());
        }

        let audio_b64 = STANDARD.encode(&wav_bytes);
        let audio_url = format!("data:audio/wav;base64,{}", audio_b64);

        let mut messages = Vec::new();

        // System prompt for language hint
        if self.language != "auto" {
            messages.push(json!({
                "role": "system",
                "content": format!("Please transcribe audio in {}.", self.language)
            }));
        }

        messages.push(json!({
            "role": "user",
            "content": [
                {
                    "type": "audio_url",
                    "audio_url": { "url": audio_url }
                }
            ]
        }));

        let request_body = json!({
            "model": self.model,
            "stream": true,
            "messages": messages
        });

        let url = format!("http://{}:{}/v1/chat/completions", self.host, self.port);
        info!("Sending audio to Qwen3-ASR at {}", url);

        let mut req = self.client.post(&url).json(&request_body);
        if let Some(key) = &self.api_key {
            if !key.is_empty() {
                req = req.bearer_auth(key);
            }
        }

        let resp = req
            .send()
            .await
            .context("Qwen3-ASR server request failed — is the vLLM server running?")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("Qwen3-ASR server error {}: {}", status, body);
        }

        let mut stream = resp.bytes_stream();
        let mut full_text = String::new();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Stream read error")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Consume complete newline-terminated SSE lines from buffer
            loop {
                let Some(pos) = buffer.find('\n') else { break };
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(parsed) = serde_json::from_str::<StreamChunk>(data) {
                        if let Some(choice) = parsed.choices.first() {
                            if let Some(content) = &choice.delta.content {
                                if !content.is_empty() {
                                    full_text.push_str(content);
                                    let _ = app.emit("transcription-stream", content.as_str());
                                }
                            }
                        }
                    }
                }
            }
        }

        let text = full_text.trim().to_string();
        info!("Qwen3-ASR transcription result: {:?}", text);
        Ok(text)
    }
}
