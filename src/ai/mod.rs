pub mod claude;
pub mod openai;

use crate::cli::Args;
use std::error::Error;

pub const CLAUDE_DEFAULT_TEMP: f32 = 0.3;
pub const OPENAI_DEFAULT_TEMP: f32 = 1.0;

pub trait AiProvider {
    fn generate_commit_message(changes: &str, args: &Args) -> Result<String, Box<dyn Error>>;
}

pub fn generate_commit_message(changes: &str, args: &Args) -> Result<String, Box<dyn Error>> {
    if changes.is_empty() {
        return Ok(String::from("No staged changes found"));
    }

    if args.openai {
        openai::OpenAiProvider::generate_commit_message(changes, args)
    } else {
        claude::ClaudeProvider::generate_commit_message(changes, args)
    }
}
