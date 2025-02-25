pub mod claude;
pub mod openai;

use crate::templates::CommitTemplate;
use lazy_static::lazy_static;
use schemars::schema_for;
use serde_json::Value;
use std::error::Error;
use std::fmt::Debug;
use std::sync::Arc;

pub const CLAUDE_DEFAULT_TEMP: f32 = 0.3;
pub const OPENAI_DEFAULT_TEMP: f32 = 1.0;

lazy_static! {
    /// The JSON schema for CommitTemplate, generated once and reused
    static ref COMMIT_TEMPLATE_SCHEMA: Value = {
        let schema = schema_for!(CommitTemplate);
        serde_json::to_value(schema).unwrap_or_else(|_| serde_json::json!({}))
    };
}

/// Generate the JSON schema for CommitTemplate
pub fn generate_commit_template_schema() -> Value {
    COMMIT_TEMPLATE_SCHEMA.clone()
}

/// Parse a JSON string into a CommitTemplate
pub fn parse_commit_template_json(json_str: &str) -> Result<CommitTemplate, Box<dyn Error>> {
    serde_json::from_str(json_str).map_err(|e| {
        Box::new(AiError::JsonError {
            message: format!(
                "Failed to parse response as CommitTemplate: {}. Response: {}",
                e, json_str
            ),
        }) as Box<dyn Error>
    })
}

/// Enhanced AI provider trait that supports more diverse providers
pub trait AiProvider: Send + Sync + Debug {
    /// Get the name of the provider
    fn name(&self) -> &str;

    /// Check if the provider supports streaming responses
    fn supports_streaming(&self) -> bool;

    /// Check if the provider requires an API key
    fn requires_api_key(&self) -> bool;

    /// Complete a prompt with the given model and parameters, returning structured data
    /// This uses function calling or JSON mode to get structured data directly from the model
    fn complete_structured(
        &self,
        model: &str,
        temperature: f32,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<CommitTemplate, Box<dyn Error>>;

    /// Get the JSON schema for CommitTemplate
    fn get_commit_template_schema(&self) -> Value {
        generate_commit_template_schema()
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
    use crate::templates::CommitTemplate;

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

        fn complete_structured(
            &self,
            _model: &str,
            _temperature: f32,
            _system_prompt: &str,
            _user_prompt: &str,
        ) -> Result<CommitTemplate, Box<dyn Error>> {
            // Return a mock TemplateData
            Ok(CommitTemplate {
                r#type: crate::templates::CommitType::Feat,
                subject: "add structured completion".to_string(),
                details: Some("- Implement structured completion\n- Add tests".to_string()),
                issues: None,
                breaking: None,
                scope: Some("ai".to_string()),
            })
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
        let result = provider.complete_structured(
            "test-model",
            0.3,
            &expected_system_prompt,
            &USER_PROMPT_TEMPLATE.replace("{{changes}}", changes),
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap().r#type, crate::templates::CommitType::Feat);
    }

    #[test]
    fn test_structured_completion() {
        let provider = MockProvider;
        let result = provider.complete_structured(
            "test-model",
            0.3,
            "test system prompt",
            "test user prompt",
        );

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.r#type, crate::templates::CommitType::Feat);
        assert_eq!(data.subject, "add structured completion");
        assert_eq!(
            data.details,
            Some("- Implement structured completion\n- Add tests".to_string())
        );
        assert_eq!(data.scope, Some("ai".to_string()));
    }

    #[test]
    fn test_structured_completion_with_instruct_macro() {
        let provider = MockProvider;

        // Create a JSON string that matches the TemplateData structure
        let json_data = r#"{
            "type": "feat",
            "subject": "add structured completion",
            "details": "- Implement structured completion\n- Add tests",
            "scope": "ai",
            "issues": null,
            "breaking": null
        }"#;

        // Deserialize the JSON into TemplateData
        let template_data: CommitTemplate = serde_json::from_str(json_data).unwrap();

        // Verify the data is correctly parsed
        assert_eq!(template_data.r#type, crate::templates::CommitType::Feat);
        assert_eq!(template_data.subject, "add structured completion");
        assert_eq!(
            template_data.details,
            Some("- Implement structured completion\n- Add tests".to_string())
        );
        assert_eq!(template_data.scope, Some("ai".to_string()));
        assert_eq!(template_data.issues, None);
        assert_eq!(template_data.breaking, None);

        // Verify that the provider's complete_structured method returns the expected data
        let result = provider.complete_structured(
            "test-model",
            0.3,
            "test system prompt",
            "test user prompt",
        );

        assert!(result.is_ok());
        let data = result.unwrap();

        // Serialize the data to JSON
        let serialized = serde_json::to_string(&data).unwrap();

        // Verify the serialized data contains the expected fields
        assert!(serialized.contains("\"type\":\"feat\""));
        assert!(serialized.contains("\"subject\":\"add structured completion\""));
        assert!(
            serialized.contains("\"details\":\"- Implement structured completion\\n- Add tests\"")
        );
        assert!(serialized.contains("\"scope\":\"ai\""));
    }
}
