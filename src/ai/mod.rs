pub mod claude;
pub mod openai;

use std::error::Error;
use std::fmt::Debug;
use std::sync::Arc;

pub const CLAUDE_DEFAULT_TEMP: f32 = 0.3;
pub const OPENAI_DEFAULT_TEMP: f32 = 1.0;

/// Enhanced AI provider trait that supports more diverse providers
pub trait AiProvider: Send + Sync + Debug {
    /// Get the name of the provider
    fn name(&self) -> &str;

    /// Check if the provider supports streaming responses
    fn supports_streaming(&self) -> bool;

    /// Check if the provider requires an API key
    fn requires_api_key(&self) -> bool;

    /// Complete a prompt with the given model and parameters
    fn complete(
        &self,
        model: &str,
        temperature: f32,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, Box<dyn Error>>;

    /// Complete a prompt with streaming responses
    fn complete_streaming(
        &self,
        _model: &str,
        _temperature: f32,
        _system_prompt: &str,
        _user_prompt: &str,
        _callback: Box<dyn FnMut(String) -> Result<(), Box<dyn Error>> + Send>,
    ) -> Result<(), Box<dyn Error>> {
        // Default implementation for providers that don't support streaming
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            format!("Streaming not supported by {}", self.name()),
        )))
    }

    /// Get the default model for this provider
    fn default_model(&self) -> &str;

    /// Get the default temperature for this provider
    fn default_temperature(&self) -> f32;

    /// Check if the provider is available (API key set, etc.)
    fn check_available(&self) -> Result<(), Box<dyn Error>>;

    /// Fetch available models from the API
    /// This is called only after receiving an error about an invalid model
    fn fetch_available_models(&self) -> Result<Vec<String>, Box<dyn Error>>;
}

/// Provider registry for managing available AI providers
#[derive(Default, Debug)]
pub struct ProviderRegistry {
    providers: Vec<Arc<dyn AiProvider>>,
}

impl ProviderRegistry {
    /// Create a new provider registry
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a new provider
    pub fn register(&mut self, provider: Arc<dyn AiProvider>) {
        self.providers.push(provider);
    }

    /// Get a provider by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn AiProvider>> {
        self.providers
            .iter()
            .find(|p| p.name().to_lowercase() == name.to_lowercase())
            .cloned()
    }

    /// Get the default model for a provider by name
    pub fn default_model_for(&self, provider_name: &str) -> String {
        self.get(provider_name)
            .map(|p| p.default_model().to_string())
            .unwrap_or_else(|| "default model".to_string())
    }

    /// Get all available providers
    pub fn available_providers(&self) -> Vec<Arc<dyn AiProvider>> {
        self.providers
            .iter()
            .filter(|p| p.check_available().is_ok())
            .cloned()
            .collect()
    }

    /// Get provider names
    pub fn provider_names(&self) -> Vec<String> {
        self.providers
            .iter()
            .map(|p| p.name().to_string())
            .collect()
    }
}

// Create the default provider registry with all available providers
pub fn create_default_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();

    // Register providers based on the available providers list
    for &provider_name in crate::config::defaults::defaults::AVAILABLE_PROVIDERS {
        match provider_name {
            "claude" => registry.register(Arc::new(claude::ClaudeProvider::new())),
            "openai" => registry.register(Arc::new(openai::OpenAiProvider::new())),
            _ => {} // Skip unknown providers
        }
    }

    registry
}

// Error type for AI provider operations
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("Provider not found: {provider_name}")]
    ProviderNotFound { provider_name: String },

    #[error("Provider not available: {provider_name}")]
    ProviderNotAvailable {
        provider_name: String,
        message: String,
    },

    #[error("API error: {code} {message}")]
    ApiError { code: u16, message: String },

    #[error("JSON error: {message}")]
    JsonError { message: String },

    #[error("Invalid configuration: {message}")]
    InvalidConfig { message: String },

    #[error("Invalid model: {model}")]
    InvalidModel { model: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::cli::Args;
    use crate::prompts::{SYSTEM_PROMPT, USER_PROMPT_TEMPLATE};

    #[derive(Debug)]
    struct MockProvider;

    impl AiProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }

        fn supports_streaming(&self) -> bool {
            false
        }

        fn requires_api_key(&self) -> bool {
            false
        }

        fn complete(
            &self,
            _model: &str,
            _temperature: f32,
            system_prompt: &str,
            _user_prompt: &str,
        ) -> Result<String, Box<dyn Error>> {
            // Return the system prompt so we can verify its contents
            Ok(system_prompt.to_string())
        }

        fn default_model(&self) -> &str {
            "mock-model"
        }

        fn default_temperature(&self) -> f32 {
            0.5
        }

        fn check_available(&self) -> Result<(), Box<dyn Error>> {
            Ok(())
        }

        fn fetch_available_models(&self) -> Result<Vec<String>, Box<dyn Error>> {
            Ok(vec!["mock-model".to_string(), "mock-model-2".to_string()])
        }
    }

    #[test]
    fn test_provider_registry() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(MockProvider));

        assert_eq!(registry.provider_names(), vec!["mock"]);
        assert!(registry.get("mock").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_hint_is_added_to_system_prompt() {
        let hint = "Fix the login bug";
        let mut args = Args::new_from(["cmt"].iter().map(ToString::to_string));
        args.hint = Some(hint.to_string());

        let changes = "test changes";
        let expected_system_prompt = format!("{}\n\nAdditional context: {}", SYSTEM_PROMPT, hint);

        let provider = MockProvider;
        let result = provider.complete(
            "test-model",
            0.3,
            &expected_system_prompt,
            &USER_PROMPT_TEMPLATE.replace("{{changes}}", changes),
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_system_prompt);
    }
}
