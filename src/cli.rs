use clap::Parser;

/// A CLI tool that generates commit messages using AI
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Only output the generated commit message, without formatting
    #[arg(short, long)]
    pub message_only: bool,

    /// Show the diff of staged changes
    #[arg(short, long)]
    pub show_diff: bool,

    /// Use a specific AI model (defaults to claude-3-5-sonnet-latest or gpt-4o depending on provider)
    #[arg(long)]
    pub model: Option<String>,

    /// Use OpenAI instead of Claude (which is default)
    #[arg(long, default_value_t = false)]
    pub openai: bool,

    /// Use Anthropic instead of OpenAI (which is default)
    #[arg(long, default_value_t = true)]
    pub anthropic: bool,

    /// Adjust the creativity of the generated message (0.0 to 2.0)
    #[arg(short, long)]
    pub temperature: Option<f32>,
}
