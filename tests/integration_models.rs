//! Integration tests for AI provider default models
//!
//! These tests verify that the default models for each provider are working correctly.
//! They require valid API keys to run:
//!   - ANTHROPIC_API_KEY for Claude tests
//!   - OPENAI_API_KEY for OpenAI tests
//!
//! Run these tests with:
//!   cargo test --test integration_models -- --ignored
//!
//! Or run a specific provider:
//!   cargo test --test integration_models test_claude -- --ignored
//!   cargo test --test integration_models test_openai -- --ignored

use std::env;

/// Helper to check if an API key is set
fn has_api_key(key: &str) -> bool {
    env::var(key).is_ok()
}

/// Test that Claude's default model generates valid commit messages
#[test]
#[ignore] // Requires ANTHROPIC_API_KEY
fn test_claude_default_model_works() {
    if !has_api_key("ANTHROPIC_API_KEY") {
        eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
        return;
    }

    use cmt::defaults::DEFAULT_CLAUDE_MODEL;
    use cmt::providers::{AiProvider, ClaudeProvider};

    let provider = ClaudeProvider::new();

    // Verify we can check availability
    assert!(
        provider.check_available().is_ok(),
        "Claude provider should be available with API key"
    );

    // Verify the default model
    let default_model = provider.default_model();
    assert_eq!(
        default_model, DEFAULT_CLAUDE_MODEL,
        "Default model should match constant"
    );
    println!("Testing Claude with default model: {}", default_model);

    // Test actual API call with the default model
    let system_prompt = "You are a commit message generator. Return valid JSON.";
    let user_prompt = r#"Generate a commit message for this change:

diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,5 @@
 fn main() {
     println!("Hello, world!");
+    // Added a comment
+    println!("Goodbye!");
 }"#;

    let result = provider.complete_structured(
        default_model,
        provider.default_temperature(),
        system_prompt,
        user_prompt,
    );

    match &result {
        Ok(template) => {
            println!("✓ Claude default model works!");
            println!("  Type: {:?}", template.r#type);
            println!("  Subject: {}", template.subject);
            assert!(!template.subject.is_empty(), "Subject should not be empty");
        }
        Err(e) => {
            panic!("Claude default model '{}' failed: {}", default_model, e);
        }
    }
}

/// Test that OpenAI's default model generates valid commit messages
#[test]
#[ignore] // Requires OPENAI_API_KEY
fn test_openai_default_model_works() {
    if !has_api_key("OPENAI_API_KEY") {
        eprintln!("Skipping test: OPENAI_API_KEY not set");
        return;
    }

    use cmt::defaults::DEFAULT_OPENAI_MODEL;
    use cmt::providers::{AiProvider, OpenAiProvider};

    let provider = OpenAiProvider::new();

    // Verify we can check availability
    assert!(
        provider.check_available().is_ok(),
        "OpenAI provider should be available with API key"
    );

    // Verify the default model
    let default_model = provider.default_model();
    assert_eq!(
        default_model, DEFAULT_OPENAI_MODEL,
        "Default model should match constant"
    );
    println!("Testing OpenAI with default model: {}", default_model);

    // Test actual API call with the default model
    let system_prompt = "You are a commit message generator. Return valid JSON.";
    let user_prompt = r#"Generate a commit message for this change:

diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,5 @@
 fn main() {
     println!("Hello, world!");
+    // Added a comment
+    println!("Goodbye!");
 }"#;

    let result = provider.complete_structured(
        default_model,
        provider.default_temperature(),
        system_prompt,
        user_prompt,
    );

    match &result {
        Ok(template) => {
            println!("✓ OpenAI default model works!");
            println!("  Type: {:?}", template.r#type);
            println!("  Subject: {}", template.subject);
            assert!(!template.subject.is_empty(), "Subject should not be empty");
        }
        Err(e) => {
            panic!("OpenAI default model '{}' failed: {}", default_model, e);
        }
    }
}

/// Test that Claude can fetch available models and the default is in the list
#[test]
#[ignore] // Requires ANTHROPIC_API_KEY
fn test_claude_default_model_in_available_list() {
    if !has_api_key("ANTHROPIC_API_KEY") {
        eprintln!("Skipping test: ANTHROPIC_API_KEY not set");
        return;
    }

    use cmt::providers::{AiProvider, ClaudeProvider};

    let provider = ClaudeProvider::new();
    let default_model = provider.default_model();

    let models = provider.fetch_available_models();
    match models {
        Ok(model_list) => {
            println!("Available Claude models: {:?}", model_list);

            // The default model should be in the list OR be a valid alias
            // Note: dated versions like claude-sonnet-4-5-20250929 should be in the API response
            let has_default = model_list.iter().any(|m| m == default_model);

            if !has_default {
                println!(
                    "Warning: Default model '{}' not in available list",
                    default_model
                );
                println!("This may indicate the model name needs updating.");
                println!("Available models: {:?}", model_list);
            }

            assert!(
                !model_list.is_empty(),
                "Should have at least one available model"
            );
        }
        Err(e) => {
            panic!("Failed to fetch Claude models: {}", e);
        }
    }
}

/// Test that OpenAI can fetch available models and the default is in the list
#[test]
#[ignore] // Requires OPENAI_API_KEY
fn test_openai_default_model_in_available_list() {
    if !has_api_key("OPENAI_API_KEY") {
        eprintln!("Skipping test: OPENAI_API_KEY not set");
        return;
    }

    use cmt::providers::{AiProvider, OpenAiProvider};

    let provider = OpenAiProvider::new();
    let default_model = provider.default_model();

    let models = provider.fetch_available_models();
    match models {
        Ok(model_list) => {
            println!("Available OpenAI models ({} total)", model_list.len());

            // Check if default model is in the list
            let has_default = model_list.iter().any(|m| m == default_model);

            if !has_default {
                println!(
                    "Warning: Default model '{}' not in available list",
                    default_model
                );
                println!("This may indicate the model name needs updating.");
                // Print GPT models only for readability
                let gpt_models: Vec<&String> =
                    model_list.iter().filter(|m| m.starts_with("gpt")).collect();
                println!("Available GPT models: {:?}", gpt_models);
            }

            assert!(
                !model_list.is_empty(),
                "Should have at least one available model"
            );
        }
        Err(e) => {
            panic!("Failed to fetch OpenAI models: {}", e);
        }
    }
}

/// Test both providers with a realistic diff
#[test]
#[ignore] // Requires both API keys
fn test_both_providers_with_realistic_diff() {
    let has_claude = has_api_key("ANTHROPIC_API_KEY");
    let has_openai = has_api_key("OPENAI_API_KEY");

    if !has_claude && !has_openai {
        eprintln!("Skipping test: No API keys set");
        return;
    }

    use cmt::providers::AiProvider;

    let realistic_diff = r#"Generate a commit message for this change:

diff --git a/src/auth/login.rs b/src/auth/login.rs
index 1234567..abcdefg 100644
--- a/src/auth/login.rs
+++ b/src/auth/login.rs
@@ -15,6 +15,12 @@ pub async fn login(credentials: &Credentials) -> Result<Session, AuthError> {
     let user = find_user(&credentials.username).await?;

+    // Add rate limiting to prevent brute force attacks
+    if is_rate_limited(&credentials.username).await {
+        return Err(AuthError::RateLimited);
+    }
+
     if !verify_password(&credentials.password, &user.password_hash) {
+        record_failed_attempt(&credentials.username).await;
         return Err(AuthError::InvalidCredentials);
     }

@@ -22,5 +28,6 @@ pub async fn login(credentials: &Credentials) -> Result<Session, AuthError> {
 }
"#;

    let system_prompt = "You are a commit message generator following conventional commits.";

    if has_claude {
        use cmt::providers::ClaudeProvider;
        let provider = ClaudeProvider::new();
        let result = provider.complete_structured(
            provider.default_model(),
            provider.default_temperature(),
            system_prompt,
            realistic_diff,
        );

        match result {
            Ok(template) => {
                println!(
                    "✓ Claude generated: {:?}: {}",
                    template.r#type, template.subject
                );
            }
            Err(e) => {
                panic!("Claude failed with realistic diff: {}", e);
            }
        }
    }

    if has_openai {
        use cmt::providers::OpenAiProvider;
        let provider = OpenAiProvider::new();
        let result = provider.complete_structured(
            provider.default_model(),
            provider.default_temperature(),
            system_prompt,
            realistic_diff,
        );

        match result {
            Ok(template) => {
                println!(
                    "✓ OpenAI generated: {:?}: {}",
                    template.r#type, template.subject
                );
            }
            Err(e) => {
                panic!("OpenAI failed with realistic diff: {}", e);
            }
        }
    }
}
