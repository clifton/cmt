use crate::ai::http::{handle_request_error, parse_json_response};
use crate::ai::{parse_commit_template_json, AiError, AiProvider};
use crate::templates::CommitTemplate;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{env, error::Error};

#[derive(Debug)]
pub struct ClaudeProvider;

impl Default for ClaudeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeProvider {
    pub fn new() -> Self {
        Self {}
    }

    fn api_base_url() -> String {
        env::var("ANTHROPIC_API_BASE").unwrap_or_else(|_| "https://api.anthropic.com".to_string())
    }

    fn get_api_key() -> Result<String, AiError> {
        env::var("ANTHROPIC_API_KEY").map_err(|_| AiError::ProviderNotAvailable {
            provider_name: "claude".to_string(),
            message: "ANTHROPIC_API_KEY environment variable not set".to_string(),
        })
    }
}

impl AiProvider for ClaudeProvider {
    fn name(&self) -> &str {
        "claude"
    }

    fn supports_streaming(&self) -> bool {
        false // We'll implement streaming in the future
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    fn complete_structured(
        &self,
        model: &str,
        temperature: f32,
        system_prompt: &str,
        user_prompt: &str,
        thinking_level: Option<crate::ai::ThinkingLevel>,
    ) -> Result<CommitTemplate, Box<dyn Error>> {
        let api_key = Self::get_api_key()?;
        let client = Client::new();

        // Get the schema from the trait method
        let schema = self.get_commit_template_schema();

        // Convert the schema to a pretty-printed string for the system prompt
        let schema_str = serde_json::to_string_pretty(&schema).unwrap_or_default();

        // Create a system prompt that instructs the model to return JSON
        let json_system_prompt = format!(
            "{}\n\nYou MUST respond with a valid JSON object that matches this schema:\n\
            {}\n\
            Do not include any explanations or text outside of the JSON object.",
            system_prompt, schema_str
        );

        // Claude Sonnet 4.5 thinking: disabled by default for speed
        // Minimum budget is 1024 tokens if enabled
        // IMPORTANT: temperature MUST be 1 when thinking is enabled
        let thinking = thinking_level.unwrap_or_default();
        let is_thinking_model = model.contains("sonnet-4") || model.contains("opus-4");
        let use_thinking = is_thinking_model && thinking.claude_thinking_enabled();

        // Claude requires temperature=1 when thinking is enabled
        let effective_temp = if use_thinking { 1.0 } else { temperature };

        let mut request_body = json!({
            "model": model,
            "max_tokens": crate::ai::DEFAULT_MAX_TOKENS,
            "temperature": effective_temp,
            "system": json_system_prompt,
            "messages": [{
                "role": "user",
                "content": user_prompt
            }]
        });

        // Add thinking config for Claude 4.x models if enabled
        if use_thinking {
            request_body["thinking"] = json!({
                "type": "enabled",
                "budget_tokens": 1024  // Minimum allowed
            });
        }

        let response = client
            .post(format!("{}/v1/messages", Self::api_base_url()))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .map_err(handle_request_error)?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());

            // Check if this is a model-related error
            if error_text.contains("model")
                && (status.as_u16() == 404 || error_text.contains("not found"))
            {
                return Err(Box::new(AiError::InvalidModel {
                    model: model.to_string(),
                }));
            }

            // Provide clearer error messages for common HTTP errors
            let error_msg = match status.as_u16() {
                520..=524 => {
                    format!(
                        "Cloudflare/API gateway error (status {}): {}. This is usually transient - please try again.",
                        status.as_u16(),
                        error_text
                    )
                }
                429 => {
                    format!(
                        "Rate limit exceeded (status {}): {}. Please wait a moment and try again.",
                        status.as_u16(),
                        error_text
                    )
                }
                503 => {
                    format!(
                        "Service unavailable (status {}): {}. The API may be temporarily down - please try again.",
                        status.as_u16(),
                        error_text
                    )
                }
                _ => format!("API error (status {}): {}", status.as_u16(), error_text),
            };

            return Err(Box::new(AiError::ApiError {
                code: status.as_u16(),
                message: error_msg,
            }));
        }

        let json: Value = parse_json_response(response)?;

        // When thinking is enabled, response contains multiple content blocks.
        // We need to find the "text" type block (not "thinking" blocks).
        let text_content = json
            .get("content")
            .and_then(|content| content.as_array())
            .and_then(|content_array| {
                // Find the text block (skip thinking blocks)
                content_array.iter().find_map(|block| {
                    let block_type = block.get("type").and_then(|t| t.as_str());
                    if block_type == Some("text") {
                        block.get("text").and_then(|t| t.as_str())
                    } else {
                        None
                    }
                })
            });

        if let Some(content) = text_content {
            // Extract the JSON object from the response
            let content = content.trim();

            // Parse the JSON response into CommitTemplate
            let template_data = parse_commit_template_json(content)?;

            Ok(template_data)
        } else {
            Err(Box::new(AiError::ApiError {
                code: 500,
                message: "Failed to extract content from response".to_string(),
            }))
        }
    }

    fn default_model(&self) -> &str {
        crate::config::defaults::DEFAULT_CLAUDE_MODEL
    }

    fn default_temperature(&self) -> f32 {
        crate::ai::DEFAULT_TEMPERATURE
    }

    fn check_available(&self) -> Result<(), Box<dyn Error>> {
        Self::get_api_key()?;
        Ok(())
    }

    fn fetch_available_models(&self) -> Result<Vec<String>, Box<dyn Error>> {
        // Use the Anthropic API to fetch available models
        let api_key = Self::get_api_key()?;
        let client = Client::new();

        let response = client
            .get(format!("{}/v1/models", Self::api_base_url()))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .send()
            .map_err(handle_request_error)?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Box::new(AiError::ApiError {
                code: status.as_u16(),
                message: format!("Failed to fetch models: {}", error_text),
            }));
        }

        let json: Value = parse_json_response(response)?;

        // Extract model IDs from the response
        let models = json
            .get("data")
            .and_then(|data| data.as_array())
            .map(|models_array| {
                models_array
                    .iter()
                    .filter_map(|model| model.get("id").and_then(|id| id.as_str()))
                    .map(|id| id.to_string())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        // If we couldn't get any models, return a fallback list
        if models.is_empty() {
            return Err(Box::new(AiError::ApiError {
                code: 404,
                message: "No models found in Anthropic API".to_string(),
            }));
        }

        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serial_test::serial;

    fn setup() -> mockito::ServerGuard {
        let server = Server::new();
        env::set_var("ANTHROPIC_API_KEY", "test-api-key");
        env::set_var("ANTHROPIC_API_BASE", server.url());
        server
    }

    #[test]
    #[serial]
    fn test_successful_commit_message_generation() {
        let mut server = setup();
        let mock = server.mock("POST", "/v1/messages")
            .match_header("x-api-key", "test-api-key")
            .match_header("anthropic-version", "2023-06-01")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_body(r#"{
                "content": [{
                    "type": "text",
                    "text": "{\"type\": \"feat\", \"subject\": \"add new feature\", \"details\": \"- Implement cool functionality\\n- Update tests\", \"issues\": null, \"breaking\": null, \"scope\": null}"
                }]
            }"#)
            .create();

        let provider = ClaudeProvider::new();
        let result = provider.complete_structured(
            "claude-sonnet-4-5-20250929",
            0.3,
            "test system prompt",
            "test user prompt",
            None,
        );
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(message.r#type, crate::templates::CommitType::Feat);
        assert_eq!(message.subject, "add new feature");
        assert_eq!(
            message.details,
            Some("- Implement cool functionality\n- Update tests".to_string())
        );

        mock.assert();
    }

    #[test]
    #[serial]
    fn test_api_error_handling() {
        let mut server = setup();
        let mock = server
            .mock("POST", "/v1/messages")
            .match_header("x-api-key", "test-api-key")
            .with_status(400)
            .with_body(
                r#"{
                "error": {
                    "type": "invalid_request_error",
                    "message": "Invalid request parameters"
                }
            }"#,
            )
            .create();

        let provider = ClaudeProvider::new();
        let result = provider.complete_structured(
            "claude-sonnet-4-5-20250929",
            0.3,
            "test system prompt",
            "test user prompt",
            None,
        );
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();

        // The error should contain either the exact message or indicate an API error
        assert!(
            error.contains("Invalid request parameters")
                || error.contains("invalid_request_error")
                || error.contains("API error")
        );

        mock.assert();
    }

    #[test]
    #[serial]
    fn test_custom_model_and_temperature() {
        let mut server = setup();
        let mock = server
            .mock("POST", "/v1/messages")
            .match_header("x-api-key", "test-api-key")
            .match_header("anthropic-version", "2023-06-01")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_body(
                r#"{
                "content": [{
                    "type": "text",
                    "text": "{\"type\": \"test\", \"subject\": \"test commit message\", \"details\": null, \"issues\": null, \"breaking\": null, \"scope\": null}"
                }]
            }"#,
            )
            .create();

        let provider = ClaudeProvider::new();
        let result = provider.complete_structured(
            "custom-model",
            0.8,
            "test system prompt",
            "test user prompt",
            None,
        );
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(message.r#type, crate::templates::CommitType::Test);
        assert_eq!(message.subject, "test commit message");

        mock.assert();
    }

    #[test]
    #[serial]
    fn test_fetch_available_models() {
        let mut server = setup();

        // Mock the models endpoint
        let models_mock = server
            .mock("GET", "/v1/models")
            .match_header("x-api-key", "test-api-key")
            .match_header("anthropic-version", "2023-06-01")
            .with_status(200)
            .with_body(
                r#"{
                "data": [
                    {"id": "claude-sonnet-4-5-20250929", "object": "model"},
                    {"id": "claude-opus-4-20250514", "object": "model"}
                ]
            }"#,
            )
            .create();

        let provider = ClaudeProvider::new();
        let models = provider.fetch_available_models().unwrap();

        // Verify we got the expected models from the API
        assert!(models.len() == 2);
        assert!(models.contains(&"claude-sonnet-4-5-20250929".to_string()));
        assert!(models.contains(&"claude-opus-4-20250514".to_string()));

        models_mock.assert();
    }

    #[test]
    #[serial]
    fn test_fetch_available_models_fallback() {
        let mut server = setup();

        // Mock the models endpoint to return an error
        let models_mock = server
            .mock("GET", "/v1/models")
            .match_header("x-api-key", "test-api-key")
            .match_header("anthropic-version", "2023-06-01")
            .with_status(500)
            .with_body(
                r#"{
                "error": {
                    "message": "Internal server error"
                }
            }"#,
            )
            .create();

        let provider = ClaudeProvider::new();

        // The method should return an error
        let result = provider.fetch_available_models();
        assert!(result.is_err());

        // Verify the error message
        let error = result.unwrap_err().to_string();
        assert!(error.contains("500"));

        models_mock.assert();
    }

    #[test]
    #[serial]
    fn test_invalid_model_error() {
        let mut server = setup();
        let mock = server
            .mock("POST", "/v1/messages")
            .match_header("x-api-key", "test-api-key")
            .with_status(404)
            .with_body(
                r#"{
                "type": "error",
                "error": {
                    "type": "not_found_error",
                    "message": "model: invalid-model-name"
                }
            }"#,
            )
            .create();

        // Mock the models endpoint
        let models_mock = server
            .mock("GET", "/v1/models")
            .match_header("x-api-key", "test-api-key")
            .match_header("anthropic-version", "2023-06-01")
            .with_status(200)
            .with_body(
                r#"{
                "data": [
                    {"id": "claude-sonnet-4-5-20250929", "object": "model"},
                    {"id": "claude-opus-4-20250514", "object": "model"}
                ]
            }"#,
            )
            .create();

        let provider = ClaudeProvider::new();
        let result = provider.complete_structured(
            "invalid-model-name",
            0.3,
            "test system prompt",
            "test user prompt",
            None,
        );

        // Verify that an error is returned
        assert!(result.is_err());

        // Check that the error is an InvalidModel error
        let error = result.unwrap_err();
        let error_string = error.to_string();
        assert!(error_string.contains("invalid-model-name"));

        // Downcast the error to check if it's an InvalidModel error
        let is_invalid_model = error
            .downcast_ref::<AiError>()
            .map(|e| matches!(e, AiError::InvalidModel { .. }))
            .unwrap_or(false);
        assert!(is_invalid_model, "Expected InvalidModel error");

        // Test that fetch_available_models returns the expected models
        let models = provider.fetch_available_models().unwrap();
        assert!(!models.is_empty());
        assert!(models.contains(&"claude-sonnet-4-5-20250929".to_string()));
        assert!(models.contains(&"claude-opus-4-20250514".to_string()));

        mock.assert();
        models_mock.assert();
    }
}
