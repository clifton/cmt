/// Default values for configuration
pub mod defaults {
    // General defaults
    pub const MESSAGE_ONLY: bool = false;
    pub const NO_DIFF_STATS: bool = false;
    pub const SHOW_RAW_DIFF: bool = false;
    pub const CONTEXT_LINES: u32 = 12;
    pub const MAX_LINES_PER_FILE: usize = 500;
    pub const MAX_LINE_WIDTH: usize = 300;

    // AI provider defaults
    pub const DEFAULT_PROVIDER: &str = "claude";

    // Git defaults
    pub const INCLUDE_RECENT_COMMITS: bool = true;
    pub const RECENT_COMMITS_COUNT: usize = 5;

    // File paths
    pub const DEFAULT_CONFIG_FILENAME: &str = ".cmt.toml";
    pub const GLOBAL_CONFIG_DIRNAME: &str = ".config/cmt";
    pub const GLOBAL_CONFIG_FILENAME: &str = "config.toml";

    // Template defaults
    pub const DEFAULT_TEMPLATE: &str = "conventional";

    // Available providers
    pub const AVAILABLE_PROVIDERS: &[&str] = &["claude", "openai"];

    pub const DEFAULT_CLAUDE_MODEL: &str = "claude-3-7-sonnet-latest";
    pub const DEFAULT_OPENAI_MODEL: &str = "gpt-4o";

    // Available templates
    pub const AVAILABLE_TEMPLATES: &[&str] = &["conventional", "simple", "detailed"];
}

/// Example configuration for initialization
pub fn example_config() -> String {
    format!(
        r#"# cmt configuration file

# General options
message_only = {}
no_diff_stats = {}
show_raw_diff = {}
context_lines = {}
max_lines_per_file = {}
max_line_width = {}

# AI provider options
provider = "{}"  # Options: {}
# model = "{}"  # Uncomment to set a specific model
# temperature = 0.3  # Uncomment to set a specific temperature

# Git options
include_recent_commits = {}
recent_commits_count = {}

# Template options
# template = "{}"  # Uncomment to use a specific template

# You can add a default hint that will be used for all commits
# hint = "Focus on the technical details"
"#,
        defaults::MESSAGE_ONLY,
        defaults::NO_DIFF_STATS,
        defaults::SHOW_RAW_DIFF,
        defaults::CONTEXT_LINES,
        defaults::MAX_LINES_PER_FILE,
        defaults::MAX_LINE_WIDTH,
        defaults::DEFAULT_PROVIDER,
        defaults::AVAILABLE_PROVIDERS.join(", "),
        defaults::DEFAULT_CLAUDE_MODEL,
        defaults::INCLUDE_RECENT_COMMITS,
        defaults::RECENT_COMMITS_COUNT,
        defaults::DEFAULT_TEMPLATE,
    )
}

/// Simple template
pub fn simple_template() -> String {
    r#"{{subject}}

{{details}}"#
        .to_string()
}

/// Conventional commits template
pub fn conventional_template() -> String {
    r#"{{type}}: {{subject}}

{{#if details}}
{{details}}
{{/if}}"#
        .to_string()
}

/// Detailed template
pub fn detailed_template() -> String {
    r#"{{type}}: {{subject}}

{{#if details}}
{{details}}
{{/if}}

{{#if issues}}
Fixes: {{issues}}
{{/if}}

{{#if breaking}}
BREAKING CHANGE: {{breaking}}
{{/if}}"#
        .to_string()
}
