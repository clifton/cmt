pub use crate::config::cli::Args;
pub use crate::git::{get_recent_commits, get_staged_changes, git_staged_changes};

mod ai;
mod config;
mod git;
mod prompts;
mod templates;

use templates::TemplateData;

pub fn generate_commit_message(
    args: &Args,
    git_diff: &str,
    recent_commits: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let registry = ai::create_default_registry();
    let template_name = args
        .template
        .clone()
        .unwrap_or_else(|| config::defaults::defaults::DEFAULT_TEMPLATE.to_string());
    let template_manager = templates::TemplateManager::new()?;

    // Get the provider from the registry
    let provider_name = &args.provider;
    let provider = match registry.get(provider_name) {
        Some(p) => p,
        None => {
            return Err(Box::new(ai::AiError::ProviderNotFound(
                provider_name.clone(),
            )))
        }
    };

    // Check if the provider is available (has API key)
    if !provider.is_available() {
        return Err(Box::new(ai::AiError::ProviderNotAvailable(
            provider_name.clone(),
        )));
    }

    // Build the prompt for the AI provider
    let mut prompt = String::new();

    if args.include_recent_commits && !recent_commits.is_empty() {
        prompt.push_str("\n\nRecent commits for context:\n");
        prompt.push_str(recent_commits);
    }

    prompt.push_str(&prompts::user_prompt(git_diff));

    // Build the system prompt
    let mut system_prompt = prompts::system_prompt();
    if let Some(hint) = &args.hint {
        system_prompt = format!("{}\n\nAdditional context: {}", system_prompt, hint);
    }

    // Generate the commit message
    let model = args
        .model
        .clone()
        .unwrap_or_else(|| provider.default_model().to_string());
    let temperature = args
        .temperature
        .unwrap_or_else(|| provider.default_temperature());
    let response = provider.complete(&model, temperature, &system_prompt, &prompt)?;

    // Parse the commit message
    let commit_data = parse_commit_message(&response)?;

    // Render the template
    let rendered = template_manager.render(&template_name, &commit_data)?;

    // Return the rendered message along with provider and model info
    Ok(rendered)
}

/// Parse a commit message into template data
fn parse_commit_message(message: &str) -> Result<TemplateData, Box<dyn std::error::Error>> {
    let mut data = TemplateData::default();

    // Split the message into lines
    let lines: Vec<&str> = message.lines().collect();

    if lines.is_empty() {
        return Err("Empty commit message".into());
    }

    // Find where the AI explanation starts (if any)
    let mut end_idx = lines.len();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Check for various AI explanation markers
        if trimmed == "AI:"
            || trimmed == "Anthropic:"
            || trimmed == "Claude:"
            || trimmed == "OpenAI:"
            || trimmed.starts_with("AI:")
            || trimmed.starts_with("Anthropic:")
            || trimmed.starts_with("Claude:")
            || trimmed.starts_with("OpenAI:")
        {
            end_idx = i;
            break;
        }
    }

    // Parse the first line (type: subject)
    let first_line = lines[0];
    if let Some(colon_pos) = first_line.find(':') {
        data.r#type = first_line[..colon_pos].trim().to_string();

        // Check for scope in type
        if let Some(open_paren) = data.r#type.find('(') {
            if let Some(close_paren) = data.r#type.find(')') {
                if open_paren < close_paren {
                    let scope = data.r#type[open_paren + 1..close_paren].trim().to_string();
                    data.scope = Some(scope);
                    data.r#type = data.r#type[..open_paren].trim().to_string();
                }
            }
        }

        data.subject = first_line[colon_pos + 1..].trim().to_string();
    } else {
        // If there's no colon, use the whole line as the subject
        data.subject = first_line.trim().to_string();
    }

    // Parse the rest of the message for details
    if lines.len() > 1 {
        // Skip empty lines after the first line
        let mut start_idx = 1;
        while start_idx < end_idx && lines[start_idx].trim().is_empty() {
            start_idx += 1;
        }

        if start_idx < end_idx {
            let details = lines[start_idx..end_idx].join("\n");
            if !details.trim().is_empty() {
                data.details = Some(details);
            }
        }
    }

    Ok(data)
}

// Re-export the config module for external use
pub mod config_mod {
    pub use crate::config::file;
    pub use crate::config::{Config, ConfigError};
}

// Re-export the templates module for external use
pub mod template_mod {
    pub use crate::templates::{TemplateData, TemplateError, TemplateManager};
}

// Re-export the ai module for external use
pub mod ai_mod {
    pub use crate::ai::create_default_registry;
    pub use crate::ai::{AiError, AiProvider, ProviderRegistry};
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
        let result = generate_commit_message(&args, "", "");

        // Verify that an error is returned
        assert!(result.is_err());

        // Convert the error to a string and check that it contains the expected message
        let error_string = format!("{}", result.unwrap_err());
        assert!(error_string.contains("Provider not found: unsupported_provider"));
    }

    #[test]
    #[serial_test::serial]
    fn test_provider_not_available() {
        // Temporarily unset the API keys
        let had_anthropic_key = env::var("ANTHROPIC_API_KEY").is_ok();
        let _had_openai_key = env::var("OPENAI_API_KEY").is_ok();

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
        let result = generate_commit_message(&args, "", "");

        // Verify that an error is returned
        assert!(result.is_err());

        // Convert the error to a string and check that it contains the expected message
        let error_string = format!("{}", result.unwrap_err());
        println!("Actual error: {}", error_string);

        // The error message should indicate that the provider is not available
        // due to missing API key
        assert!(error_string.contains("API_KEY") || error_string.contains("not available"));

        // Restore the API keys if they were set
        if had_anthropic_key {
            // We can't restore the actual value, but for testing purposes,
            // we can set a dummy value that will pass the is_available check
            env::set_var("ANTHROPIC_API_KEY", "dummy_key_for_test");
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_provider_and_model_info() {
        // This test requires mocking the AI provider, so we'll use a simple approach
        // by setting up a mock environment

        // Set up environment for testing
        env::set_var("ANTHROPIC_API_KEY", "test_key");

        // Create args with specific provider and model
        let args = Args::new_from(
            ["cmt", "--provider", "claude", "--model", "test-model"]
                .iter()
                .map(ToString::to_string),
        );

        // We can't actually call generate_commit_message because it would try to make
        // a real API call. Instead, we'll verify that the provider and model are correctly
        // extracted from the args.

        // Verify that the provider and model match what we expect
        assert_eq!(args.provider, "claude");
        assert_eq!(args.model, Some("test-model".to_string()));

        // Clean up
        env::remove_var("ANTHROPIC_API_KEY");
    }
}
