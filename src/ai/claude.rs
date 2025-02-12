use crate::ai::{AiProvider, CLAUDE_DEFAULT_TEMP};
use crate::cli::Args;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{env, error::Error};

pub struct ClaudeProvider;

impl ClaudeProvider {
    fn api_base_url() -> String {
        env::var("ANTHROPIC_API_BASE").unwrap_or_else(|_| "https://api.anthropic.com".to_string())
    }
}

impl AiProvider for ClaudeProvider {
    fn generate_commit_message(changes: &str, args: &Args) -> Result<String, Box<dyn Error>> {
        let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");
        let client = Client::new();

        let model = args
            .model
            .clone()
            .unwrap_or_else(|| "claude-3-5-sonnet-latest".to_string());

        let user_prompt = crate::prompts::USER_PROMPT_TEMPLATE.replace("{}", changes);

        let response = client
            .post(format!("{}/v1/messages", Self::api_base_url()))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&json!({
                "model": model,
                "max_tokens": 1024,
                "temperature": args.temperature.unwrap_or(CLAUDE_DEFAULT_TEMP),
                "system": crate::prompts::SYSTEM_PROMPT,
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
                    format!("Request error: {}", e)
                }
            })?;

        if !response.status().is_success() {
            let error_text = response.text()?;
            return Err(format!("API returned error: {}", error_text).into());
        }

        let response_json: Value = response
            .json()
            .map_err(|e| format!("Failed to parse API response: {}", e))?;
        let message = response_json["content"][0]["text"]
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

        let args = Args::new_from(["cmt"].iter().map(ToString::to_string));
        let changes = "Some test changes";

        let result = ClaudeProvider::generate_commit_message(changes, &args);
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

        let args = Args::new_from(["cmt"].iter().map(ToString::to_string));
        let changes = "Some test changes";

        let result = ClaudeProvider::generate_commit_message(changes, &args);
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

        let args = Args::new_from(
            ["cmt", "--model", "custom-model", "--temperature", "0.8"]
                .iter()
                .map(ToString::to_string),
        );
        let changes = "Some test changes";

        let result = ClaudeProvider::generate_commit_message(changes, &args);
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(message, "test commit message");

        mock.assert();
    }
}
