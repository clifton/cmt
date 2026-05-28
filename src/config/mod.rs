pub mod cli;
pub mod defaults;
pub mod file;

use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Configuration error type
#[derive(Debug)]
pub enum ConfigError {
    IoError(std::io::Error),
    ParseError(String),
    ValidationError(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::IoError(e) => write!(f, "IO error: {}", e),
            ConfigError::ParseError(e) => write!(f, "Parse error: {}", e),
            ConfigError::ValidationError(e) => write!(f, "Validation error: {}", e),
        }
    }
}

impl Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(error: std::io::Error) -> Self {
        ConfigError::IoError(error)
    }
}

/// Main configuration struct that combines CLI and file configs.
///
/// `#[serde(default)]` lets a config file set only the keys it cares about;
/// any omitted field falls back to [`Config::default`] instead of failing to
/// deserialize (which `load()` would otherwise silently swallow).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    // General options
    pub message_only: bool,
    pub no_diff_stats: bool,
    pub show_raw_diff: bool,
    pub redact: bool,
    pub context_lines: u32,
    pub max_lines_per_file: usize,
    pub max_line_width: usize,
    pub max_file_lines: usize,

    // AI provider options
    pub provider: String,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub thinking: String,
    pub timeout_secs: u64,

    // Git options
    pub include_recent_commits: bool,
    pub recent_commits_count: usize,

    // Template options
    pub template: Option<String>,

    // Additional context
    pub hint: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            message_only: defaults::MESSAGE_ONLY,
            no_diff_stats: defaults::NO_DIFF_STATS,
            show_raw_diff: defaults::SHOW_RAW_DIFF,
            redact: defaults::REDACT,
            context_lines: defaults::CONTEXT_LINES,
            max_lines_per_file: defaults::MAX_LINES_PER_FILE,
            max_line_width: defaults::MAX_LINE_WIDTH,
            max_file_lines: defaults::MAX_FILE_LINES,
            provider: defaults::DEFAULT_PROVIDER.to_string(),
            model: None,
            temperature: None,
            thinking: defaults::DEFAULT_THINKING.to_string(),
            timeout_secs: defaults::TIMEOUT_SECS,
            include_recent_commits: defaults::INCLUDE_RECENT_COMMITS,
            recent_commits_count: defaults::RECENT_COMMITS_COUNT,
            template: None,
            hint: None,
        }
    }
}

impl Config {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from a file
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;

        // Parse based on file extension
        if let Some(ext) = path.extension() {
            if ext == "toml" {
                toml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))
            } else if ext == "json" {
                serde_json::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))
            } else {
                Err(ConfigError::ParseError(format!(
                    "Unsupported file format: {:?}",
                    ext
                )))
            }
        } else {
            Err(ConfigError::ParseError("Unknown file format".to_string()))
        }
    }

    /// Save configuration to a file
    pub fn save_to_file(&self, path: &Path) -> Result<(), ConfigError> {
        let content = if let Some(ext) = path.extension() {
            if ext == "toml" {
                toml::to_string_pretty(self).map_err(|e| ConfigError::ParseError(e.to_string()))?
            } else if ext == "json" {
                serde_json::to_string_pretty(self)
                    .map_err(|e| ConfigError::ParseError(e.to_string()))?
            } else {
                return Err(ConfigError::ParseError(format!(
                    "Unsupported file format: {:?}",
                    ext
                )));
            }
        } else {
            return Err(ConfigError::ParseError("Unknown file format".to_string()));
        };

        fs::write(path, content)?;
        Ok(())
    }

    /// Merge with another configuration (other takes precedence)
    pub fn merge(&mut self, other: &Config) {
        // Only override non-default values
        if other.message_only != defaults::MESSAGE_ONLY {
            self.message_only = other.message_only;
        }
        if other.no_diff_stats != defaults::NO_DIFF_STATS {
            self.no_diff_stats = other.no_diff_stats;
        }
        if other.show_raw_diff != defaults::SHOW_RAW_DIFF {
            self.show_raw_diff = other.show_raw_diff;
        }
        if other.redact != defaults::REDACT {
            self.redact = other.redact;
        }
        if other.context_lines != defaults::CONTEXT_LINES {
            self.context_lines = other.context_lines;
        }
        if other.max_lines_per_file != defaults::MAX_LINES_PER_FILE {
            self.max_lines_per_file = other.max_lines_per_file;
        }
        if other.max_line_width != defaults::MAX_LINE_WIDTH {
            self.max_line_width = other.max_line_width;
        }
        if other.max_file_lines != defaults::MAX_FILE_LINES {
            self.max_file_lines = other.max_file_lines;
        }
        if other.provider != defaults::DEFAULT_PROVIDER {
            self.provider = other.provider.clone();
        }
        if other.model.is_some() {
            self.model = other.model.clone();
        }
        if other.temperature.is_some() {
            self.temperature = other.temperature;
        }
        if other.thinking != defaults::DEFAULT_THINKING {
            self.thinking = other.thinking.clone();
        }
        if other.timeout_secs != defaults::TIMEOUT_SECS {
            self.timeout_secs = other.timeout_secs;
        }
        if other.include_recent_commits != defaults::INCLUDE_RECENT_COMMITS {
            self.include_recent_commits = other.include_recent_commits;
        }
        if other.recent_commits_count != defaults::RECENT_COMMITS_COUNT {
            self.recent_commits_count = other.recent_commits_count;
        }
        if other.template.is_some() {
            self.template = other.template.clone();
        }
        if other.hint.is_some() {
            self.hint = other.hint.clone();
        }
    }

    /// Load configuration from CLI args
    pub fn from_args(args: &cli::Args) -> Self {
        Self {
            message_only: args.message_only,
            no_diff_stats: args.no_diff_stats,
            show_raw_diff: args.show_raw_diff,
            redact: !args.no_redact,
            context_lines: args.context_lines,
            max_lines_per_file: args.max_lines_per_file,
            max_line_width: args.max_line_width,
            max_file_lines: args.max_file_lines,
            provider: args.provider.clone(),
            model: args.model.clone(),
            temperature: args.temperature,
            thinking: args.thinking.clone(),
            timeout_secs: args.timeout,
            include_recent_commits: !args.no_recent_commits,
            recent_commits_count: args.recent_commits_count,
            template: args.template.clone(),
            hint: args.hint.clone(),
        }
    }

    /// Load configuration from all sources (global, local, args)
    pub fn load() -> Result<Self, ConfigError> {
        // Start with default config
        let mut config = Self::default();

        // Try to load global config
        if let Some(global_config_path) = Self::global_config_path() {
            if global_config_path.exists() {
                if let Ok(global_config) = Self::from_file(&global_config_path) {
                    config.merge(&global_config);
                }
            }
        }

        // Try to load project config
        if let Some(project_config_path) = Self::find_project_config() {
            if let Ok(project_config) = Self::from_file(&project_config_path) {
                config.merge(&project_config);
            }
        }

        Ok(config)
    }

    /// Get the global config path
    fn global_config_path() -> Option<PathBuf> {
        if let Ok(home) = env::var("HOME") {
            Some(
                PathBuf::from(home)
                    .join(".config")
                    .join("cmt")
                    .join("config.toml"),
            )
        } else {
            None
        }
    }

    /// Find project config by walking up the directory tree
    fn find_project_config() -> Option<PathBuf> {
        let current_dir = env::current_dir().ok()?;
        let mut dir = current_dir.as_path();

        loop {
            let config_path = dir.join(".cmt.toml");
            if config_path.exists() {
                return Some(config_path);
            }

            if let Some(parent) = dir.parent() {
                dir = parent;
            } else {
                break;
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::cli::Args;

    fn args_from(argv: &[&str]) -> cli::Args {
        Args::new_from(argv.iter().map(ToString::to_string))
    }

    #[test]
    fn test_from_args_carries_all_cli_fields() {
        // These fields were previously hardcoded to defaults in from_args,
        // silently discarding the CLI/file values.
        let args = args_from(&[
            "cmt",
            "--template",
            "simple",
            "--recent-commits-count",
            "3",
            "--thinking",
            "high",
            "--no-recent-commits",
        ]);
        let cfg = Config::from_args(&args);
        assert_eq!(cfg.template.as_deref(), Some("simple"));
        assert_eq!(cfg.recent_commits_count, 3);
        assert_eq!(cfg.thinking, "high");
        assert!(!cfg.include_recent_commits);
    }

    #[test]
    fn test_file_config_applies_when_cli_uses_defaults() {
        // The dead-config bug: a .cmt.toml provider/template must survive when
        // the CLI passes no explicit overrides.
        let mut merged = Config::default();
        let file = Config {
            provider: "claude".to_string(),
            template: Some("detailed".to_string()),
            recent_commits_count: 25,
            ..Config::default()
        };
        merged.merge(&file);
        merged.merge(&Config::from_args(&args_from(&["cmt"])));

        assert_eq!(merged.provider, "claude");
        assert_eq!(merged.template.as_deref(), Some("detailed"));
        assert_eq!(merged.recent_commits_count, 25);
    }

    #[test]
    fn test_partial_config_file_deserializes() {
        // A .cmt.toml that sets only one key must parse (filling the rest from
        // defaults), not fail and get silently swallowed by load().
        let cfg: Config =
            toml::from_str("template = \"simple\"\n").expect("partial config must deserialize");
        assert_eq!(cfg.template.as_deref(), Some("simple"));
        assert_eq!(cfg.provider, defaults::DEFAULT_PROVIDER);
        assert_eq!(cfg.thinking, defaults::DEFAULT_THINKING);
    }

    #[test]
    fn test_cli_overrides_file() {
        let mut merged = Config::default();
        merged.merge(&Config {
            provider: "claude".to_string(),
            ..Config::default()
        });
        merged.merge(&Config::from_args(&args_from(&[
            "cmt",
            "--provider",
            "openai",
        ])));
        assert_eq!(merged.provider, "openai");
    }
}
