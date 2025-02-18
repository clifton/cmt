pub use crate::cli::Args;
pub use crate::git::{get_recent_commits, get_staged_changes, git_staged_changes};

mod ai;
mod cli;
mod git;
mod prompts;

use ai::AiProvider;

pub fn generate_commit_message(
    staged_changes: &str,
    recent_commits: &str,
    args: &Args,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut prompt = String::new();

    if !recent_commits.is_empty() {
        prompt.push_str("\n\nRecent commits for context:\n");
        prompt.push_str(recent_commits);
    }

    prompt.push_str(&prompts::user_prompt(staged_changes));

    let mut system_prompt = prompts::system_prompt();
    if let Some(hint) = &args.hint {
        system_prompt = format!("{}\n\nAdditional context: {}", system_prompt, hint);
    }

    if args.openai {
        let model = args.model.clone().unwrap_or_else(|| "gpt-4o".to_string());
        let temperature = args.temperature.unwrap_or(ai::OPENAI_DEFAULT_TEMP);
        ai::openai::OpenAiProvider::complete(&model, temperature, &system_prompt, &prompt)
    } else {
        let model = args
            .model
            .clone()
            .unwrap_or_else(|| "claude-3-5-sonnet-latest".to_string());
        let temperature = args.temperature.unwrap_or(ai::CLAUDE_DEFAULT_TEMP);
        ai::claude::ClaudeProvider::complete(&model, temperature, &system_prompt, &prompt)
    }
}
