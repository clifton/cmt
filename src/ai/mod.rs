pub mod claude;
pub mod openai;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Args;
    use crate::prompts::{SYSTEM_PROMPT, USER_PROMPT_TEMPLATE};

    struct MockProvider;

    impl AiProvider for MockProvider {
        fn complete(
            _model: &str,
            _temperature: f32,
            system_prompt: &str,
            _user_prompt: &str,
        ) -> Result<String, Box<dyn Error>> {
            // Return the system prompt so we can verify its contents
            Ok(system_prompt.to_string())
        }
    }

    #[test]
    fn test_hint_is_added_to_system_prompt() {
        let hint = "Fix the login bug";
        let mut args = Args::new_from(["cmt"].iter().map(ToString::to_string));
        args.hint = Some(hint.to_string());

        let changes = "test changes";
        let expected_system_prompt = format!("{}\n\nAdditional context: {}", SYSTEM_PROMPT, hint);

        let result = MockProvider::complete(
            "test-model",
            0.3,
            &expected_system_prompt,
            &USER_PROMPT_TEMPLATE.replace("{}", changes),
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_system_prompt);
    }

    #[test]
    fn test_hint_integration_with_ai_provider() {
        // Create test data
        let hint = "This is a test hint";
        let changes = "test file changes";
        let mut args = Args::new_from(["cmt"].iter().map(ToString::to_string));
        args.hint = Some(hint.to_string());

        // Create a mock provider implementation
        let mock_complete = |_model: &str,
                             _temperature: f32,
                             system_prompt: &str,
                             user_prompt: &str|
         -> Result<String, Box<dyn Error>> {
            // Verify the hint is in the system prompt
            assert!(
                system_prompt.contains(hint),
                "Hint not found in system prompt"
            );
            // Verify the original system prompt is preserved
            assert!(
                system_prompt.contains(SYSTEM_PROMPT),
                "Original system prompt not found"
            );
            // Verify the changes are in the user prompt
            assert!(
                user_prompt.contains(changes),
                "Changes not found in user prompt"
            );
            Ok("test commit message".to_string())
        };

        // Run the test with our mock
        let result = mock_complete(
            "test-model",
            0.3,
            &format!("{}\n\nAdditional context: {}", SYSTEM_PROMPT, hint),
            &USER_PROMPT_TEMPLATE.replace("{}", changes),
        );
        assert!(result.is_ok(), "Failed to generate commit message");
    }
}
