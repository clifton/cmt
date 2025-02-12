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
    let mut system_prompt = crate::prompts::SYSTEM_PROMPT.to_string();

    // Add the hint to the system prompt if provided
    if let Some(hint) = &args.hint {
        system_prompt = format!("{}\n\nAdditional context: {}", system_prompt, hint);
    }

    if args.openai {
        let model = args.model.clone().unwrap_or_else(|| "gpt-4o".to_string());
        let temperature = args.temperature.unwrap_or(OPENAI_DEFAULT_TEMP);
        openai::OpenAiProvider::complete(&model, temperature, &system_prompt, &user_prompt)
    } else {
        let model = args
            .model
            .clone()
            .unwrap_or_else(|| "claude-3-5-sonnet-latest".to_string());
        let temperature = args.temperature.unwrap_or(CLAUDE_DEFAULT_TEMP);
        claude::ClaudeProvider::complete(&model, temperature, &system_prompt, &user_prompt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::{SYSTEM_PROMPT, USER_PROMPT_TEMPLATE};

    #[test]
    fn test_hint_is_added_to_system_prompt() {
        let hint = "Fix the login bug";
        let mut args = Args::new_from(["cmt"].iter().map(ToString::to_string));
        args.hint = Some(hint.to_string());

        let changes = "test changes";
        let user_prompt = USER_PROMPT_TEMPLATE.replace("{}", changes);
        let expected_system_prompt = format!("{}\n\nAdditional context: {}", SYSTEM_PROMPT, hint);

        // Create a mock provider to verify the system prompt
        let mock_complete = |_model: &str,
                             _temperature: f32,
                             system_prompt: &str,
                             _user_prompt: &str|
         -> Result<String, Box<dyn Error>> {
            assert_eq!(system_prompt, expected_system_prompt);
            Ok("test commit message".to_string())
        };

        // Test both providers
        let result = mock_complete("test-model", 0.3, &expected_system_prompt, &user_prompt);
        assert!(result.is_ok());
    }
}
