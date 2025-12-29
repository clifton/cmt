use clap::Parser;

/// A CLI tool that generates commit messages using AI
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Only output the generated commit message, without formatting
    #[arg(short, long)]
    pub message_only: bool,

    /// Hide the diff statistics for staged changes
    #[arg(long, default_value_t = false)]
    pub no_diff_stats: bool,

    /// Show the raw git diff that will be sent to the AI model
    #[arg(long, default_value_t = false)]
    pub show_raw_diff: bool,

    /// Number of context lines to show in the git diff
    #[arg(long, default_value_t = 20)]
    pub context_lines: u32,

    /// Use a specific AI model (defaults to gemini-3-flash-preview, claude-sonnet-4-5-20250929, or gpt-5.2 depending on provider)
    #[arg(long)]
    pub model: Option<String>,

    /// List available models for the selected provider
    #[arg(long)]
    pub list_models: bool,

    /// Adjust the creativity of the generated message (0.0 to 2.0)
    #[arg(short, long)]
    pub temperature: Option<f32>,

    /// Add a hint to guide the AI in generating the commit message
    #[arg(long)]
    pub hint: Option<String>,

    /// Number of maximum lines to show per file in the git diff
    #[arg(long, default_value_t = 2000)]
    pub max_lines_per_file: usize,

    /// Maximum line width for diffs
    #[arg(long, default_value_t = 500)]
    pub max_line_width: usize,

    /// Use a specific template for the commit message
    #[arg(long)]
    pub template: Option<String>,

    /// List all available templates
    #[arg(long)]
    pub list_templates: bool,

    /// Create a new template
    #[arg(long)]
    pub create_template: Option<String>,

    /// Content for the new template (used with --create-template)
    #[arg(long)]
    pub template_content: Option<String>,

    /// Show the content of a specific template
    #[arg(long)]
    pub show_template: Option<String>,

    /// Disable including recent commits for context
    #[arg(long)]
    pub no_recent_commits: bool,

    /// Number of recent commits to include for context
    #[arg(long, default_value_t = 10)]
    pub recent_commits_count: usize,

    /// Create a new configuration file
    #[arg(long)]
    pub init_config: bool,

    /// Path to save the configuration file (defaults to .cmt.toml in current directory)
    #[arg(long)]
    pub config_path: Option<String>,

    /// Use a specific provider (gemini, claude, openai)
    #[arg(long, default_value = "gemini")]
    pub provider: String,

    /// Copy the generated commit message to clipboard
    #[arg(short, long)]
    pub copy: bool,

    /// Skip commit prompt (just show the message)
    #[arg(long)]
    pub no_commit: bool,

    /// Skip confirmation when committing
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Reasoning depth for AI models (none=fastest, minimal, low, high)
    #[arg(long, default_value = "low", value_parser = ["none", "minimal", "low", "high"])]
    pub thinking: String,
}

impl Args {
    pub fn new_from(args: impl Iterator<Item = String>) -> Self {
        Self::parse_from(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_args() {
        let args = Args::new_from(["cmt"].iter().map(ToString::to_string));
        assert!(!args.message_only);
        assert!(!args.no_diff_stats);
        assert!(!args.show_raw_diff);
        assert_eq!(args.context_lines, 20);
        assert!(args.model.is_none());
        assert!(args.temperature.is_none());
        assert!(args.hint.is_none());
        assert!(!args.no_recent_commits);
        assert_eq!(args.recent_commits_count, 10);
        assert!(!args.init_config);
        assert!(args.config_path.is_none());
        assert_eq!(args.provider, "gemini");
        assert!(!args.list_templates);
        assert!(!args.list_models);
        assert!(args.create_template.is_none());
        assert!(args.template_content.is_none());
        assert!(args.show_template.is_none());
        assert!(!args.copy);
    }

    #[test]
    fn test_copy_flag() {
        let args = Args::new_from(["cmt", "--copy"].iter().map(ToString::to_string));
        assert!(args.copy);

        let args = Args::new_from(["cmt", "-c"].iter().map(ToString::to_string));
        assert!(args.copy);
    }

    #[test]
    fn test_no_commit_flag() {
        let args = Args::new_from(["cmt", "--no-commit"].iter().map(ToString::to_string));
        assert!(args.no_commit);

        // Default is to prompt for commit (no_commit = false)
        let args = Args::new_from(["cmt"].iter().map(ToString::to_string));
        assert!(!args.no_commit);
    }

    #[test]
    fn test_yes_flag() {
        let args = Args::new_from(["cmt", "--yes"].iter().map(ToString::to_string));
        assert!(args.yes);

        let args = Args::new_from(["cmt", "-y"].iter().map(ToString::to_string));
        assert!(args.yes);
    }

    #[test]
    fn test_message_only_flag() {
        let args = Args::new_from(["cmt", "--message-only"].iter().map(ToString::to_string));
        assert!(args.message_only);

        let args = Args::new_from(["cmt", "-m"].iter().map(ToString::to_string));
        assert!(args.message_only);
    }

    #[test]
    fn test_provider_option() {
        // Explicit provider should be used
        let args = Args::new_from(
            ["cmt", "--provider", "openai"]
                .iter()
                .map(ToString::to_string),
        );
        assert_eq!(args.provider, "openai");

        // Default should be gemini
        let args = Args::new_from(["cmt"].iter().map(ToString::to_string));
        assert_eq!(args.provider, "gemini");
    }

    #[test]
    fn test_no_diff_stats_flag() {
        let args = Args::new_from(["cmt", "--no-diff-stats"].iter().map(ToString::to_string));
        assert!(args.no_diff_stats);
    }

    #[test]
    fn test_model_option() {
        let model = "gpt-5.2";
        let args = Args::new_from(["cmt", "--model", model].iter().map(ToString::to_string));
        assert_eq!(args.model, Some(model.to_string()));
    }

    #[test]
    fn test_temperature_option() {
        let temp = 0.7;
        let args = Args::new_from(
            ["cmt", "--temperature", &temp.to_string()]
                .iter()
                .map(ToString::to_string),
        );
        assert_eq!(args.temperature, Some(temp));

        let args = Args::new_from(
            ["cmt", "-t", &temp.to_string()]
                .iter()
                .map(ToString::to_string),
        );
        assert_eq!(args.temperature, Some(temp));
    }

    #[test]
    fn test_hint_option() {
        let hint = "Fix the bug in the login flow";
        let args = Args::new_from(["cmt", "--hint", hint].iter().map(ToString::to_string));
        assert_eq!(args.hint, Some(hint.to_string()));
    }

    #[test]
    fn test_combined_flags() {
        let args = Args::new_from(
            [
                "cmt",
                "--message-only",
                "--no-diff-stats",
                "--provider",
                "openai",
                "--model",
                "gpt-5.2",
                "--temperature",
                "0.8",
                "--hint",
                "Fix the login bug",
            ]
            .iter()
            .map(ToString::to_string),
        );

        assert!(args.message_only);
        assert!(args.no_diff_stats);
        assert_eq!(args.model, Some("gpt-5.2".to_string()));
        assert_eq!(args.temperature, Some(0.8));
        assert_eq!(args.hint, Some("Fix the login bug".to_string()));
    }

    #[test]
    fn test_invalid_temperature() {
        let result = Args::try_parse_from(["cmt", "--temperature", "invalid"]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid float literal"));
    }

    #[test]
    fn test_show_raw_diff_flag() {
        let args = Args::new_from(["cmt", "--show-raw-diff"].iter().map(ToString::to_string));
        assert!(args.show_raw_diff);
    }

    #[test]
    fn test_context_lines_option() {
        let args = Args::new_from(
            ["cmt", "--context-lines", "10"]
                .iter()
                .map(ToString::to_string),
        );
        assert_eq!(args.context_lines, 10);
    }

    #[test]
    fn test_list_templates_flag() {
        let args = Args::new_from(["cmt", "--list-templates"].iter().map(ToString::to_string));
        assert!(args.list_templates);
    }

    #[test]
    fn test_create_template_option() {
        let template_name = "custom-template";
        let template_content = "{{type}}: {{subject}}\n\n{{details}}";
        let args = Args::new_from(
            [
                "cmt",
                "--create-template",
                template_name,
                "--template-content",
                template_content,
            ]
            .iter()
            .map(ToString::to_string),
        );
        assert_eq!(args.create_template, Some(template_name.to_string()));
        assert_eq!(args.template_content, Some(template_content.to_string()));
    }

    #[test]
    fn test_show_template_option() {
        let template_name = "conventional";
        let args = Args::new_from(
            ["cmt", "--show-template", template_name]
                .iter()
                .map(ToString::to_string),
        );
        assert_eq!(args.show_template, Some(template_name.to_string()));
    }

    #[test]
    fn test_list_models_flag() {
        let args = Args::new_from(["cmt", "--list-models"].iter().map(ToString::to_string));
        assert!(args.list_models);
    }
}
