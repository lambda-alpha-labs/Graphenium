/// LLM API client supporting multiple AI providers.
///
/// Two request formats are supported:
/// - **Anthropic Messages** (Anthropic): `POST /v1/messages`, `x-api-key` auth,
///   system prompt as top-level field, image blocks as `{type: image, source: ...}`.
/// - **OpenAI Chat Completions** (OpenAI, OpenRouter, DeepSeek, compatible):
///   `POST /v1/chat/completions`, `Authorization: Bearer` auth, system prompt
///   as a `role: system` message, image blocks as `{type: image_url, image_url: ...}`.
///
/// Retry logic (429 rate-limit, 5xx server errors, JSON parse failures) is
/// shared across all providers.
use serde::{Deserialize, Serialize};

use super::provider::{AiProvider, RequestFormat};

// ── Public types ──────────────────────────────────────────────────────────

/// A single block in the content array sent to the API.
///
/// Serializes differently depending on format.  For Anthropic, images use
/// `{type: "image", source: {type: "base64", ...}}`.  For OpenAI, images
/// use `{type: "image_url", image_url: {url: "data:...;base64,..."}}`.
#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text { text: String },
    Image { media_type: String, data: String },
}

impl ContentBlock {
    pub fn text(t: impl Into<String>) -> Self {
        Self::Text { text: t.into() }
    }

    pub fn image(media_type: impl Into<String>, base64_data: impl Into<String>) -> Self {
        Self::Image {
            media_type: media_type.into(),
            data: base64_data.into(),
        }
    }
}

/// Public image source struct (kept for backward compat).
#[derive(Debug, Clone, Serialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub kind: String,
    pub media_type: String,
    pub data: String,
}

// ── Anthropic API types ───────────────────────────────────────────────────

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<AnthropicMessage<'a>>,
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: Vec<AnthropicContentBlock<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentBlock<'a> {
    Text { text: &'a str },
    Image { source: AnthropicImageSource<'a> },
}

#[derive(Serialize)]
struct AnthropicImageSource<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    media_type: &'a str,
    data: &'a str,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicResponseBlock>,
    usage: AnthropicUsage,
}

#[derive(Deserialize)]
struct AnthropicResponseBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u64,
    output_tokens: u64,
}

// ── OpenAI API types ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct OpenAiRequest<'a> {
    model: &'a str,
    max_completion_tokens: u32,
    messages: Vec<OpenAiMessage<'a>>,
}

#[derive(Serialize)]
struct OpenAiMessage<'a> {
    role: &'a str,
    content: Vec<OpenAiContentBlock<'a>>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum OpenAiContentBlock<'a> {
    Text {
        #[serde(rename = "type")]
        kind: &'a str,
        text: &'a str,
    },
    Image {
        #[serde(rename = "type")]
        kind: &'a str,
        image_url: OpenAiImageUrl<'a>,
    },
}

#[derive(Serialize)]
struct OpenAiImageUrl<'a> {
    url: String,
    #[serde(skip)]
    _phantom: std::marker::PhantomData<&'a str>,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: OpenAiUsage,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
}

#[derive(Deserialize)]
struct OpenAiChoiceMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

// ── LlmClient ─────────────────────────────────────────────────────────────

/// Provider-agnostic LLM client for semantic extraction.
pub struct LlmClient {
    provider: AiProvider,
    api_key: String,
    model: String,
    http: reqwest::Client,
}

impl LlmClient {
    pub fn new(provider: AiProvider, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider,
            api_key: api_key.into(),
            model: model.into(),
            http: reqwest::Client::new(),
        }
    }

    /// Send a request and return `(response_text, input_tokens, output_tokens)`.
    ///
    /// Routes to the correct request format based on `self.provider`.
    pub async fn messages(
        &self,
        system: &str,
        content: &[ContentBlock],
    ) -> crate::Result<(String, u64, u64)> {
        match self.provider.request_format() {
            RequestFormat::AnthropicMessages => self.anthropic_messages(system, content).await,
            RequestFormat::OpenAIChatCompletions => {
                self.openai_chat_completions(system, content).await
            }
        }
    }

    // ── Anthropic path ────────────────────────────────────────────────────

    async fn anthropic_messages(
        &self,
        system: &str,
        content: &[ContentBlock],
    ) -> crate::Result<(String, u64, u64)> {
        let mut parse_retries = 0u32;

        'retry: for attempt in 0u32..4 {
            let anthro_content: Vec<AnthropicContentBlock> = content
                .iter()
                .map(|b| match b {
                    ContentBlock::Text { text } => AnthropicContentBlock::Text { text },
                    ContentBlock::Image { media_type, data } => AnthropicContentBlock::Image {
                        source: AnthropicImageSource {
                            kind: "base64",
                            media_type,
                            data,
                        },
                    },
                })
                .collect();

            let body = AnthropicRequest {
                model: &self.model,
                max_tokens: 8192,
                system,
                messages: vec![AnthropicMessage {
                    role: "user",
                    content: anthro_content,
                }],
            };

            let resp = match self
                .http
                .post(self.provider.endpoint_url())
                .header(
                    self.provider.auth_header_name(),
                    self.provider.auth_header_value(&self.api_key),
                )
                .header("anthropic-version", "2023-06-01")
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => return Err(crate::GrapheniumError::Http(e)),
            };

            let status = resp.status().as_u16();

            match status {
                429 => {
                    let secs = 1u64 << attempt.min(3);
                    eprintln!(
                        "[graphenium] rate limited (429), back-off {secs}s (attempt {attempt})"
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(secs)).await;
                    continue 'retry;
                }
                500 | 502 | 503 if attempt == 0 => {
                    eprintln!("[graphenium] server error ({status}), retrying after 2 s");
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    continue 'retry;
                }
                s if s >= 400 => {
                    let msg = resp.text().await.unwrap_or_default();
                    return Err(crate::GrapheniumError::Api {
                        status: s,
                        message: msg,
                    });
                }
                _ => {}
            }

            let api_resp: AnthropicResponse = match resp.json().await {
                Ok(r) => r,
                Err(e) => {
                    parse_retries += 1;
                    if parse_retries <= 2 {
                        eprintln!(
                            "[graphenium] API response parse error, retrying ({parse_retries}/2)"
                        );
                        continue 'retry;
                    }
                    return Err(crate::GrapheniumError::Http(e));
                }
            };

            let text = api_resp
                .content
                .into_iter()
                .filter_map(|b| if b.kind == "text" { b.text } else { None })
                .collect::<Vec<_>>()
                .join("\n");

            return Ok((
                text,
                api_resp.usage.input_tokens,
                api_resp.usage.output_tokens,
            ));
        }

        Err(crate::GrapheniumError::Api {
            status: 429,
            message: "max retries exceeded".to_string(),
        })
    }

    // ── OpenAI path ───────────────────────────────────────────────────────

    async fn openai_chat_completions(
        &self,
        system: &str,
        content: &[ContentBlock],
    ) -> crate::Result<(String, u64, u64)> {
        let mut parse_retries = 0u32;

        'retry: for attempt in 0u32..4 {
            let openai_content: Vec<OpenAiContentBlock> = content
                .iter()
                .map(|b| match b {
                    ContentBlock::Text { text } => OpenAiContentBlock::Text { kind: "text", text },
                    ContentBlock::Image { media_type, data } => OpenAiContentBlock::Image {
                        kind: "image_url",
                        image_url: OpenAiImageUrl {
                            url: format!("data:{media_type};base64,{data}"),
                            _phantom: std::marker::PhantomData,
                        },
                    },
                })
                .collect();

            let messages = vec![
                OpenAiMessage {
                    role: "system",
                    content: vec![OpenAiContentBlock::Text {
                        kind: "text",
                        text: system,
                    }],
                },
                OpenAiMessage {
                    role: "user",
                    content: openai_content,
                },
            ];

            let body = OpenAiRequest {
                model: &self.model,
                max_completion_tokens: 8192,
                messages,
            };

            let resp = match self
                .http
                .post(self.provider.endpoint_url())
                .header(
                    self.provider.auth_header_name(),
                    self.provider.auth_header_value(&self.api_key),
                )
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => return Err(crate::GrapheniumError::Http(e)),
            };

            let status = resp.status().as_u16();

            match status {
                429 => {
                    let secs = 1u64 << attempt.min(3);
                    eprintln!(
                        "[graphenium] rate limited (429), back-off {secs}s (attempt {attempt})"
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(secs)).await;
                    continue 'retry;
                }
                500 | 502 | 503 if attempt == 0 => {
                    eprintln!("[graphenium] server error ({status}), retrying after 2 s");
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    continue 'retry;
                }
                s if s >= 400 => {
                    let msg = resp.text().await.unwrap_or_default();
                    return Err(crate::GrapheniumError::Api {
                        status: s,
                        message: msg,
                    });
                }
                _ => {}
            }

            let api_resp: OpenAiResponse = match resp.json().await {
                Ok(r) => r,
                Err(e) => {
                    parse_retries += 1;
                    if parse_retries <= 2 {
                        eprintln!(
                            "[graphenium] API response parse error, retrying ({parse_retries}/2)"
                        );
                        continue 'retry;
                    }
                    return Err(crate::GrapheniumError::Http(e));
                }
            };

            let text = api_resp
                .choices
                .into_iter()
                .filter_map(|c| c.message.content)
                .collect::<Vec<_>>()
                .join("\n");

            return Ok((
                text,
                api_resp.usage.prompt_tokens,
                api_resp.usage.completion_tokens,
            ));
        }

        Err(crate::GrapheniumError::Api {
            status: 429,
            message: "max retries exceeded".to_string(),
        })
    }
}

// ── Backward-compat alias ─────────────────────────────────────────────────

/// ClaudeClient is kept as a backward-compatible alias around LlmClient.
/// Prefer LlmClient for new code.
pub struct ClaudeClient {
    inner: LlmClient,
}

impl ClaudeClient {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            inner: LlmClient::new(AiProvider::Anthropic, api_key, model),
        }
    }

    pub async fn messages(
        &self,
        system: &str,
        content: Vec<ContentBlock>,
    ) -> crate::Result<(String, u64, u64)> {
        self.inner.messages(system, &content).await
    }
}

// ── Serialize helper ──────────────────────────────────────────────────────

impl Serialize for ContentBlock {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ContentBlock::Text { text } => {
                use serde::ser::SerializeStruct;
                let mut s = serializer.serialize_struct("ContentBlock", 2)?;
                s.serialize_field("type", "text")?;
                s.serialize_field("text", text)?;
                s.end()
            }
            ContentBlock::Image { .. } => Err(serde::ser::Error::custom(
                "ContentBlock::serialize is not used directly; use format-specific types",
            )),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ContentBlock ──────────────────────────────────────────────────────

    #[test]
    fn content_block_text_creates_correctly() {
        let b = ContentBlock::text("hello world");
        match b {
            ContentBlock::Text { text } => assert_eq!(text, "hello world"),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn content_block_image_creates_correctly() {
        let b = ContentBlock::image("image/png", "base64data==");
        match b {
            ContentBlock::Image { media_type, data } => {
                assert_eq!(media_type, "image/png");
                assert_eq!(data, "base64data==");
            }
            _ => panic!("expected image"),
        }
    }

    // ── Anthropic serialization ───────────────────────────────────────────

    #[test]
    fn anthropic_request_body_structure() {
        let content = vec![ContentBlock::text("test")];
        let anthro_content: Vec<AnthropicContentBlock> = content
            .iter()
            .map(|b| match b {
                ContentBlock::Text { text } => AnthropicContentBlock::Text { text },
                _ => panic!("unexpected image in text test"),
            })
            .collect();

        let body = AnthropicRequest {
            model: "claude-sonnet-4-20250514",
            max_tokens: 8192,
            system: "sys",
            messages: vec![AnthropicMessage {
                role: "user",
                content: anthro_content,
            }],
        };
        let v: serde_json::Value = serde_json::to_value(&body).unwrap();
        assert_eq!(v["model"], "claude-sonnet-4-20250514");
        assert_eq!(v["max_tokens"], 8192);
        assert_eq!(v["system"], "sys");
        assert_eq!(v["messages"][0]["role"], "user");
        assert_eq!(v["messages"][0]["content"][0]["type"], "text");
    }

    #[test]
    fn anthropic_serializes_image_block() {
        let content = vec![ContentBlock::image("image/png", "abc")];
        let anthro: Vec<AnthropicContentBlock> = content
            .iter()
            .map(|b| match b {
                ContentBlock::Image { media_type, data } => AnthropicContentBlock::Image {
                    source: AnthropicImageSource {
                        kind: "base64",
                        media_type,
                        data,
                    },
                },
                _ => panic!("unexpected text"),
            })
            .collect();

        let body = AnthropicRequest {
            model: "claude-sonnet-4-20250514",
            max_tokens: 8192,
            system: "sys",
            messages: vec![AnthropicMessage {
                role: "user",
                content: anthro,
            }],
        };
        let v: serde_json::Value = serde_json::to_value(&body).unwrap();
        let c = &v["messages"][0]["content"][0];
        assert_eq!(c["type"], "image");
        assert_eq!(c["source"]["type"], "base64");
        assert_eq!(c["source"]["media_type"], "image/png");
        assert_eq!(c["source"]["data"], "abc");
    }

    // ── OpenAI serialization ──────────────────────────────────────────────

    #[test]
    fn openai_request_body_structure() {
        let content = vec![ContentBlock::text("hello")];
        let openai_content: Vec<OpenAiContentBlock> = content
            .iter()
            .map(|b| match b {
                ContentBlock::Text { text } => OpenAiContentBlock::Text { kind: "text", text },
                _ => panic!("unexpected image"),
            })
            .collect();

        let body = OpenAiRequest {
            model: "gpt-4o",
            max_completion_tokens: 8192,
            messages: vec![
                OpenAiMessage {
                    role: "system",
                    content: vec![OpenAiContentBlock::Text {
                        kind: "text",
                        text: "sys",
                    }],
                },
                OpenAiMessage {
                    role: "user",
                    content: openai_content,
                },
            ],
        };
        let v: serde_json::Value = serde_json::to_value(&body).unwrap();
        assert_eq!(v["model"], "gpt-4o");
        assert_eq!(v["max_completion_tokens"], 8192);
        assert_eq!(v["messages"][0]["role"], "system");
        assert_eq!(v["messages"][0]["content"][0]["text"], "sys");
        assert_eq!(v["messages"][1]["role"], "user");
        assert_eq!(v["messages"][1]["content"][0]["type"], "text");
        assert_eq!(v["messages"][1]["content"][0]["text"], "hello");
    }

    #[test]
    fn openai_serializes_image_as_data_uri() {
        let content = vec![ContentBlock::image("image/png", "abc")];
        let openai_content: Vec<OpenAiContentBlock> = content
            .iter()
            .map(|b| match b {
                ContentBlock::Image { media_type, data } => OpenAiContentBlock::Image {
                    kind: "image_url",
                    image_url: OpenAiImageUrl {
                        url: format!("data:{media_type};base64,{data}"),
                        _phantom: std::marker::PhantomData,
                    },
                },
                _ => panic!("unexpected text"),
            })
            .collect();

        let body = OpenAiRequest {
            model: "gpt-4o",
            max_completion_tokens: 8192,
            messages: vec![OpenAiMessage {
                role: "user",
                content: openai_content,
            }],
        };
        let v: serde_json::Value = serde_json::to_value(&body).unwrap();
        let c = &v["messages"][0]["content"][0];
        assert_eq!(c["type"], "image_url");
        assert!(c["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,abc"));
    }

    #[test]
    fn openai_response_deserialization() {
        let json = r#"{
            "choices": [{"message": {"content": "{\"nodes\":[]}"}}],
            "usage": {"prompt_tokens": 100, "completion_tokens": 50}
        }"#;
        let resp: OpenAiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            resp.choices[0].message.content.as_deref().unwrap(),
            "{\"nodes\":[]}"
        );
        assert_eq!(resp.usage.prompt_tokens, 100);
        assert_eq!(resp.usage.completion_tokens, 50);
    }

    #[test]
    fn openai_response_null_content_is_filtered() {
        let json = r#"{
            "choices": [{"message": {"content": null}}],
            "usage": {"prompt_tokens": 0, "completion_tokens": 0}
        }"#;
        let resp: OpenAiResponse = serde_json::from_str(json).unwrap();
        assert!(resp.choices[0].message.content.is_none());
    }

    // ── LlmClient construction ────────────────────────────────────────────

    #[test]
    fn llm_client_constructs_for_each_provider() {
        for (provider, model) in [
            (AiProvider::Anthropic, "claude-sonnet-4-20250514"),
            (AiProvider::OpenAI, "gpt-4o"),
            (AiProvider::DeepSeek, "deepseek-chat"),
            (AiProvider::OpenRouter, "anthropic/claude-sonnet-4"),
        ] {
            let client = LlmClient::new(provider.clone(), "key", model);
            assert_eq!(client.model, model);
        }
    }

    #[test]
    fn claude_client_is_backward_compatible() {
        let client = ClaudeClient::new("key", "claude-sonnet-4-20250514");
        assert_eq!(client.inner.provider, AiProvider::Anthropic);
        assert_eq!(client.inner.model, "claude-sonnet-4-20250514");
    }
}
