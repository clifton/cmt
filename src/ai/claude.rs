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
                if e.is_timeout() {
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
                }
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
                return Err(format!(
                    "The model `{}` does not exist or you do not have access to it.",
                    model
                )
                .into());
            }

            return Err(format!("API error (status {}): {}", status, error_text).into());
        }

        let json: Value = response
            .json()
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        if let Some(content) = json
            .get("content")
            .and_then(|content| content.as_array())
            .and_then(|content_array| content_array.first())
            .and_then(|first_content| first_content.get("text"))
            .and_then(|text| text.as_str())
        {
            Ok(content.trim().to_string())
        } else {
            Err("Failed to extract content from response".into())
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
        // Anthropic doesn't have a public models endpoint yet, so we'll return a curated list
        // This could be updated in the future when Anthropic adds a models endpoint
        Ok(vec![
            "claude-3-5-sonnet-20240620".to_string(),
            "claude-3-opus-20240229".to_string(),
            "claude-3-sonnet-20240229".to_string(),
            "claude-3-haiku-20240307".to_string(),
            "claude-2.1".to_string(),
            "claude-2.0".to_string(),
        ])
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
        assert!(models.contains(&"claude-3-5-sonnet-20240620".to_string()));
        assert!(models.contains(&"claude-3-opus-20240229".to_string()));

        mock.assert();
    }
}
