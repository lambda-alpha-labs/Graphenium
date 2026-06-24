/// AI provider abstraction.
///
/// Maps provider names to API endpoints, authentication schemes, and request/
/// response formats so the semantic extraction pipeline works with multiple
/// LLM backends without changing the prompt or extraction logic.

// ── Provider enum ─────────────────────────────────────────────────────────

/// Supported AI providers for semantic extraction.
///
/// Each variant carries the information needed to construct the correct
/// HTTP request: base URL, auth header name, and which request format to use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiProvider {
    /// Anthropic Claude API (`https://api.anthropic.com/v1/messages`).
    Anthropic,
    /// OpenAI API (`https://api.openai.com/v1/chat/completions`).
    OpenAI,
    /// OpenRouter (`https://openrouter.ai/api/v1/chat/completions`).
    /// Uses the OpenAI-compatible chat completions format.
    OpenRouter,
    /// DeepSeek API (`https://api.deepseek.com/v1/chat/completions`).
    /// Uses the OpenAI-compatible chat completions format.
    DeepSeek,
    /// Any OpenAI-compatible endpoint. The `base_url` is the full
    /// chat completions URL (e.g. `https://api.example.com/v1/chat/completions`).
    OpenAICompatible { base_url: String },
}

impl AiProvider {
    /// Full URL for chat completions / messages.
    pub fn endpoint_url(&self) -> &str {
        match self {
            AiProvider::Anthropic => "https://api.anthropic.com/v1/messages",
            AiProvider::OpenAI => "https://api.openai.com/v1/chat/completions",
            AiProvider::OpenRouter => "https://openrouter.ai/api/v1/chat/completions",
            AiProvider::DeepSeek => "https://api.deepseek.com/v1/chat/completions",
            AiProvider::OpenAICompatible { base_url } => base_url.as_str(),
        }
    }

    /// Name of the HTTP header that carries the API key.
    pub fn auth_header_name(&self) -> &str {
        match self {
            AiProvider::Anthropic => "x-api-key",
            _ => "Authorization",
        }
    }

    /// Value of the auth header for the given API key.
    /// Anthropic uses bare key; OpenAI-compatible uses `Bearer <key>`.
    pub fn auth_header_value(&self, api_key: &str) -> String {
        match self {
            AiProvider::Anthropic => api_key.to_string(),
            _ => format!("Bearer {api_key}"),
        }
    }

    /// Which request format this provider uses.
    pub fn request_format(&self) -> RequestFormat {
        match self {
            AiProvider::Anthropic => RequestFormat::AnthropicMessages,
            _ => RequestFormat::OpenAIChatCompletions,
        }
    }

    /// Default model for this provider when none is specified.
    pub fn default_model(&self) -> &str {
        match self {
            AiProvider::Anthropic => "claude-sonnet-4-20250514",
            AiProvider::OpenAI => "gpt-4o",
            AiProvider::OpenRouter => "anthropic/claude-sonnet-4",
            AiProvider::DeepSeek => "deepseek-chat",
            AiProvider::OpenAICompatible { .. } => "",
        }
    }

    /// Environment variable checked for the API key when no explicit key is given.
    pub fn env_var_name(&self) -> &str {
        match self {
            AiProvider::Anthropic => "ANTHROPIC_API_KEY",
            AiProvider::OpenAI => "OPENAI_API_KEY",
            AiProvider::OpenRouter => "OPENROUTER_API_KEY",
            AiProvider::DeepSeek => "DEEPSEEK_API_KEY",
            AiProvider::OpenAICompatible { .. } => "GRAPhenium_API_KEY",
        }
    }
}

// ── Request format ────────────────────────────────────────────────────────

/// How to serialize a request to the provider's API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestFormat {
    /// Anthropic Messages API (`POST /v1/messages`).
    /// System prompt is a top-level field. Max tokens is `max_tokens`.
    AnthropicMessages,
    /// OpenAI Chat Completions API (`POST /v1/chat/completions`).
    /// System prompt is a message with `role: "system"`.
    /// Max tokens is `max_completion_tokens` for newer models.
    OpenAIChatCompletions,
}

// ── Parsing ───────────────────────────────────────────────────────────────

impl std::str::FromStr for AiProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "anthropic" => Ok(AiProvider::Anthropic),
            "openai" => Ok(AiProvider::OpenAI),
            "openrouter" => Ok(AiProvider::OpenRouter),
            "deepseek" => Ok(AiProvider::DeepSeek),
            "openai-compatible" => Ok(AiProvider::OpenAICompatible {
                base_url: String::new(),
            }),
            other => Err(format!(
                "Unknown provider '{other}'. Expected: anthropic, openai, openrouter, deepseek, or openai-compatible."
            )),
        }
    }
}

impl std::fmt::Display for AiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiProvider::Anthropic => write!(f, "anthropic"),
            AiProvider::OpenAI => write!(f, "openai"),
            AiProvider::OpenRouter => write!(f, "openrouter"),
            AiProvider::DeepSeek => write!(f, "deepseek"),
            AiProvider::OpenAICompatible { base_url } => {
                write!(f, "openai-compatible ({base_url})")
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_provider_strings() {
        assert_eq!(
            "anthropic".parse::<AiProvider>().unwrap(),
            AiProvider::Anthropic
        );
        assert_eq!("openai".parse::<AiProvider>().unwrap(), AiProvider::OpenAI);
        assert_eq!(
            "openrouter".parse::<AiProvider>().unwrap(),
            AiProvider::OpenRouter
        );
        assert_eq!(
            "deepseek".parse::<AiProvider>().unwrap(),
            AiProvider::DeepSeek
        );
        assert_eq!(
            "openai-compatible".parse::<AiProvider>().unwrap(),
            AiProvider::OpenAICompatible {
                base_url: String::new()
            }
        );
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!(
            "ANTHROPIC".parse::<AiProvider>().unwrap(),
            AiProvider::Anthropic
        );
        assert_eq!("OpenAI".parse::<AiProvider>().unwrap(), AiProvider::OpenAI);
    }

    #[test]
    fn unknown_provider_returns_err() {
        assert!("unknown".parse::<AiProvider>().is_err());
        assert!("".parse::<AiProvider>().is_err());
    }

    #[test]
    fn anthropic_auth_header_is_bare_key() {
        let p = AiProvider::Anthropic;
        assert_eq!(p.auth_header_name(), "x-api-key");
        assert_eq!(p.auth_header_value("sk-ant-abc"), "sk-ant-abc");
    }

    #[test]
    fn openai_auth_header_is_bearer() {
        let p = AiProvider::OpenAI;
        assert_eq!(p.auth_header_name(), "Authorization");
        assert_eq!(p.auth_header_value("sk-abc"), "Bearer sk-abc");
    }

    #[test]
    fn deepseek_auth_header_is_bearer() {
        let p = AiProvider::DeepSeek;
        assert_eq!(p.auth_header_name(), "Authorization");
        assert_eq!(p.auth_header_value("sk-abc"), "Bearer sk-abc");
    }

    #[test]
    fn anthropic_uses_messages_format() {
        assert_eq!(
            AiProvider::Anthropic.request_format(),
            RequestFormat::AnthropicMessages
        );
    }

    #[test]
    fn openai_uses_chat_completions_format() {
        assert_eq!(
            AiProvider::OpenAI.request_format(),
            RequestFormat::OpenAIChatCompletions
        );
        assert_eq!(
            AiProvider::DeepSeek.request_format(),
            RequestFormat::OpenAIChatCompletions
        );
        assert_eq!(
            AiProvider::OpenRouter.request_format(),
            RequestFormat::OpenAIChatCompletions
        );
    }

    #[test]
    fn provider_urls() {
        assert_eq!(
            AiProvider::Anthropic.endpoint_url(),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            AiProvider::OpenAI.endpoint_url(),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            AiProvider::OpenRouter.endpoint_url(),
            "https://openrouter.ai/api/v1/chat/completions"
        );
        assert_eq!(
            AiProvider::DeepSeek.endpoint_url(),
            "https://api.deepseek.com/v1/chat/completions"
        );
        let compat = AiProvider::OpenAICompatible {
            base_url: "https://api.example.com/v1/chat/completions".into(),
        };
        assert_eq!(
            compat.endpoint_url(),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn default_models() {
        assert!(AiProvider::Anthropic.default_model().contains("claude"));
        assert!(AiProvider::OpenAI.default_model().contains("gpt"));
        assert!(AiProvider::DeepSeek.default_model().contains("deepseek"));
    }

    #[test]
    fn env_var_names() {
        assert_eq!(AiProvider::Anthropic.env_var_name(), "ANTHROPIC_API_KEY");
        assert_eq!(AiProvider::OpenAI.env_var_name(), "OPENAI_API_KEY");
        assert_eq!(AiProvider::DeepSeek.env_var_name(), "DEEPSEEK_API_KEY");
        assert_eq!(AiProvider::OpenRouter.env_var_name(), "OPENROUTER_API_KEY");
    }
}
