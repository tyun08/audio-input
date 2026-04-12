use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

const VISION_MODEL: &str = "meta-llama/llama-4-scout-17b-16e-instruct";
const PRIMARY_MODEL: &str = "openai/gpt-oss-20b";

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

pub(crate) const SYSTEM_PROMPT_TEXT: &str = "You are a transcription cleanup assistant. \
    For speech-to-text output: \
    1) Add punctuation and sentence breaks \
    2) Fix obvious speech recognition errors (homophones, mishearing) \
    3) Preserve the original meaning without rewriting. \
    Output only the cleaned text, no explanations. Respond in the same language as the input.";

pub(crate) const SYSTEM_PROMPT_VISION: &str = "You are a transcription cleanup assistant with access to a \
    screenshot of the user's current screen for context. \
    Use visible text — especially brand names, product names, and technical terms — as a \
    reference when the transcription contains a word that sounds similar but may be a \
    mishearing. For speech-to-text output: \
    1) Add punctuation and sentence breaks \
    2) Fix speech recognition errors (homophones, mishearing); prefer screen-visible spellings \
       for proper nouns and technical terms when there is a plausible phonetic match \
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
        info!("Using vision model for polish (screenshot context attached)");
        match try_polish_vision(text, api_key, img_data, 0.1, false, max_tokens).await {
            Ok(polished) if polished.chars().count() >= threshold => {
                info!("Vision model polish complete");
                return (polished, false);
            }
            Ok(short) => {
                warn!(
                    "Vision polish result too short ({}/{} chars), retrying",
                    short.chars().count(),
                    threshold
                );
                if let Ok(p) =
                    try_polish_vision(text, api_key, img_data, 0.3, true, max_tokens).await
                {
                    if p.chars().count() >= threshold {
                        info!("Vision model retry succeeded");
                        return (p, false);
                    }
                }
                warn!("Vision model retry failed, falling back to text-only polish");
            }
            Err(e) => {
                warn!("Vision model failed: {}, falling back to text-only polish", e);
            }
        }
    }

    // Text-only path (either no screenshot or vision failed)
    match try_polish_text(text, api_key, PRIMARY_MODEL, 0.1, false, max_tokens).await {
        Ok(polished) if polished.chars().count() >= threshold => {
            info!("Polish complete");
            (polished, false)
        }
        Ok(short) => {
            warn!(
                "Polish result too short ({}/{} chars), retrying",
                short.chars().count(),
                threshold
            );
            match try_polish_text(text, api_key, PRIMARY_MODEL, 0.3, true, max_tokens).await {
                Ok(p) if p.chars().count() >= threshold => {
                    info!("Polish retry succeeded");
                    (p, false)
                }
                Ok(_) | Err(_) => {
                    warn!("Polish retry still failed, returning original text");
                    (text.to_string(), true)
                }
            }
        }
        Err(e) => {
            warn!("Primary model polish failed: {}, retrying with higher temperature", e);
            match try_polish_text(text, api_key, PRIMARY_MODEL, 0.3, true, max_tokens).await {
                Ok(p) if p.chars().count() >= threshold => {
                    info!("Polish retry succeeded");
                    (p, false)
                }
                Ok(_) | Err(_) => {
                    warn!("Polish retry failed, returning original text");
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
        .timeout(Duration::from_secs(10))
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
        anyhow::bail!("Groq polish API error: HTTP {} {}", status, body);
    }

    parse_polish_response(&body)
}

/// Parses a Groq-compatible chat-completion JSON response and returns the trimmed content of the
/// first choice. Returns an error if the JSON is malformed, the choices array is empty, or the
/// extracted content is blank after trimming.
pub(crate) fn parse_polish_response(body: &str) -> anyhow::Result<String> {
    let response: ChatResponse = serde_json::from_str(body)?;
    let polished = response
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content.trim().to_string())
        .unwrap_or_default();

    if polished.is_empty() {
        anyhow::bail!("Polish returned empty content");
    }

    Ok(polished)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- compute_max_tokens ---

    #[test]
    fn test_compute_max_tokens_minimum() {
        // Very short input should clamp to minimum of 512
        assert_eq!(compute_max_tokens(0), 512);
        assert_eq!(compute_max_tokens(1), 512);
    }

    #[test]
    fn test_compute_max_tokens_scales_with_input() {
        // 1000-char input: (1000 * 3) / 2 + 256 = 1500 + 256 = 1756
        assert_eq!(compute_max_tokens(1000), 1756);
    }

    #[test]
    fn test_compute_max_tokens_maximum() {
        // Very large input should clamp to maximum of 65536
        assert_eq!(compute_max_tokens(100_000), 65_536);
    }

    // --- parse_polish_response ---

    #[test]
    fn test_parse_polish_response_valid() {
        let body = r#"{
            "choices": [{"message": {"content": "  polished text  "}}]
        }"#;
        let result = parse_polish_response(body).unwrap();
        assert_eq!(result, "polished text");
    }

    #[test]
    fn test_parse_polish_response_trims_whitespace() {
        let body = r#"{"choices": [{"message": {"content": "\n  hello world\n  "}}]}"#;
        let result = parse_polish_response(body).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_parse_polish_response_empty_content_returns_error() {
        let body = r#"{"choices": [{"message": {"content": ""}}]}"#;
        let err = parse_polish_response(body).unwrap_err();
        assert!(
            err.to_string().contains("empty content"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn test_parse_polish_response_whitespace_only_returns_error() {
        let body = r#"{"choices": [{"message": {"content": "   "}}]}"#;
        let err = parse_polish_response(body).unwrap_err();
        assert!(err.to_string().contains("empty content"));
    }

    #[test]
    fn test_parse_polish_response_empty_choices_returns_error() {
        let body = r#"{"choices": []}"#;
        let err = parse_polish_response(body).unwrap_err();
        assert!(err.to_string().contains("empty content"));
    }

    #[test]
    fn test_parse_polish_response_invalid_json_returns_error() {
        let err = parse_polish_response("not json").unwrap_err();
        // Should be a serde_json parse error
        assert!(!err.to_string().is_empty());
    }

    // --- system prompt constants ---

    #[test]
    fn test_system_prompt_text_not_empty() {
        assert!(!SYSTEM_PROMPT_TEXT.is_empty());
        assert!(SYSTEM_PROMPT_TEXT.contains("transcription"));
    }

    #[test]
    fn test_system_prompt_vision_not_empty() {
        assert!(!SYSTEM_PROMPT_VISION.is_empty());
        assert!(SYSTEM_PROMPT_VISION.contains("screenshot"));
    }

    #[test]
    fn test_system_prompt_text_instructs_same_language() {
        assert!(SYSTEM_PROMPT_TEXT.contains("same language"));
    }

    #[test]
    fn test_system_prompt_vision_instructs_same_language() {
        assert!(SYSTEM_PROMPT_VISION.contains("same language"));
    }
}
