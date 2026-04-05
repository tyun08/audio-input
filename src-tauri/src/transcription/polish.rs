use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

const PRIMARY_MODEL: &str = "openai/gpt-oss-20b";
const FALLBACK_MODEL: &str = "mistral-saba-24b";

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

/// Dynamic max_tokens scaled to input length (proxy for recording duration).
/// Clamps to [512, 4096].
fn compute_max_tokens(char_count: usize) -> u32 {
    ((char_count as u32 * 3 / 2) + 256).max(512).min(65_536)
}

/// Returns `(text, polish_failed)`.
/// Retry uses higher temperature + completeness hint.
/// On primary model error, falls back to `mistral-saba-24b`.
pub async fn polish_text(text: &str, api_key: &str) -> (String, bool) {
    let original_len = text.chars().count();
    let threshold = (original_len as f64 * 0.8) as usize;
    let max_tokens = compute_max_tokens(original_len);

    match try_polish(text, api_key, PRIMARY_MODEL, 0.1, false, max_tokens).await {
        Ok(polished) if polished.chars().count() >= threshold => {
            info!("润色完成");
            (polished, false)
        }
        Ok(short) => {
            warn!(
                "润色结果字数({})低于阈值({}), 重试（temperature=0.3 + 完整性提示）",
                short.chars().count(),
                threshold
            );
            match try_polish(text, api_key, PRIMARY_MODEL, 0.3, true, max_tokens).await {
                Ok(polished2) if polished2.chars().count() >= threshold => {
                    info!("重试润色成功");
                    (polished2, false)
                }
                Ok(_) | Err(_) => {
                    warn!("重试润色仍失败，返回原始文本");
                    (text.to_string(), true)
                }
            }
        }
        Err(e) => {
            warn!("主模型润色失败: {}，尝试备用模型 {}", e, FALLBACK_MODEL);
            match try_polish(text, api_key, FALLBACK_MODEL, 0.1, false, max_tokens).await {
                Ok(polished) if polished.chars().count() >= threshold => {
                    info!("备用模型润色成功");
                    (polished, false)
                }
                Ok(_) | Err(_) => {
                    warn!("备用模型润色失败，返回原始文本");
                    (text.to_string(), true)
                }
            }
        }
    }
}

async fn try_polish(
    text: &str,
    api_key: &str,
    model: &str,
    temperature: f32,
    with_completeness_hint: bool,
    max_tokens: u32,
) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;

    let system_prompt = "You are a transcription cleanup assistant. For speech-to-text output: \
        1) Add punctuation and sentence breaks \
        2) Fix obvious speech recognition errors (homophones, mishearing) \
        3) Preserve the original meaning without rewriting. \
        Output only the cleaned text, no explanations. Respond in the same language as the input.";

    let user_content = if with_completeness_hint {
        format!(
            "{}\n\n(Please ensure the output is complete and covers all content without truncation.)",
            text
        )
    } else {
        text.to_string()
    };

    let request = ChatRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_content,
            },
        ],
        temperature,
        max_tokens,
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
        .unwrap_or_default();

    if polished.is_empty() {
        anyhow::bail!("润色返回空内容");
    }

    Ok(polished)
}
