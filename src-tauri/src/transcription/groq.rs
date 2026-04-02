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
    client: reqwest::Client,
}

impl GroqClient {
    pub fn new(api_key: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("无法创建 HTTP 客户端");

        GroqClient { api_key, client }
    }

    pub async fn transcribe(&self, wav_bytes: Vec<u8>) -> Result<String> {
        // 不发送空/过短录音（< 0.1秒 = < 1600 bytes for 16kHz 16-bit mono）
        if wav_bytes.len() < 1600 {
            warn!("录音太短，跳过转录");
            return Ok(String::new());
        }

        info!("发送 {} bytes 到 Groq Whisper API", wav_bytes.len());

        let file_part = multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .context("设置 MIME 类型失败")?;

        let form = multipart::Form::new()
            .part("file", file_part)
            .text("model", "whisper-large-v3-turbo")
            .text("temperature", "0")
            .text("response_format", "verbose_json");

        let resp = self
            .client
            .post("https://api.groq.com/openai/v1/audio/transcriptions")
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .context("请求 Groq API 失败")?;

        let status = resp.status();
        let body = resp.text().await.context("读取响应失败")?;

        if !status.is_success() {
            let err_msg = serde_json::from_str::<GroqError>(&body)
                .map(|e| e.error.message)
                .unwrap_or_else(|_| format!("HTTP {}: {}", status, body));
            bail!("Groq API 错误: {}", err_msg);
        }

        let result: GroqResponse =
            serde_json::from_str(&body).context("解析转录响应失败")?;

        let text = result.text.trim().to_string();
        info!("转录结果: {:?}", text);
        Ok(text)
    }
}
