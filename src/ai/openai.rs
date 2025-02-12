use crate::ai::AiProvider;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{env, error::Error};

pub struct OpenAiProvider;

impl OpenAiProvider {
    fn api_base_url() -> String {
        env::var("OPENAI_API_BASE").unwrap_or_else(|_| "https://api.openai.com".to_string())
    }
}

impl AiProvider for OpenAiProvider {
    fn complete(
        model: &str,
        temperature: f32,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, Box<dyn Error>> {
        let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
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
            .send()?;

        if !response.status().is_success() {
            let error_text = response.text()?;
            return Err(format!("API returned error: {}", error_text).into());
        }

        let response_json: Value = response.json()?;
        let message = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("Failed to generate commit message")
            .trim()
            .to_string();

        Ok(message)
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
        let mock = server.mock("POST", "/v1/chat/completions")
            .match_header("Authorization", "Bearer test-api-key")
            .match_header("Content-Type", "application/json")
            .with_status(200)
            .with_body(r#"{
                "choices": [{
                    "message": {
                        "content": "feat: add new feature\n\n- Implement cool functionality\n- Update tests"
                    }
                }]
            }"#)
            .create();

        let result =
            OpenAiProvider::complete("gpt-4", 1.0, "test system prompt", "test user prompt");
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
                    "message": "Invalid request parameters"
                }
            }"#,
            )
            .create();

        let result =
            OpenAiProvider::complete("gpt-4", 1.0, "test system prompt", "test user prompt");
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
                "choices": [{
                    "message": {
                        "content": "test commit message"
                    }
                }]
            }"#,
            )
            .create();

        let result = OpenAiProvider::complete(
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
}
