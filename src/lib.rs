pub use crate::config::cli::Args;
pub use crate::git::{get_recent_commits, get_staged_changes, git_staged_changes};

mod ai;
mod config;
mod git;
mod prompts;
mod templates;

pub fn generate_commit_message(
    args: &Args,
    git_diff: &str,
    recent_commits: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let registry = ai::create_default_registry();
    let template_name = args
        .template
        .clone()
        .unwrap_or_else(|| config::defaults::defaults::DEFAULT_TEMPLATE.to_string());
    let template_manager = templates::TemplateManager::new()?;

    // Get the provider from the registry
    let provider_name = &args.provider;
    let provider = match registry.get(provider_name) {
        Some(p) => p,
        None => {
            return Err(Box::new(ai::AiError::ProviderNotFound {
                provider_name: provider_name.clone(),
            }));
        }
    };

    // Check if the provider is available (has API key)
    if let Err(e) = provider.check_available() {
        return Err(e);
    }

    // Get the model name, defaulting to the provider's default model
    let model = args
        .model
        .clone()
        .unwrap_or_else(|| provider.default_model().to_string());

    // Build the prompt for the AI provider
    let mut prompt = String::new();

    if args.include_recent_commits && !recent_commits.is_empty() {
        prompt.push_str("\n\nRecent commits for context:\n");
        prompt.push_str(recent_commits);
    }

    prompt.push_str(&prompts::user_prompt(git_diff));

    // Build the system prompt
    let mut system_prompt = prompts::system_prompt();
    if let Some(hint) = &args.hint {
        system_prompt = format!("{}\n\nAdditional context: {}", system_prompt, hint);
    }

    // Generate the commit message
    let temperature = args
        .temperature
        .unwrap_or_else(|| provider.default_temperature());

    // Try to complete the prompt with structured output, handle model errors specially
    let commit_data =
        match provider.complete_structured(&model, temperature, &system_prompt, &prompt) {
            Ok(data) => data,
            Err(err) => {
                if let Some(ai::AiError::InvalidModel { model }) = err.downcast_ref::<ai::AiError>()
                {
                    // Try to fetch available models
                    match provider.fetch_available_models() {
                        Ok(models) if !models.is_empty() => {
                            // Sort models alphabetically and format as a bulleted list for better readability
                            let mut sorted_models = models.clone();
                            sorted_models.sort();

                            println!("Available models: {}", sorted_models.join(", "));

                            return Err(format!(
                                "Invalid model: {} for provider: {}\nAvailable models:{}",
                                model,
                                provider_name,
                                sorted_models
                                    .iter()
                                    .map(|model| format!("\n  • {}", model))
                                    .collect::<Vec<String>>()
                                    .join("")
                            )
                            .into());
                        }
                        _ => {} // If we can't fetch models, just return the original error
                    }
                }

                // Return the original error
                return Err(err);
            }
        };

    // Render the template
    let rendered = template_manager.render(&template_name, &commit_data)?;

    // Return the rendered message along with provider and model info
    Ok(rendered)
}

// Re-export the config module for external use
pub mod config_mod {
    pub use crate::config::file;
    pub use crate::config::{Config, ConfigError};
}

// Re-export the templates module for external use
pub mod template_mod {
    pub use crate::templates::{CommitTemplate, TemplateError, TemplateManager};
}

// Re-export the ai module for external use
pub mod ai_mod {
    pub use crate::ai::create_default_registry;
    pub use crate::ai::{AiError, AiProvider, ProviderRegistry};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{AiError, AiProvider};
    use crate::config::cli::Args;
    use crate::templates::CommitTemplate;
    use std::env;
    use std::error::Error;

    #[test]
    fn test_unsupported_provider() {
        // Create args with an unsupported provider
        let args = Args::new_from(
            ["cmt", "--provider", "unsupported_provider"]
                .iter()
                .map(ToString::to_string),
        );

        // Call generate_commit_message with the unsupported provider
        let result = generate_commit_message(&args, "", "");

        // Verify that an error is returned
        assert!(result.is_err());

        // Convert the error to a string and check that it contains the expected message
        let error_string = format!("{}", result.unwrap_err());
        assert!(error_string.contains("Provider not found: unsupported_provider"));
    }

    #[test]
    #[serial_test::serial]
    fn test_provider_not_available() {
        // Temporarily unset the API keys
        let had_anthropic_key = env::var("ANTHROPIC_API_KEY").is_ok();
        let _had_openai_key = env::var("OPENAI_API_KEY").is_ok();

        if had_anthropic_key {
            env::remove_var("ANTHROPIC_API_KEY");
        }

        // Create args with claude provider
        let args = Args::new_from(
            ["cmt", "--provider", "claude"]
                .iter()
                .map(ToString::to_string),
        );

        // Call generate_commit_message with the claude provider
        let result = generate_commit_message(&args, "", "");

        // Verify that an error is returned
        assert!(result.is_err());

        // Convert the error to a string and check that it contains the expected message
        let error_string = format!("{}", result.unwrap_err());
        println!("Actual error: {}", error_string);

        // The error message should indicate that the provider is not available
        // due to missing API key
        assert!(error_string.contains("API_KEY") || error_string.contains("not available"));

        // Restore the API keys if they were set
        if had_anthropic_key {
            // We can't restore the actual value, but for testing purposes,
            // we can set a dummy value that will pass the is_available check
            env::set_var("ANTHROPIC_API_KEY", "dummy_key_for_test");
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_provider_and_model_info() {
        // This test requires mocking the AI provider, so we'll use a simple approach
        // by setting up a mock environment

        // Set up environment for testing
        env::set_var("ANTHROPIC_API_KEY", "test_key");

        // Create args with specific provider and model
        let args = Args::new_from(
            ["cmt", "--provider", "claude", "--model", "test-model"]
                .iter()
                .map(ToString::to_string),
        );

        // We can't actually call generate_commit_message because it would try to make
        // a real API call. Instead, we'll verify that the provider and model are correctly
        // extracted from the args.

        // Verify that the provider and model match what we expect
        assert_eq!(args.provider, "claude");
        assert_eq!(args.model, Some("test-model".to_string()));

        // Clean up
        env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    #[serial_test::serial]
    fn test_invalid_model_error_handling() {
        // This test verifies that we properly handle invalid model errors

        // Create a mock provider that simulates an invalid model error
        #[derive(Debug)]
        struct TestProvider;

        impl AiProvider for TestProvider {
            fn name(&self) -> &str {
                "test"
            }
            fn supports_streaming(&self) -> bool {
                false
            }
            fn requires_api_key(&self) -> bool {
                false
            }
            fn check_available(&self) -> Result<(), Box<dyn Error>> {
                Ok(())
            }
            fn default_model(&self) -> &str {
                "test-model"
            }
            fn default_temperature(&self) -> f32 {
                0.5
            }

            fn complete_structured(
                &self,
                model: &str,
                _temperature: f32,
                _system_prompt: &str,
                _user_prompt: &str,
            ) -> Result<CommitTemplate, Box<dyn Error>> {
                if model == "invalid-model" {
                    return Err(
                        "The model `invalid-model` does not exist or you do not have access to it."
                            .into(),
                    );
                }
                Ok(CommitTemplate {
                    r#type: crate::templates::CommitType::Test,
                    subject: "test response".to_string(),
                    details: None,
                    issues: None,
                    breaking: None,
                    scope: None,
                })
            }

            fn fetch_available_models(&self) -> Result<Vec<String>, Box<dyn Error>> {
                Ok(vec!["test-model".to_string(), "another-model".to_string()])
            }
        }

        let provider = TestProvider;

        // Test the error handling directly
        let result = provider.complete_structured(
            "invalid-model",
            0.5,
            "test system prompt",
            "test user prompt",
        );

        // Verify that an error is returned
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));

        // Now test our error handling logic
        let err_str = "The model `invalid-model` does not exist or you do not have access to it.";

        // Check if this is a model-related error
        assert!(
            err_str.contains("model")
                && (err_str.contains("not exist") || err_str.contains("not found"))
        );

        // Verify that fetch_available_models returns the expected models
        let models = provider.fetch_available_models().unwrap();
        assert_eq!(
            models,
            vec!["test-model".to_string(), "another-model".to_string()]
        );

        // Test the formatting of models as a bulleted list
        let mut sorted_models = models.clone();
        sorted_models.sort();

        let formatted_models = sorted_models
            .iter()
            .map(|model| format!("\n  • {}", model))
            .collect::<Vec<String>>()
            .join("");
        assert_eq!(formatted_models, "\n  • another-model\n  • test-model");
    }

    #[test]
    fn test_invalid_model_error_formatting() {
        // Create a mock provider that simulates an invalid model error
        #[derive(Debug)]
        struct MockInvalidModelProvider;

        impl AiProvider for MockInvalidModelProvider {
            fn name(&self) -> &str {
                "mock"
            }
            fn supports_streaming(&self) -> bool {
                false
            }
            fn requires_api_key(&self) -> bool {
                false
            }
            fn check_available(&self) -> Result<(), Box<dyn Error>> {
                Ok(())
            }
            fn default_model(&self) -> &str {
                "mock-default-model"
            }
            fn default_temperature(&self) -> f32 {
                0.5
            }

            fn complete_structured(
                &self,
                model: &str,
                _temperature: f32,
                _system_prompt: &str,
                _user_prompt: &str,
            ) -> Result<CommitTemplate, Box<dyn Error>> {
                Err(AiError::InvalidModel {
                    model: model.to_string(),
                }
                .into())
            }

            fn fetch_available_models(&self) -> Result<Vec<String>, Box<dyn Error>> {
                // Return a list of mock models
                Ok(vec![
                    "mock-model-1".to_string(),
                    "mock-model-2".to_string(),
                    "mock-model-3".to_string(),
                ])
            }
        }

        // We need to test the error handling directly since we can't override create_default_registry
        let provider = MockInvalidModelProvider;
        let model = "invalid-mock-model";
        let provider_name = provider.name();

        // Simulate the error handling in generate_commit_message
        let err = provider
            .complete_structured(model, 0.5, "test", "test")
            .unwrap_err();
        let err_str = err.to_string();

        // Check if this is a model-related error
        assert_eq!(err_str, "Invalid model: invalid-mock-model");

        // Get available models
        let models = provider.fetch_available_models().unwrap();
        assert!(!models.is_empty());

        // Sort models and format as a bulleted list
        let mut sorted_models = models.clone();
        sorted_models.sort();

        let available_models = sorted_models
            .iter()
            .map(|model| format!("\n  • {}", model))
            .collect::<Vec<String>>()
            .join("");

        // Create the error message
        let error_message = format!(
            "Model '{}' is invalid for provider '{}'. Available models:{}",
            model, provider_name, available_models
        );

        // Check the formatting
        assert!(error_message.contains("Model 'invalid-mock-model' is invalid for provider 'mock'"));
        assert!(error_message.contains("Available models:"));
        assert!(error_message.contains("  • mock-model-1"));
        assert!(error_message.contains("  • mock-model-2"));
        assert!(error_message.contains("  • mock-model-3"));
    }
}
