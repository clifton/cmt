use crate::ai::{AiProvider, OPENAI_DEFAULT_TEMP};
use crate::cli::Args;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{env, error::Error};

pub struct OpenAiProvider;

impl AiProvider for OpenAiProvider {
    fn generate_commit_message(changes: &str, args: &Args) -> Result<String, Box<dyn Error>> {
        let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
        let client = Client::new();

        let model = args.model.clone().unwrap_or_else(|| "gpt-4o".to_string());
        let user_prompt = crate::prompts::USER_PROMPT_TEMPLATE.replace("{}", changes);

        let response = client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&json!({
                "model": model,
                "messages": [
                    {
                        "role": "system",
                        "content": crate::prompts::SYSTEM_PROMPT
                    },
                    {
                        "role": "user",
                        "content": user_prompt
                    }
                ],
                "temperature": args.temperature.unwrap_or(OPENAI_DEFAULT_TEMP),
                "max_tokens": 100
            }))
            .send()?;

        let response_json: Value = response.json()?;
        let message = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("Failed to generate commit message")
            .trim()
            .to_string();

        Ok(message)
    }
}
