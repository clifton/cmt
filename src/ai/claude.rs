use crate::ai::{AiError, AiProvider};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{env, error::Error};

#[derive(Debug)]
pub struct ClaudeProvider;

impl ClaudeProvider {
    pub fn new() -> Self {
        Self {}
    }

    fn api_base_url() -> String {
        env::var("ANTHROPIC_API_BASE").unwrap_or_else(|_| "https://api.anthropic.com".to_string())
    }

    fn get_api_key() -> Result<String, AiError> {
        env::var("ANTHROPIC_API_KEY").map_err(|_| {
            AiError::ProviderNotAvailable(
                "ANTHROPIC_API_KEY environment variable not set".to_string(),
            )
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

    fn complete(
        &self,
        model: &str,
        temperature: f32,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, Box<dyn Error>> {
        let api_key = Self::get_api_key()?;
        let client = Client::new();

        let response = client
            .post(format!("{}/v1/messages", Self::api_base_url()))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&json!({
                "model": model,
                "max_tokens": 1024,
                "temperature": temperature,
                "system": system_prompt,
                "messages": [{
                    "role": "user",
                    "content": user_prompt
                }]
            }))
            .send()
            .map_err(|e| {
                let error_msg = if e.is_timeout() {
                    format!("Request timed out: {}", e)
                } else if e.is_connect() {
                    format!(
                        "Connection error: {}. Please check your internet connection.",
                        e
                    )
                } else if let Some(status) = e.status() {
                    format!("API error (status {}): {}", status, e)
                } else {
                    format!("Unknown error: {}", e)
                };
                Box::new(AiError::ApiError(error_msg))
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());

            // Check if this is a model-related error
            if error_text.contains("model")
                && (status.as_u16() == 404 || error_text.contains("not found"))
            {
                return Err(Box::new(AiError::InvalidModel(format!(
                    "The model `{}` does not exist or you do not have access to it.",
                    model
                ))));
            }

            return Err(Box::new(AiError::ApiError(format!(
                "API error (status {}): {}",
                status, error_text
            ))));
        }

        let json: Value = response
            .json()
            .map_err(|e| Box::new(AiError::ApiError(format!("Failed to parse JSON: {}", e))))?;

        if let Some(content) = json
            .get("content")
            .and_then(|content| content.as_array())
            .and_then(|content_array| content_array.first())
            .and_then(|first_content| first_content.get("text"))
            .and_then(|text| text.as_str())
        {
            Ok(content.trim().to_string())
        } else {
            Err(Box::new(AiError::ApiError(
                "Failed to extract content from response".to_string(),
            )))
        }
    }

    fn default_model(&self) -> &str {
        crate::config::defaults::defaults::DEFAULT_CLAUDE_MODEL
    }

    fn default_temperature(&self) -> f32 {
        crate::ai::CLAUDE_DEFAULT_TEMP
    }

    fn is_available(&self) -> bool {
        Self::get_api_key().is_ok()
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
            .map_err(|e| Box::new(AiError::ApiError(format!("Failed to fetch models: {}", e))))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Box::new(AiError::ApiError(format!(
                "API error (status {}): {}",
                status, error_text
            ))));
        }

        let json: Value = response
            .json()
            .map_err(|e| Box::new(AiError::ApiError(format!("Failed to parse JSON: {}", e))))?;

        // Extract model IDs from the response
        let mut models = json
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
            return Err(Box::new(AiError::ApiError(
                "Failed to fetch available models from the API".to_string(),
            )));
        }

        // models ending in -latest do not show up in the API response
        if !models.contains(&self.default_model().to_string()) {
            models.push(self.default_model().to_string());
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
        env::set_var("ANTHROPIC_API_BASE", &server.url());
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
                    "text": "feat: add new feature\n\n- Implement cool functionality\n- Update tests"
                }]
            }"#)
            .create();

        let provider = ClaudeProvider::new();
        let result = provider.complete(
            "claude-3-7-sonnet-latest",
            0.3,
            "test system prompt",
            "test user prompt",
        );
        assert!(result.is_ok());
        let message = result.unwrap();
        assert!(message.contains("feat: add new feature"));
        assert!(message.contains("Implement cool functionality"));

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
        let result = provider.complete(
            "claude-3-7-sonnet-latest",
            0.3,
            "test system prompt",
            "test user prompt",
        );
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        println!("Actual Claude API error: {}", error);

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
                    "text": "test commit message"
                }]
            }"#,
            )
            .create();

        let provider = ClaudeProvider::new();
        let result = provider.complete(
            "custom-model",
            0.8,
            "test system prompt",
            "test user prompt",
        );
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(message, "test commit message");

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
                    {"id": "claude-3-5-sonnet-20241022", "object": "model"},
                    {"id": "claude-3-opus-20240229", "object": "model"},
                    {"id": "claude-3-sonnet-20240229", "object": "model"},
                    {"id": "claude-3-haiku-20240307", "object": "model"},
                    {"id": "claude-3-5-sonnet-20240620", "object": "model"}
                ]
            }"#,
            )
            .create();

        let provider = ClaudeProvider::new();
        let models = provider.fetch_available_models().unwrap();

        // Verify we got the expected models plus potentially the default model
        // The default model might be added if it's not in the list
        assert!(models.len() == 6);
        assert!(models.contains(&"claude-3-5-sonnet-20241022".to_string()));
        assert!(models.contains(&"claude-3-opus-20240229".to_string()));
        assert!(models.contains(&"claude-3-sonnet-20240229".to_string()));
        assert!(models.contains(&"claude-3-haiku-20240307".to_string()));
        assert!(models.contains(&"claude-3-5-sonnet-20240620".to_string()));
        // The default model should also be in the list
        assert!(models.contains(&provider.default_model().to_string()));

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
                    {"id": "claude-3-5-sonnet-20241022", "object": "model"},
                    {"id": "claude-3-opus-20240229", "object": "model"},
                    {"id": "claude-3-sonnet-20240229", "object": "model"},
                    {"id": "claude-3-haiku-20240307", "object": "model"}
                ]
            }"#,
            )
            .create();

        let provider = ClaudeProvider::new();
        let result = provider.complete(
            "invalid-model-name",
            0.3,
            "test system prompt",
            "test user prompt",
        );

        // Verify that an error is returned
        assert!(result.is_err());

        // Check that the error message contains the expected text
        let error = result.unwrap_err().to_string();
        assert!(error.contains("does not exist"));
        assert!(error.contains("invalid-model-name"));

        // Test that fetch_available_models returns the expected models
        let models = provider.fetch_available_models().unwrap();
        assert!(!models.is_empty());
        assert!(models.contains(&"claude-3-5-sonnet-20241022".to_string()));
        assert!(models.contains(&"claude-3-opus-20240229".to_string()));

        mock.assert();
        models_mock.assert();
    }
}
