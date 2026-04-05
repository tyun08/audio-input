use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

const VISION_MODEL: &str = "meta-llama/llama-4-scout-17b-16e-instruct";
const PRIMARY_MODEL: &str = "openai/gpt-oss-20b";
const FALLBACK_MODEL: &str = "mistral-saba-24b";

// --- Request structs ---

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

/// Either a plain string (text-only) or an array of parts (multimodal).
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

// --- Response structs ---

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

// --- Helpers ---

fn compute_max_tokens(char_count: usize) -> u32 {
    ((char_count as u32 * 3 / 2) + 256).max(512).min(65_536)
}

const SYSTEM_PROMPT_TEXT: &str = "You are a transcription cleanup assistant. \
    For speech-to-text output: \
    1) Add punctuation and sentence breaks \
    2) Fix obvious speech recognition errors (homophones, mishearing) \
    3) Preserve the original meaning without rewriting. \
    Output only the cleaned text, no explanations. Respond in the same language as the input.";

const SYSTEM_PROMPT_VISION: &str = "You are a transcription cleanup assistant with access to a \
    screenshot of the user's current screen for context. Use the screen content to better \
    understand domain-specific terms, proper nouns, code identifiers, or UI elements the user \
    is referring to. For speech-to-text output: \
    1) Add punctuation and sentence breaks \
    2) Fix obvious speech recognition errors using screen context (homophones, mishearing, \
       technical terms visible on screen) \
    3) Preserve the original meaning without rewriting. \
    Output only the cleaned text, no explanations. Respond in the same language as the input.";

// --- Public API ---

/// Returns `(text, polish_failed)`.
/// If `screenshot` is Some, tries the vision model first for better context accuracy.
pub async fn polish_text(text: &str, api_key: &str, screenshot: Option<&str>) -> (String, bool) {
    let original_len = text.chars().count();
    let threshold = (original_len as f64 * 0.8) as usize;
    let max_tokens = compute_max_tokens(original_len);

    if let Some(img_data) = screenshot {
        info!("使用视觉模型润色 (截图上下文已附加)");
        match try_polish_vision(text, api_key, img_data, 0.1, false, max_tokens).await {
            Ok(polished) if polished.chars().count() >= threshold => {
                info!("视觉模型润色完成");
                return (polished, false);
            }
            Ok(short) => {
                warn!(
                    "视觉模型润色结果字数({})低于阈值({}), 重试",
                    short.chars().count(),
                    threshold
                );
                if let Ok(p) =
                    try_polish_vision(text, api_key, img_data, 0.3, true, max_tokens).await
                {
                    if p.chars().count() >= threshold {
                        info!("视觉模型重试润色成功");
                        return (p, false);
                    }
                }
                warn!("视觉模型重试仍失败，降级为纯文字润色");
            }
            Err(e) => {
                warn!("视觉模型失败: {}，降级为纯文字润色", e);
            }
        }
    }

    // Text-only path (either no screenshot or vision failed)
    match try_polish_text(text, api_key, PRIMARY_MODEL, 0.1, false, max_tokens).await {
        Ok(polished) if polished.chars().count() >= threshold => {
            info!("润色完成");
            (polished, false)
        }
        Ok(short) => {
            warn!(
                "润色结果字数({})低于阈值({}), 重试",
                short.chars().count(),
                threshold
            );
            match try_polish_text(text, api_key, PRIMARY_MODEL, 0.3, true, max_tokens).await {
                Ok(p) if p.chars().count() >= threshold => {
                    info!("重试润色成功");
                    (p, false)
                }
                Ok(_) | Err(_) => {
                    warn!("重试润色仍失败，返回原始文本");
                    (text.to_string(), true)
                }
            }
        }
        Err(e) => {
            warn!("主模型润色失败: {}，尝试备用模型 {}", e, FALLBACK_MODEL);
            match try_polish_text(text, api_key, FALLBACK_MODEL, 0.1, false, max_tokens).await {
                Ok(p) if p.chars().count() >= threshold => {
                    info!("备用模型润色成功");
                    (p, false)
                }
                Ok(_) | Err(_) => {
                    warn!("备用模型润色失败，返回原始文本");
                    (text.to_string(), true)
                }
            }
        }
    }
}

// --- Internal helpers ---

async fn try_polish_vision(
    text: &str,
    api_key: &str,
    screenshot_data_url: &str,
    temperature: f32,
    with_completeness_hint: bool,
    max_tokens: u32,
) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()?;

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
        model: VISION_MODEL.to_string(),
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

    send_request(&client, api_key, request).await
}

async fn try_polish_text(
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

    send_request(&client, api_key, request).await
}

async fn send_request(
    client: &reqwest::Client,
    api_key: &str,
    request: ChatRequest,
) -> anyhow::Result<String> {
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
