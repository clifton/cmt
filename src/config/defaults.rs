//! Default values for configuration

// General defaults
pub const MESSAGE_ONLY: bool = false;
pub const NO_DIFF_STATS: bool = false;
pub const SHOW_RAW_DIFF: bool = false;
pub const CONTEXT_LINES: u32 = 20; // Full function context - Gemini Flash supports 1M tokens
pub const MAX_LINES_PER_FILE: usize = 2000; // Allow large files - we have token budget
pub const MAX_LINE_WIDTH: usize = 500; // Allow wider lines for better context

// AI provider defaults
pub const DEFAULT_PROVIDER: &str = "gemini";

// Git defaults
pub const INCLUDE_RECENT_COMMITS: bool = true;
pub const RECENT_COMMITS_COUNT: usize = 10; // More history for better context

// File paths
pub const DEFAULT_CONFIG_FILENAME: &str = ".cmt.toml";
pub const GLOBAL_CONFIG_DIRNAME: &str = ".config/cmt";
pub const GLOBAL_CONFIG_FILENAME: &str = "config.toml";

// Template defaults
pub const DEFAULT_TEMPLATE: &str = "conventional";

// Available providers
pub const AVAILABLE_PROVIDERS: &[&str] = &["claude", "openai", "gemini"];

// Last Verified: 2025-12-29 (use dated version - Anthropic API doesn't accept -latest aliases)
pub const DEFAULT_CLAUDE_MODEL: &str = "claude-sonnet-4-5-20250929";
// Last Verified: 2025-12-29
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-5.2";
// Last Verified: 2025-12-29 (use -preview suffix for Gemini 3 models)
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-3-flash-preview";

// Available templates
pub const AVAILABLE_TEMPLATES: &[&str] = &["conventional", "simple", "detailed"];

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
        MESSAGE_ONLY,
        NO_DIFF_STATS,
        SHOW_RAW_DIFF,
        CONTEXT_LINES,
        MAX_LINES_PER_FILE,
        MAX_LINE_WIDTH,
        DEFAULT_PROVIDER,
        AVAILABLE_PROVIDERS.join(", "),
        DEFAULT_CLAUDE_MODEL,
        INCLUDE_RECENT_COMMITS,
        RECENT_COMMITS_COUNT,
        DEFAULT_TEMPLATE,
    )
}

/// Simple template
pub fn simple_template() -> String {
    r#"{{{subject}}}

{{{details}}}"#
        .to_string()
}

/// Conventional commits template (triple braces to avoid HTML escaping)
pub fn conventional_template() -> String {
    r#"{{type}}{{#if scope}}({{{scope}}}){{/if}}: {{{subject}}}

{{#if details}}
{{{details}}}
{{/if}}"#
        .to_string()
}

/// Detailed template (triple braces to avoid HTML escaping)
pub fn detailed_template() -> String {
    r#"{{type}}{{#if scope}}({{{scope}}}){{/if}}: {{{subject}}}

{{#if details}}
{{{details}}}
{{/if}}

{{#if issues}}
Fixes: {{{issues}}}
{{/if}}

{{#if breaking}}
BREAKING CHANGE: {{{breaking}}}
{{/if}}"#
        .to_string()
}
