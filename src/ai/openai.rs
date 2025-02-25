use crate::ai::{AiError, AiProvider};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{env, error::Error};

#[derive(Debug)]
pub struct OpenAiProvider;

impl OpenAiProvider {
    pub fn new() -> Self {
        Self {}
    }

    fn api_base_url() -> String {
        env::var("OPENAI_API_BASE").unwrap_or_else(|_| "https://api.openai.com".to_string())
    }

    fn get_api_key() -> Result<String, AiError> {
        env::var("OPENAI_API_KEY").map_err(|_| {
            AiError::ProviderNotAvailable("OPENAI_API_KEY environment variable not set".to_string())
        })
    }
}

impl AiProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
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
            .post(format!("{}/v1/chat/completions", Self::api_base_url()))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&json!({
                "model": model,
                "messages": [
                    {
                        "role": "system",
                        "content": system_prompt
                    },
                    {
                        "role": "user",
                        "content": user_prompt
                    }
                ],
                "temperature": temperature,
                "max_tokens": 100
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
            return Err(format!("API error (status {}): {}", status, error_text).into());
        }

        let json: Value = response
            .json()
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        if let Some(content) = json
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
        {
            Ok(content.trim().to_string())
        } else {
            Err("Failed to extract content from response".into())
        }
    }

    fn default_model(&self) -> &str {
        crate::config::defaults::defaults::DEFAULT_OPENAI_MODEL
    }

    fn default_temperature(&self) -> f32 {
        crate::ai::OPENAI_DEFAULT_TEMP
    }

    fn is_available(&self) -> bool {
        Self::get_api_key().is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serial_test::serial;

    fn setup() -> mockito::ServerGuard {
        let server = Server::new();
        env::set_var("OPENAI_API_KEY", "test-api-key");
        env::set_var("OPENAI_API_BASE", &server.url());
        server
    }

    #[test]
    #[serial]
    fn test_successful_commit_message_generation() {
        let mut server = setup();
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("Authorization", "Bearer test-api-key")
            .match_header("Content-Type", "application/json")
            .with_status(200)
            .with_body(
                r#"{
                "choices": [
                    {
                        "message": {
                            "content": "feat: add new feature\n\n- Implement cool functionality\n- Update tests"
                        }
                    }
                ]
            }"#,
            )
            .create();

        let provider = OpenAiProvider::new();
        let result = provider.complete("gpt-4o", 1.0, "test system prompt", "test user prompt");
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
            .mock("POST", "/v1/chat/completions")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(400)
            .with_body(
                r#"{
                "error": {
                    "message": "Invalid request parameters",
                    "type": "invalid_request_error"
                }
            }"#,
            )
            .create();

        let provider = OpenAiProvider::new();
        let result = provider.complete("gpt-4o", 1.0, "test system prompt", "test user prompt");
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Invalid request parameters"));

        mock.assert();
    }

    #[test]
    #[serial]
    fn test_custom_model_and_temperature() {
        let mut server = setup();
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("Authorization", "Bearer test-api-key")
            .match_header("Content-Type", "application/json")
            .with_status(200)
            .with_body(
                r#"{
                "choices": [
                    {
                        "message": {
                            "content": "test commit message"
                        }
                    }
                ]
            }"#,
            )
            .create();

        let provider = OpenAiProvider::new();
        let result = provider.complete(
            "custom-model",
            0.5,
            "test system prompt",
            "test user prompt",
        );
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(message, "test commit message");

        mock.assert();
    }
}
