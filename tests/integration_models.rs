//! Integration tests for AI providers via rstructor
//!
//! These tests verify that the AI providers work correctly with rstructor.
//! They require valid API keys to run:
//!   - GEMINI_API_KEY for Gemini tests
//!   - ANTHROPIC_API_KEY for Claude tests
//!   - OPENAI_API_KEY for OpenAI tests
//!
//! Run these tests with:
//!   cargo test --test integration_models
//!
//! Or run a specific provider:
//!   cargo test --test integration_models test_gemini
//!   cargo test --test integration_models test_claude
//!   cargo test --test integration_models test_openai

use std::env;

use cmt::ai_mod::{ThinkingLevel, PROVIDERS};
use cmt::defaults::{DEFAULT_CLAUDE_MODEL, DEFAULT_GEMINI_MODEL, DEFAULT_OPENAI_MODEL};
use rstructor::{AnthropicClient, GeminiClient, Instructor, LLMClient, OpenAIClient};
use serde::{Deserialize, Serialize};

/// Simple test struct for validation
#[derive(Debug, Serialize, Deserialize, Instructor)]
#[llm(description = "A simple test response")]
struct TestResponse {
    #[llm(description = "A greeting message")]
    message: String,
}

/// Helper to require an API key - panics if not set
fn require_api_key(key: &str) {
    if env::var(key).is_err() {
        panic!("{} environment variable not set", key);
    }
}

/// Test that Claude works with rstructor
#[tokio::test]
async fn test_claude_with_rstructor() {
    require_api_key("ANTHROPIC_API_KEY");

    let client = AnthropicClient::from_env()
        .expect("Failed to build Claude client")
        .model(DEFAULT_CLAUDE_MODEL)
        .temperature(0.3);

    let result = client
        .materialize::<TestResponse>("Say hello in a friendly way")
        .await;

    match result {
        Ok(response) => {
            println!("✓ Claude works with rstructor!");
            println!("  Message: {}", response.message);
            assert!(!response.message.is_empty(), "Message should not be empty");
        }
        Err(e) => {
            panic!(
                "Claude with rstructor failed (model: {}): {}",
                DEFAULT_CLAUDE_MODEL, e
            );
        }
    }
}

/// Test that OpenAI works with rstructor
#[tokio::test]
async fn test_openai_with_rstructor() {
    require_api_key("OPENAI_API_KEY");

    let client = OpenAIClient::from_env()
        .expect("Failed to build OpenAI client")
        .model(DEFAULT_OPENAI_MODEL)
        .temperature(0.3);

    let result = client
        .materialize::<TestResponse>("Say hello in a friendly way")
        .await;

    match result {
        Ok(response) => {
            println!("✓ OpenAI works with rstructor!");
            println!("  Message: {}", response.message);
            assert!(!response.message.is_empty(), "Message should not be empty");
        }
        Err(e) => {
            panic!(
                "OpenAI with rstructor failed (model: {}): {}",
                DEFAULT_OPENAI_MODEL, e
            );
        }
    }
}

/// Test that Gemini works with rstructor
#[tokio::test]
async fn test_gemini_with_rstructor() {
    require_api_key("GEMINI_API_KEY");

    let client = GeminiClient::from_env()
        .expect("Failed to build Gemini client")
        .model(DEFAULT_GEMINI_MODEL)
        .temperature(0.3)
        .thinking_level(rstructor::ThinkingLevel::Low);

    let result = client
        .materialize::<TestResponse>("Say hello in a friendly way")
        .await;

    match result {
        Ok(response) => {
            println!("✓ Gemini works with rstructor!");
            println!("  Message: {}", response.message);
            assert!(!response.message.is_empty(), "Message should not be empty");
        }
        Err(e) => {
            panic!(
                "Gemini with rstructor failed (model: {}): {}",
                DEFAULT_GEMINI_MODEL, e
            );
        }
    }
}

/// Test that the provider list is correct
#[test]
fn test_provider_list() {
    assert!(PROVIDERS.contains(&"claude"));
    assert!(PROVIDERS.contains(&"openai"));
    assert!(PROVIDERS.contains(&"gemini"));
}

/// Test thinking level parsing
#[test]
fn test_thinking_level() {
    assert_eq!(ThinkingLevel::parse("none"), ThinkingLevel::Off);
    assert_eq!(ThinkingLevel::parse("off"), ThinkingLevel::Off);
    assert_eq!(ThinkingLevel::parse("minimal"), ThinkingLevel::Minimal);
    assert_eq!(ThinkingLevel::parse("low"), ThinkingLevel::Low);
    assert_eq!(ThinkingLevel::parse("high"), ThinkingLevel::High);
    // Default for unknown values
    assert_eq!(ThinkingLevel::parse("unknown"), ThinkingLevel::Low);
}
