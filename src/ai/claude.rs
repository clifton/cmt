use crate::ai::{AiProvider, CLAUDE_DEFAULT_TEMP};
use crate::cli::Args;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{env, error::Error};

pub struct ClaudeProvider;

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
            .post("https://api.anthropic.com/v1/messages")
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
