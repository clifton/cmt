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

/// Main configuration struct that combines CLI and file configs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // General options
    pub message_only: bool,
    pub no_diff_stats: bool,
    pub show_raw_diff: bool,
    pub context_lines: u32,
    pub max_lines_per_file: usize,
    pub max_line_width: usize,

    // AI provider options
    pub provider: String,
    pub model: Option<String>,
    pub temperature: Option<f32>,

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
            message_only: defaults::defaults::MESSAGE_ONLY,
            no_diff_stats: defaults::defaults::NO_DIFF_STATS,
            show_raw_diff: defaults::defaults::SHOW_RAW_DIFF,
            context_lines: defaults::defaults::CONTEXT_LINES,
            max_lines_per_file: defaults::defaults::MAX_LINES_PER_FILE,
            max_line_width: defaults::defaults::MAX_LINE_WIDTH,
            provider: defaults::defaults::DEFAULT_PROVIDER.to_string(),
            model: None,
            temperature: None,
            include_recent_commits: defaults::defaults::INCLUDE_RECENT_COMMITS,
            recent_commits_count: defaults::defaults::RECENT_COMMITS_COUNT,
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
        if other.message_only {
            self.message_only = other.message_only;
        }
        if other.no_diff_stats {
            self.no_diff_stats = other.no_diff_stats;
        }
        if other.show_raw_diff {
            self.show_raw_diff = other.show_raw_diff;
        }
        if other.context_lines != 12 {
            self.context_lines = other.context_lines;
        }
        if other.max_lines_per_file != 500 {
            self.max_lines_per_file = other.max_lines_per_file;
        }
        if other.max_line_width != 300 {
            self.max_line_width = other.max_line_width;
        }
        if other.provider != "claude" {
            self.provider = other.provider.clone();
        }
        if other.model.is_some() {
            self.model = other.model.clone();
        }
        if other.temperature.is_some() {
            self.temperature = other.temperature;
        }
        if !other.include_recent_commits {
            self.include_recent_commits = other.include_recent_commits;
        }
        if other.recent_commits_count != 5 {
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
        let mut config = Self::default();

        config.message_only = args.message_only;
        config.no_diff_stats = args.no_diff_stats;
        config.show_raw_diff = args.show_raw_diff;
        config.context_lines = args.context_lines;
        config.max_lines_per_file = args.max_lines_per_file;
        config.max_line_width = args.max_line_width;
        config.provider = args.provider.clone();

        if let Some(model) = &args.model {
            config.model = Some(model.clone());
        }

        if let Some(temperature) = args.temperature {
            config.temperature = Some(temperature);
        }

        if let Some(hint) = &args.hint {
            config.hint = Some(hint.clone());
        }

        config
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
