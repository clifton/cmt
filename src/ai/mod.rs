//! AI provider module using rstructor for structured LLM outputs

use crate::templates::CommitTemplate;
use rstructor::{LLMClient, ModelInfo, ThinkingLevel as RstructorThinkingLevel, TokenUsage};
use std::error::Error;

/// Default temperature for commit message generation
pub const DEFAULT_TEMPERATURE: f32 = 0.3;

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

/// Default models for each provider
pub fn default_model(provider: &str) -> &'static str {
    match provider.to_lowercase().as_str() {
        "claude" => "claude-sonnet-4-5-20250929",
        "openai" => "gpt-5.2",
        "gemini" => "gemini-3-flash-preview",
        _ => "gpt-5.2",
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

/// Generate a structured commit template from the AI provider
/// Returns the template along with token usage information
pub async fn complete_structured(
    provider: &str,
    model: &str,
    temperature: f32,
    system_prompt: &str,
    user_prompt: &str,
    thinking_level: Option<ThinkingLevel>,
) -> Result<CompletionResult, Box<dyn Error>> {
    // Check provider is available
    check_available(provider)?;

    let thinking = thinking_level.unwrap_or_default().as_rstructor();

    // Build prompt combining system and user prompts
    let full_prompt = format!("{}\n\n{}", system_prompt, user_prompt);

    // Execute the appropriate provider
    match provider.to_lowercase().as_str() {
        "claude" => complete_claude(model, temperature, &full_prompt, thinking).await,
        "openai" => complete_openai(model, temperature, &full_prompt, thinking).await,
        "gemini" => complete_gemini(model, temperature, &full_prompt, thinking).await,
        _ => Err(Box::new(AiError::ProviderNotFound {
            provider_name: provider.to_string(),
        }) as Box<dyn Error>),
    }
}

async fn complete_claude(
    model: &str,
    temperature: f32,
    prompt: &str,
    thinking: RstructorThinkingLevel,
) -> Result<CompletionResult, Box<dyn Error>> {
    use rstructor::AnthropicClient;

    let client = AnthropicClient::from_env()?
        .model(model)
        .temperature(temperature)
        .thinking_level(thinking);

    let result = client
        .materialize_with_metadata::<CommitTemplate>(prompt)
        .await
        .map_err(|e| map_rstructor_error(e, model))?;

    Ok(CompletionResult {
        template: result.data,
        usage: result.usage,
    })
}

async fn complete_openai(
    model: &str,
    temperature: f32,
    prompt: &str,
    thinking: RstructorThinkingLevel,
) -> Result<CompletionResult, Box<dyn Error>> {
    use rstructor::OpenAIClient;

    let client = OpenAIClient::from_env()?
        .model(model)
        .temperature(temperature)
        .thinking_level(thinking);

    let result = client
        .materialize_with_metadata::<CommitTemplate>(prompt)
        .await
        .map_err(|e| map_rstructor_error(e, model))?;

    Ok(CompletionResult {
        template: result.data,
        usage: result.usage,
    })
}

async fn complete_gemini(
    model: &str,
    temperature: f32,
    prompt: &str,
    thinking: RstructorThinkingLevel,
) -> Result<CompletionResult, Box<dyn Error>> {
    use rstructor::GeminiClient;

    let client = GeminiClient::from_env()?
        .model(model)
        .temperature(temperature)
        .thinking_level(thinking);

    let result = client
        .materialize_with_metadata::<CommitTemplate>(prompt)
        .await
        .map_err(|e| map_rstructor_error(e, model))?;

    Ok(CompletionResult {
        template: result.data,
        usage: result.usage,
    })
}

/// List available models for a provider
pub async fn list_models(provider: &str) -> Result<Vec<String>, Box<dyn Error>> {
    // Check provider is available
    check_available(provider)?;

    // Execute the appropriate provider's list_models
    let models = match provider.to_lowercase().as_str() {
        "claude" => list_models_claude().await,
        "openai" => list_models_openai().await,
        "gemini" => list_models_gemini().await,
        _ => Err(Box::new(AiError::ProviderNotFound {
            provider_name: provider.to_string(),
        }) as Box<dyn Error>),
    }?;

    // Extract model IDs from ModelInfo
    Ok(models.into_iter().map(|m| m.id).collect())
}

async fn list_models_claude() -> Result<Vec<ModelInfo>, Box<dyn Error>> {
    use rstructor::AnthropicClient;
    let client = AnthropicClient::from_env()?;
    client
        .list_models()
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error>)
}

async fn list_models_openai() -> Result<Vec<ModelInfo>, Box<dyn Error>> {
    use rstructor::OpenAIClient;
    let client = OpenAIClient::from_env()?;
    client
        .list_models()
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error>)
}

async fn list_models_gemini() -> Result<Vec<ModelInfo>, Box<dyn Error>> {
    use rstructor::GeminiClient;
    let client = GeminiClient::from_env()?;
    client
        .list_models()
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error>)
}

/// Map rstructor errors to our error types
fn map_rstructor_error(err: rstructor::RStructorError, model: &str) -> Box<dyn Error> {
    let err_str = err.to_string();

    // Check for model not found errors
    if err_str.contains("model")
        && (err_str.contains("not exist")
            || err_str.contains("not found")
            || err_str.contains("does not exist"))
    {
        return Box::new(AiError::InvalidModel {
            model: model.to_string(),
        });
    }

    // Check for API errors
    if err_str.contains("API") || err_str.contains("status") {
        return Box::new(AiError::ApiError {
            code: 0,
            message: err_str,
        });
    }

    Box::new(err)
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

    #[error("Invalid model: {model}")]
    InvalidModel { model: String },
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
        assert_eq!(default_model("claude"), "claude-sonnet-4-5-20250929");
        assert_eq!(default_model("openai"), "gpt-5.2");
        assert_eq!(default_model("gemini"), "gemini-3-flash-preview");
    }

    #[test]
    fn test_api_key_env_var() {
        assert_eq!(api_key_env_var("claude"), "ANTHROPIC_API_KEY");
        assert_eq!(api_key_env_var("openai"), "OPENAI_API_KEY");
        assert_eq!(api_key_env_var("gemini"), "GEMINI_API_KEY");
    }
}
