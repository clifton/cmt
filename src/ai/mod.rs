pub mod claude;
pub mod openai;

use crate::cli::Args;
use std::error::Error;

pub const CLAUDE_DEFAULT_TEMP: f32 = 0.3;
pub const OPENAI_DEFAULT_TEMP: f32 = 1.0;

pub trait AiProvider {
    fn complete(
        model: &str,
        temperature: f32,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, Box<dyn Error>>;
}

pub fn generate_commit_message(changes: &str, args: &Args) -> Result<String, Box<dyn Error>> {
    if changes.is_empty() {
        return Ok(String::from("No staged changes found"));
    }

    let user_prompt = crate::prompts::USER_PROMPT_TEMPLATE.replace("{}", changes);
    let system_prompt = crate::prompts::SYSTEM_PROMPT;

    if args.openai {
        let model = args.model.clone().unwrap_or_else(|| "gpt-4o".to_string());
        let temperature = args.temperature.unwrap_or(OPENAI_DEFAULT_TEMP);
        openai::OpenAiProvider::complete(&model, temperature, system_prompt, &user_prompt)
    } else {
        let model = args
            .model
            .clone()
            .unwrap_or_else(|| "claude-3-5-sonnet-latest".to_string());
        let temperature = args.temperature.unwrap_or(CLAUDE_DEFAULT_TEMP);
        claude::ClaudeProvider::complete(&model, temperature, system_prompt, &user_prompt)
    }
}
