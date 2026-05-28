//! AI provider module using rstructor for structured LLM outputs

use crate::templates::CommitTemplate;
use rstructor::{
    ApiErrorKind, LLMClient, ModelInfo, RStructorError, ThinkingLevel as RstructorThinkingLevel,
    TokenUsage,
};
use std::error::Error;
use std::time::Duration;

/// Default temperature for commit message generation.
///
/// Low by design: this is a structured, validated extraction task (classify the
/// change + summarize it), so determinism matters more than variety. The
/// `--temperature` flag / `temperature` config key still override it.
pub const DEFAULT_TEMPERATURE: f32 = 0.2;

/// Result of a completion request, including token usage
#[derive(Debug)]
pub struct CompletionResult {
    /// The generated commit template
    pub template: CommitTemplate,
    /// Token usage information (if available)
    pub usage: Option<TokenUsage>,
}

/// Thinking/reasoning level for models that support it
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThinkingLevel {
    /// No reasoning - fastest
    Off,
    /// Minimal thinking - very fast
    Minimal,
    /// Low thinking - balanced speed and reasoning (default)
    #[default]
    Low,
    /// High thinking - most thorough reasoning
    High,
}

impl ThinkingLevel {
    /// Parse from string (for CLI parsing)
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "none" | "off" => ThinkingLevel::Off,
            "minimal" => ThinkingLevel::Minimal,
            "low" => ThinkingLevel::Low,
            "high" => ThinkingLevel::High,
            _ => ThinkingLevel::Low,
        }
    }

    /// Convert to rstructor ThinkingLevel
    fn as_rstructor(self) -> RstructorThinkingLevel {
        match self {
            ThinkingLevel::Off => RstructorThinkingLevel::Off,
            ThinkingLevel::Minimal => RstructorThinkingLevel::Minimal,
            ThinkingLevel::Low => RstructorThinkingLevel::Low,
            ThinkingLevel::High => RstructorThinkingLevel::High,
        }
    }
}

/// Available AI providers
pub const PROVIDERS: &[&str] = &["claude", "openai", "gemini"];

/// Default models for each provider.
///
/// Single source of truth: these reference the constants in [`crate::config::defaults`]
/// so the model strings live in exactly one place and cannot drift.
pub fn default_model(provider: &str) -> &'static str {
    use crate::config::defaults;
    match provider.to_lowercase().as_str() {
        "claude" => defaults::DEFAULT_CLAUDE_MODEL,
        "openai" => defaults::DEFAULT_OPENAI_MODEL,
        "gemini" => defaults::DEFAULT_GEMINI_MODEL,
        _ => defaults::DEFAULT_OPENAI_MODEL,
    }
}

/// Get the environment variable name for a provider's API key
pub fn api_key_env_var(provider: &str) -> &'static str {
    match provider.to_lowercase().as_str() {
        "claude" => "ANTHROPIC_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "gemini" => "GEMINI_API_KEY",
        _ => "OPENAI_API_KEY",
    }
}

/// Check if a provider is available (exists and has API key set)
pub fn check_available(provider: &str) -> Result<(), AiError> {
    // First check if provider is valid
    if !PROVIDERS.contains(&provider.to_lowercase().as_str()) {
        return Err(AiError::ProviderNotFound {
            provider_name: provider.to_string(),
        });
    }

    // Then check if API key is set
    let env_var = api_key_env_var(provider);
    if std::env::var(env_var).is_err() {
        return Err(AiError::ProviderNotAvailable {
            provider_name: provider.to_string(),
            message: format!("{} environment variable not set", env_var),
        });
    }
    Ok(())
}

/// Normalize the thinking level for a provider's quirks.
///
/// Claude's structured-output path is unreliable with `low` thinking under
/// rstructor's current max_tokens/budget_tokens handling, so coerce `Low` to
/// `Off` for Claude. This lives here (next to the provider call) rather than in
/// the generation logic so all provider quirks are in one place.
fn normalize_thinking(provider: &str, level: ThinkingLevel) -> ThinkingLevel {
    if provider.eq_ignore_ascii_case("claude") && level == ThinkingLevel::Low {
        ThinkingLevel::Off
    } else {
        level
    }
}

/// Generate a structured commit template from the AI provider.
///
/// Returns the template along with token usage information. Each provider's
/// HTTP request is bounded by `timeout_secs` so a hung endpoint can't stall the
/// tool indefinitely; rstructor retries transient (429/5xx) failures internally.
pub async fn complete_structured(
    provider: &str,
    model: &str,
    temperature: f32,
    system_prompt: &str,
    user_prompt: &str,
    thinking_level: Option<ThinkingLevel>,
    timeout_secs: u64,
) -> Result<CompletionResult, Box<dyn Error>> {
    // Check provider is available
    check_available(provider)?;

    let thinking = normalize_thinking(provider, thinking_level.unwrap_or_default()).as_rstructor();

    // Build prompt combining system and user prompts
    let full_prompt = format!("{}\n\n{}", system_prompt, user_prompt);
    let timeout = Duration::from_secs(timeout_secs);

    use rstructor::{AnthropicClient, GeminiClient, OpenAIClient};

    // The three clients are distinct concrete types but share an identical call
    // shape; a macro collapses them into one body so cross-cutting concerns
    // (timeout, error mapping) live in exactly one place.
    macro_rules! materialize_with {
        ($Client:ty) => {{
            let client = <$Client>::from_env()
                .map_err(|e| Box::new(e) as Box<dyn Error>)?
                .model(model)
                .temperature(temperature)
                .thinking_level(thinking)
                .timeout(timeout);
            client
                .materialize_with_metadata::<CommitTemplate>(&full_prompt)
                .await
                .map(|r| CompletionResult {
                    template: r.data,
                    usage: r.usage,
                })
                .map_err(|e| Box::new(map_rstructor_error(e, model)) as Box<dyn Error>)
        }};
    }

    match provider.to_lowercase().as_str() {
        "claude" => materialize_with!(AnthropicClient),
        "openai" => materialize_with!(OpenAIClient),
        "gemini" => materialize_with!(GeminiClient),
        _ => Err(Box::new(AiError::ProviderNotFound {
            provider_name: provider.to_string(),
        }) as Box<dyn Error>),
    }
}

/// List available models for a provider
pub async fn list_models(provider: &str) -> Result<Vec<String>, Box<dyn Error>> {
    // Check provider is available
    check_available(provider)?;

    use rstructor::{AnthropicClient, GeminiClient, OpenAIClient};

    macro_rules! list_with {
        ($Client:ty) => {{
            <$Client>::from_env()
                .map_err(|e| Box::new(e) as Box<dyn Error>)?
                .list_models()
                .await
                .map_err(|e| Box::new(e) as Box<dyn Error>)
        }};
    }

    let models: Vec<ModelInfo> = match provider.to_lowercase().as_str() {
        "claude" => list_with!(AnthropicClient),
        "openai" => list_with!(OpenAIClient),
        "gemini" => list_with!(GeminiClient),
        _ => Err(Box::new(AiError::ProviderNotFound {
            provider_name: provider.to_string(),
        }) as Box<dyn Error>),
    }?;

    // Extract model IDs from ModelInfo
    Ok(models.into_iter().map(|m| m.id).collect())
}

/// Map a rstructor error to a typed, actionable [`AiError`].
///
/// Uses rstructor's classified [`ApiErrorKind`] rather than matching on the
/// error's display string, so each real-world failure (bad key, unknown model,
/// oversized request, rate limit, ...) produces a specific, useful message.
fn map_rstructor_error(err: RStructorError, model: &str) -> AiError {
    match &err {
        RStructorError::Timeout => AiError::Timeout,
        RStructorError::ApiError { provider, kind } => match kind {
            ApiErrorKind::InvalidModel {
                model: api_model,
                suggestion,
            } => AiError::InvalidModel {
                // Prefer the model cmt actually sent if the API echoed something empty.
                model: if api_model.is_empty() {
                    model.to_string()
                } else {
                    api_model.clone()
                },
                suggestion: suggestion.clone(),
            },
            ApiErrorKind::AuthenticationFailed => AiError::Auth {
                provider: provider.clone(),
                message: format!(
                    "API key is invalid, expired, or missing (set {})",
                    api_key_env_var(provider)
                ),
            },
            ApiErrorKind::PermissionDenied => AiError::Auth {
                provider: provider.clone(),
                message: "API key lacks permission for this model or operation".to_string(),
            },
            ApiErrorKind::RequestTooLarge => AiError::RequestTooLarge,
            ApiErrorKind::RateLimited { .. } => AiError::RateLimited {
                provider: provider.clone(),
            },
            ApiErrorKind::GatewayError { code } | ApiErrorKind::ServerError { code } => {
                AiError::ApiError {
                    code: *code,
                    message: err.to_string(),
                }
            }
            ApiErrorKind::Other { code, message } => AiError::ApiError {
                code: *code,
                message: message.clone(),
            },
            _ => AiError::ApiError {
                code: 0,
                message: err.to_string(),
            },
        },
        _ => AiError::Other(err.to_string()),
    }
}

// Error type for AI provider operations
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("Provider not found: {provider_name}")]
    ProviderNotFound { provider_name: String },

    #[error("Provider not available: {provider_name} - {message}")]
    ProviderNotAvailable {
        provider_name: String,
        message: String,
    },

    #[error("API error: {code} {message}")]
    ApiError { code: u16, message: String },

    #[error("Invalid model: {model}{}", .suggestion.as_ref().map(|s| format!(" (did you mean \"{s}\"?)")).unwrap_or_default())]
    InvalidModel {
        model: String,
        suggestion: Option<String>,
    },

    #[error("Authentication failed for {provider}: {message}")]
    Auth { provider: String, message: String },

    #[error("Request too large: the diff is too big for the model. Try a smaller --max-file-lines, staging fewer files, or adding large files to .cmtignore")]
    RequestTooLarge,

    #[error(
        "Rate limited by {provider}; the request was retried but kept failing. Try again shortly."
    )]
    RateLimited { provider: String },

    #[error(
        "Request timed out. Increase the limit with --timeout <secs> if the model is just slow."
    )]
    Timeout,

    #[error("{0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_level_parse() {
        assert_eq!(ThinkingLevel::parse("none"), ThinkingLevel::Off);
        assert_eq!(ThinkingLevel::parse("off"), ThinkingLevel::Off);
        assert_eq!(ThinkingLevel::parse("minimal"), ThinkingLevel::Minimal);
        assert_eq!(ThinkingLevel::parse("low"), ThinkingLevel::Low);
        assert_eq!(ThinkingLevel::parse("high"), ThinkingLevel::High);
        assert_eq!(ThinkingLevel::parse("unknown"), ThinkingLevel::Low);
    }

    #[test]
    fn test_default_models() {
        use crate::config::defaults;
        // default_model() must stay in sync with the constants (single source of truth).
        assert_eq!(default_model("claude"), defaults::DEFAULT_CLAUDE_MODEL);
        assert_eq!(default_model("openai"), defaults::DEFAULT_OPENAI_MODEL);
        assert_eq!(default_model("gemini"), defaults::DEFAULT_GEMINI_MODEL);
        // The Gemini default is the maintained GA Flash model.
        assert_eq!(default_model("gemini"), "gemini-3.5-flash");
    }

    #[test]
    fn test_api_key_env_var() {
        assert_eq!(api_key_env_var("claude"), "ANTHROPIC_API_KEY");
        assert_eq!(api_key_env_var("openai"), "OPENAI_API_KEY");
        assert_eq!(api_key_env_var("gemini"), "GEMINI_API_KEY");
    }

    #[test]
    fn test_normalize_thinking_claude_low_becomes_off() {
        assert_eq!(
            normalize_thinking("claude", ThinkingLevel::Low),
            ThinkingLevel::Off
        );
        assert_eq!(
            normalize_thinking("Claude", ThinkingLevel::High),
            ThinkingLevel::High
        );
        // Other providers keep Low.
        assert_eq!(
            normalize_thinking("gemini", ThinkingLevel::Low),
            ThinkingLevel::Low
        );
    }

    #[test]
    fn test_map_invalid_model_keeps_suggestion() {
        let err = RStructorError::api_error(
            "Gemini",
            ApiErrorKind::InvalidModel {
                model: "gemini-bogus".to_string(),
                suggestion: Some("gemini-3.5-flash".to_string()),
            },
        );
        match map_rstructor_error(err, "gemini-bogus") {
            AiError::InvalidModel { model, suggestion } => {
                assert_eq!(model, "gemini-bogus");
                assert_eq!(suggestion.as_deref(), Some("gemini-3.5-flash"));
            }
            other => panic!("expected InvalidModel, got {other:?}"),
        }
    }

    #[test]
    fn test_map_classifies_auth_too_large_timeout() {
        assert!(matches!(
            map_rstructor_error(
                RStructorError::api_error("OpenAI", ApiErrorKind::AuthenticationFailed),
                "m"
            ),
            AiError::Auth { .. }
        ));
        assert!(matches!(
            map_rstructor_error(
                RStructorError::api_error("OpenAI", ApiErrorKind::RequestTooLarge),
                "m"
            ),
            AiError::RequestTooLarge
        ));
        assert!(matches!(
            map_rstructor_error(RStructorError::Timeout, "m"),
            AiError::Timeout
        ));
    }

    #[test]
    fn test_invalid_model_message_includes_suggestion() {
        let e = AiError::InvalidModel {
            model: "foo".to_string(),
            suggestion: Some("bar".to_string()),
        };
        let s = e.to_string();
        assert!(s.contains("foo"), "message: {s}");
        assert!(s.contains("bar"), "message: {s}");
    }
}
