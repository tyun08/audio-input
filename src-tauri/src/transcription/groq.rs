use anyhow::{bail, Context, Result};
use reqwest::multipart;
use serde::Deserialize;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Deserialize, Debug)]
struct GroqResponse {
    text: String,
}

#[derive(Deserialize, Debug)]
struct GroqError {
    error: GroqErrorDetail,
}

#[derive(Deserialize, Debug)]
struct GroqErrorDetail {
    message: String,
}

pub struct GroqClient {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl GroqClient {
    pub fn new(api_key: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        GroqClient {
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

        info!("Sending {} bytes to Groq Whisper API", wav_bytes.len());

        let file_part = multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .context("Failed to set MIME type")?;

        let form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("temperature", "0")
            .text("response_format", "verbose_json");

        let resp = self
            .client
            .post("https://api.groq.com/openai/v1/audio/transcriptions")
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .context("Groq API request failed")?;

        let status = resp.status();
        let body = resp.text().await.context("Failed to read response")?;

        if !status.is_success() {
            let err_msg = serde_json::from_str::<GroqError>(&body)
                .map(|e| e.error.message)
                .unwrap_or_else(|_| format!("HTTP {}: {}", status, body));
            bail!("Groq API error: {}", err_msg);
        }

        let result: GroqResponse =
            serde_json::from_str(&body).context("Failed to parse transcription response")?;

        let text = result.text.trim().to_string();
        info!("Transcription result: {:?}", text);
        Ok(text)
    }
}
