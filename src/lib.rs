pub mod ai;
pub mod cli;
pub mod git;
pub mod prompts;

pub use ai::{generate_commit_message, AiProvider};
pub use cli::Args;
pub use git::{get_staged_changes, show_git_diff};
