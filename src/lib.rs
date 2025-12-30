pub use crate::config::cli::Args;
pub use crate::git::{
    get_current_branch, get_readme_excerpt, get_recent_commits, get_staged_changes, DiffStats,
    StagedChanges,
};

mod ai;
mod analysis;
mod config;
mod git;
pub mod pricing;
mod progress;
mod prompts;
mod templates;

pub use pricing::PricingCache;
pub use progress::Spinner;

pub use analysis::{analyze_diff, DiffAnalysis};

use templates::CommitTemplate;

/// Result of commit message generation
#[derive(Debug)]
pub struct GenerateResult {
    /// The rendered commit message
    pub message: String,
    /// Input tokens used (if available from provider)
    pub input_tokens: Option<u64>,
    /// Output tokens used (if available from provider)
    pub output_tokens: Option<u64>,
}

/// Validate and fix commit data to ensure quality output
fn validate_commit_data(mut data: CommitTemplate) -> CommitTemplate {
    // Ensure subject starts with lowercase
    if let Some(first_char) = data.subject.chars().next() {
        if first_char.is_uppercase() {
            data.subject =
                first_char.to_lowercase().to_string() + &data.subject[first_char.len_utf8()..];
        }
    }

    // Remove trailing period from subject
    if data.subject.ends_with('.') {
        data.subject.pop();
    }

    // Validate scope (lowercase, no spaces)
    if let Some(ref mut scope) = data.scope {
        *scope = scope.to_lowercase().replace(' ', "-");
        // Remove scope if it's too generic, empty, or literally "null"
        if scope.is_empty()
            || scope == "general"
            || scope == "misc"
            || scope == "other"
            || scope == "null"
        {
            data.scope = None;
        }
    }

    // Clean up details - remove bullets that duplicate subject
    if let Some(ref mut details) = data.details {
        let subject_lower = data.subject.to_lowercase();
        let lines: Vec<&str> = details
            .lines()
            .filter(|line| {
                let line_lower = line.to_lowercase();
                // Keep line if it's not too similar to subject
                !line_lower.contains(&subject_lower)
                    && !subject_lower.contains(line_lower.trim_start_matches("- "))
            })
            .collect();

        if lines.is_empty() {
            data.details = None;
        } else {
            *details = lines.join("\n");
        }
    }

    data
}

pub fn generate_commit_message(
    args: &Args,
    git_diff: &str,
    recent_commits: &str,
    analysis: Option<&DiffAnalysis>,
    branch_name: Option<&str>,
    readme_excerpt: Option<&str>,
) -> Result<GenerateResult, Box<dyn std::error::Error>> {
    let template_name = args
        .template
        .clone()
        .unwrap_or_else(|| config::defaults::DEFAULT_TEMPLATE.to_string());
    let template_manager = templates::TemplateManager::new()?;

    // Get provider name
    let provider_name = &args.provider;

    // Check if the provider is available (has API key)
    ai::check_available(provider_name)?;

    // Get the model name, defaulting to the provider's default model
    let model = args
        .model
        .clone()
        .unwrap_or_else(|| ai::default_model(provider_name).to_string());

    // Build the prompt for the AI provider
    let mut prompt = String::new();

    // Include README excerpt for project context
    if let Some(readme) = readme_excerpt {
        prompt.push_str("Project README:\n");
        prompt.push_str(readme);
        prompt.push_str("\n\n");
    }

    // Include branch name for context (often contains feature/ticket info)
    if let Some(branch) = branch_name {
        if branch != "main" && branch != "master" && !branch.starts_with("detached@") {
            prompt.push_str(&format!("Branch: {}\n", branch));
        }
    }

    if !args.no_recent_commits && !recent_commits.is_empty() {
        prompt.push_str("\nRecent commits for context:\n");
        prompt.push_str(recent_commits);
    }

    // Generate analysis summary if available
    let analysis_summary = analysis.map(|a| a.summary());
    prompt.push_str(&prompts::user_prompt(git_diff, analysis_summary.as_deref()));

    // Build the system prompt
    let mut system_prompt = prompts::system_prompt();
    if let Some(hint) = &args.hint {
        system_prompt = format!("{}\n\nAdditional context: {}", system_prompt, hint);
    }

    // Generate the commit message
    let temperature = args.temperature.unwrap_or(ai::DEFAULT_TEMPERATURE);

    // Parse thinking level
    let thinking_level = Some(ai::ThinkingLevel::parse(&args.thinking));

    // Try to complete the prompt with structured output
    let completion = match ai::complete_structured(
        provider_name,
        &model,
        temperature,
        &system_prompt,
        &prompt,
        thinking_level,
    ) {
        Ok(result) => result,
        Err(err) => {
            // Check for invalid model error
            if let Some(ai::AiError::InvalidModel { model }) = err.downcast_ref::<ai::AiError>() {
                return Err(format!(
                    "Invalid model: {} for provider: {}\nCheck the provider's documentation for available models.",
                    model,
                    provider_name
                )
                .into());
            }
            return Err(err);
        }
    };

    // Validate and fix the commit data
    let commit_data = validate_commit_data(completion.template);

    // Render the template
    let rendered = template_manager.render(&template_name, &commit_data)?;

    // Extract token usage if available
    let (input_tokens, output_tokens) = match completion.usage {
        Some(usage) => (Some(usage.input_tokens), Some(usage.output_tokens)),
        None => (None, None),
    };

    Ok(GenerateResult {
        message: rendered,
        input_tokens,
        output_tokens,
    })
}

// Re-export the config module for external use
pub mod config_mod {
    pub use crate::config::file;
    pub use crate::config::{Config, ConfigError};
}

// Re-export the templates module for external use
pub mod template_mod {
    pub use crate::templates::{CommitTemplate, TemplateError, TemplateManager};
}

// Re-export AI types for external use
pub mod ai_mod {
    pub use crate::ai::{
        default_model, list_models, AiError, CompletionResult, ThinkingLevel, PROVIDERS,
    };
}

// Re-export config defaults for testing
pub mod defaults {
    pub use crate::config::defaults::*;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::cli::Args;
    use std::env;

    #[test]
    fn test_unsupported_provider() {
        // Create args with an unsupported provider
        let args = Args::new_from(
            ["cmt", "--provider", "unsupported_provider"]
                .iter()
                .map(ToString::to_string),
        );

        // Call generate_commit_message with the unsupported provider
        let result = generate_commit_message(&args, "", "", None, None, None);

        // Verify that an error is returned
        assert!(result.is_err());

        // Convert the error to a string and check that it contains the expected message
        let error_string = format!("{}", result.unwrap_err());
        assert!(
            error_string.contains("Provider not found")
                || error_string.contains("not available")
                || error_string.contains("unsupported_provider"),
            "Expected error about unsupported provider, got: {}",
            error_string
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_provider_not_available() {
        // Temporarily unset the API keys
        let had_anthropic_key = env::var("ANTHROPIC_API_KEY").is_ok();

        if had_anthropic_key {
            env::remove_var("ANTHROPIC_API_KEY");
        }

        // Create args with claude provider
        let args = Args::new_from(
            ["cmt", "--provider", "claude"]
                .iter()
                .map(ToString::to_string),
        );

        // Call generate_commit_message with the claude provider
        let result = generate_commit_message(&args, "", "", None, None, None);

        // Verify that an error is returned
        assert!(result.is_err());

        // Convert the error to a string and check that it contains the expected message
        let error_string = format!("{}", result.unwrap_err());
        assert!(error_string.contains("API_KEY") || error_string.contains("not available"));

        // Restore the API keys if they were set
        if had_anthropic_key {
            env::set_var("ANTHROPIC_API_KEY", "dummy_key_for_test");
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_provider_and_model_info() {
        // Set up environment for testing
        env::set_var("ANTHROPIC_API_KEY", "test_key");

        // Create args with specific provider and model
        let args = Args::new_from(
            ["cmt", "--provider", "claude", "--model", "test-model"]
                .iter()
                .map(ToString::to_string),
        );

        // Verify that the provider and model match what we expect
        assert_eq!(args.provider, "claude");
        assert_eq!(args.model, Some("test-model".to_string()));

        // Clean up
        env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_validate_commit_data() {
        let data = CommitTemplate {
            commit_type: templates::CommitType::Feat,
            subject: "Add new feature.".to_string(),
            details: Some("- Add new feature\n- Update tests".to_string()),
            issues: None,
            breaking: None,
            scope: Some("General".to_string()),
        };

        let validated = validate_commit_data(data);

        // Subject should be lowercase and without trailing period
        assert_eq!(validated.subject, "add new feature");
        // Scope should be None because "General" is too generic
        assert!(validated.scope.is_none());
        // Details that duplicate subject should be removed
        assert!(validated.details.is_some());
    }
}
