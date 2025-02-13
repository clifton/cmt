use clap::Parser;

/// A CLI tool that generates commit messages using AI
#[derive(Parser, Debug)]
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

    /// Use a specific AI model (defaults to claude-3-5-sonnet-latest or gpt-4o depending on provider)
    #[arg(long)]
    pub model: Option<String>,

    /// Use OpenAI instead of Claude (which is default)
    #[arg(long)]
    pub openai: bool,

    /// Use Anthropic instead of OpenAI (which is default)
    #[arg(long, default_value_t = true)]
    pub anthropic: bool,

    /// Adjust the creativity of the generated message (0.0 to 2.0)
    #[arg(short, long)]
    pub temperature: Option<f32>,

    /// Add a hint to guide the AI in generating the commit message
    #[arg(long)]
    pub hint: Option<String>,
}

impl Args {
    pub fn new_from(args: impl Iterator<Item = String>) -> Self {
        let mut parsed = Self::parse_from(args);
        if parsed.openai {
            parsed.anthropic = false;
        }
        parsed
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
        assert!(args.model.is_none());
        assert!(!args.openai);
        assert!(args.anthropic);
        assert!(args.temperature.is_none());
        assert!(args.hint.is_none());
    }

    #[test]
    fn test_message_only_flag() {
        let args = Args::new_from(["cmt", "--message-only"].iter().map(ToString::to_string));
        assert!(args.message_only);

        let args = Args::new_from(["cmt", "-m"].iter().map(ToString::to_string));
        assert!(args.message_only);
    }

    #[test]
    fn test_no_diff_stats_flag() {
        let args = Args::new_from(["cmt", "--no-diff-stats"].iter().map(ToString::to_string));
        assert!(args.no_diff_stats);
    }

    #[test]
    fn test_model_option() {
        let model = "gpt-4";
        let args = Args::new_from(["cmt", "--model", model].iter().map(ToString::to_string));
        assert_eq!(args.model, Some(model.to_string()));
    }

    #[test]
    fn test_provider_flags() {
        // Default is Anthropic
        let args = Args::new_from(["cmt"].iter().map(ToString::to_string));
        assert!(args.anthropic);
        assert!(!args.openai);

        // Switch to OpenAI
        let args = Args::new_from(["cmt", "--openai"].iter().map(ToString::to_string));
        assert!(!args.anthropic);
        assert!(args.openai);
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
                "--model",
                "gpt-4",
                "--openai",
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
        assert_eq!(args.model, Some("gpt-4".to_string()));
        assert!(args.openai);
        assert!(!args.anthropic);
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
}
