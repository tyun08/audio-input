use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

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
    content: String,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize, Debug)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Deserialize, Debug)]
struct ChatMessageResponse {
    content: String,
}

pub async fn polish_text(text: &str, api_key: &str) -> String {
    match try_polish(text, api_key).await {
        Ok(polished) => {
            info!("润色完成: {:?}", polished);
            polished
        }
        Err(e) => {
            warn!("润色失败，使用原始文本: {}", e);
            text.to_string()
        }
    }
}

async fn try_polish(text: &str, api_key: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;

    let system_prompt = "你是文字整理助手。对语音转录文字：1)添加标点断句 2)修正明显同音错字 3)保持原意不改写。只输出整理后的文字，不加任何解释。";

    let request = ChatRequest {
        model: "openai/gpt-oss-20b".to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: text.to_string(),
            },
        ],
        temperature: 0.1,
        max_tokens: 1024,
    };

    let resp = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&request)
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        anyhow::bail!("Groq 润色 API 错误: HTTP {} {}", status, body);
    }

    let response: ChatResponse = serde_json::from_str(&body)?;
    let polished = response
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content.trim().to_string())
        .unwrap_or_else(|| text.to_string());

    Ok(polished)
}
