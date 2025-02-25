use crate::ai::{AiError, AiProvider};
use crate::templates::TemplateData;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{env, error::Error};

#[derive(Debug)]
pub struct OpenAiProvider;

impl OpenAiProvider {
    pub fn new() -> Self {
        Self {}
    }

    fn api_base_url() -> String {
        env::var("OPENAI_API_BASE").unwrap_or_else(|_| "https://api.openai.com".to_string())
    }

    fn get_api_key() -> Result<String, AiError> {
        env::var("OPENAI_API_KEY").map_err(|_| AiError::ProviderNotAvailable {
            provider_name: "openai".to_string(),
            message: "OPENAI_API_KEY environment variable not set".to_string(),
        })
    }
}

impl AiProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn supports_streaming(&self) -> bool {
        false // We'll implement streaming in the future
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    fn complete_structured(
        &self,
        model: &str,
        temperature: f32,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<TemplateData, Box<dyn Error>> {
        let api_key = Self::get_api_key()?;
        let client = Client::new();

        // Define the function schema for the commit message structure
        let function_schema = json!({
            "name": "generate_commit_message",
            "description": "Generate a structured commit message based on the changes",
            "parameters": {
                "type": "object",
                "properties": {
                    "type": {
                        "type": "string",
                        "description": "The type of change (e.g., feat, fix, docs, style, refactor, test, chore)"
                    },
                    "subject": {
                        "type": "string",
                        "description": "A short description of the change"
                    },
                    "details": {
                        "type": ["string", "null"],
                        "description": "Optional detailed description of the change"
                    },
                    "issues": {
                        "type": ["string", "null"],
                        "description": "Optional references to issues"
                    },
                    "breaking": {
                        "type": ["string", "null"],
                        "description": "Optional description of breaking changes"
                    },
                    "scope": {
                        "type": ["string", "null"],
                        "description": "Optional scope of the change"
                    }
                },
                "required": ["type", "subject"]
            }
        });

        let response = client
            .post(format!("{}/v1/chat/completions", Self::api_base_url()))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&json!({
                "model": model,
                "messages": [
                    {
                        "role": "system",
                        "content": system_prompt
                    },
                    {
                        "role": "user",
                        "content": user_prompt
                    }
                ],
                "temperature": temperature,
                "tools": [
                    {
                        "type": "function",
                        "function": function_schema
                    }
                ],
                "tool_choice": {
                    "type": "function",
                    "function": {
                        "name": "generate_commit_message"
                    }
                }
            }))
            .send()
            .map_err(|e| {
                let error_msg = if e.is_timeout() {
                    format!("Request timed out: {}", e)
                } else if e.is_connect() {
                    format!(
                        "Connection error: {}. Please check your internet connection.",
                        e
                    )
                } else if let Some(status) = e.status() {
                    format!("API error (status {}): {}", status, e)
                } else {
                    format!("Unknown error: {}", e)
                };
                Box::new(AiError::ApiError {
                    code: e.status().map(|s| s.as_u16()).unwrap_or(500),
                    message: error_msg,
                })
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());

            // Check if this is a model-related error
            if error_text.contains("model")
                && (status.as_u16() == 404 || error_text.contains("not found"))
            {
                return Err(Box::new(AiError::InvalidModel {
                    model: model.to_string(),
                }));
            }

            return Err(Box::new(AiError::ApiError {
                code: status.as_u16(),
                message: format!("API error (status {}): {}", status, error_text),
            }));
        }

        let json: Value = response.json().map_err(|e| {
            Box::new(AiError::JsonError {
                message: format!("Failed to parse JSON: {}", e),
            })
        })?;

        // Extract the function call arguments from the response
        let function_args = json
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("tool_calls"))
            .and_then(|tool_calls| tool_calls.as_array())
            .and_then(|tool_calls| tool_calls.first())
            .and_then(|tool_call| tool_call.get("function"))
            .and_then(|function| function.get("arguments"))
            .and_then(|arguments| arguments.as_str())
            .ok_or_else(|| {
                Box::new(AiError::JsonError {
                    message: "Failed to extract function arguments from response".to_string(),
                }) as Box<dyn Error>
            })?;

        // Parse the function arguments into TemplateData
        let template_data: TemplateData = serde_json::from_str(function_args).map_err(|e| {
            Box::new(AiError::JsonError {
                message: format!("Failed to parse function arguments as TemplateData: {}", e),
            })
        })?;

        Ok(template_data)
    }

    fn default_model(&self) -> &str {
        crate::config::defaults::defaults::DEFAULT_OPENAI_MODEL
    }

    fn default_temperature(&self) -> f32 {
        crate::ai::OPENAI_DEFAULT_TEMP
    }

    fn check_available(&self) -> Result<(), Box<dyn Error>> {
        Self::get_api_key()?;
        Ok(())
    }

    fn fetch_available_models(&self) -> Result<Vec<String>, Box<dyn Error>> {
        let api_key = Self::get_api_key()?;
        let client = Client::new();

        let response = client
            .get(format!("{}/v1/models", Self::api_base_url()))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .send()
            .map_err(|e| {
                Box::new(AiError::ApiError {
                    code: e.status().map(|s| s.as_u16()).unwrap_or(500),
                    message: format!("Failed to fetch models: {}", e),
                })
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Box::new(AiError::ApiError {
                code: status.as_u16(),
                message: format!("API error (status {}): {}", status, error_text),
            }));
        }

        let json: Value = response.json().map_err(|e| {
            Box::new(AiError::JsonError {
                message: format!("Failed to parse JSON: {}", e),
            })
        })?;

        let models = json
            .get("data")
            .and_then(|data| data.as_array())
            .map(|models_array| {
                models_array
                    .iter()
                    .filter_map(|model| model.get("id").and_then(|id| id.as_str()))
                    .filter(|id| id.starts_with("gpt-")) // Only include GPT models
                    .map(|id| id.to_string())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        // If we couldn't get any models, return a default list
        if models.is_empty() {
            return Ok(vec![
                "gpt-4o".to_string(),
                "gpt-4".to_string(),
                "gpt-3.5-turbo".to_string(),
            ]);
        }

        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serial_test::serial;

    fn setup() -> mockito::ServerGuard {
        let server = Server::new();
        env::set_var("OPENAI_API_KEY", "test-api-key");
        env::set_var("OPENAI_API_BASE", &server.url());
        server
    }

    #[test]
    #[serial]
    fn test_successful_commit_message_generation() {
        let mut server = setup();
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("Authorization", "Bearer test-api-key")
            .match_header("Content-Type", "application/json")
            .with_status(200)
            .with_body(
                r#"{
                "choices": [
                    {
                        "message": {
                            "tool_calls": [
                                {
                                    "function": {
                                        "name": "generate_commit_message",
                                        "arguments": "{\"type\": \"feat\", \"subject\": \"add new feature\", \"details\": \"- Implement cool functionality\\n- Update tests\", \"issues\": null, \"breaking\": null, \"scope\": null}"
                                    }
                                }
                            ]
                        }
                    }
                ]
            }"#,
            )
            .create();

        let provider = OpenAiProvider::new();
        let result =
            provider.complete_structured("gpt-4o", 1.0, "test system prompt", "test user prompt");
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(message.r#type, "feat");
        assert_eq!(message.subject, "add new feature");
        assert_eq!(
            message.details,
            Some("- Implement cool functionality\n- Update tests".to_string())
        );

        mock.assert();
    }

    #[test]
    #[serial]
    fn test_api_error_handling() {
        let mut server = setup();
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(400)
            .with_body(
                r#"{
                "error": {
                    "message": "Invalid request parameters",
                    "type": "invalid_request_error"
                }
            }"#,
            )
            .create();

        let provider = OpenAiProvider::new();
        let result =
            provider.complete_structured("gpt-4o", 1.0, "test system prompt", "test user prompt");
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Invalid request parameters"));

        mock.assert();
    }

    #[test]
    #[serial]
    fn test_custom_model_and_temperature() {
        let mut server = setup();
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("Authorization", "Bearer test-api-key")
            .match_header("Content-Type", "application/json")
            .with_status(200)
            .with_body(
                r#"{
                "choices": [
                    {
                        "message": {
                            "tool_calls": [
                                {
                                    "function": {
                                        "name": "generate_commit_message",
                                        "arguments": "{\"type\": \"test\", \"subject\": \"test commit message\", \"details\": null, \"issues\": null, \"breaking\": null, \"scope\": null}"
                                    }
                                }
                            ]
                        }
                    }
                ]
            }"#,
            )
            .create();

        let provider = OpenAiProvider::new();
        let result = provider.complete_structured(
            "custom-model",
            0.5,
            "test system prompt",
            "test user prompt",
        );
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(message.r#type, "test");
        assert_eq!(message.subject, "test commit message");

        mock.assert();
    }

    #[test]
    #[serial]
    fn test_invalid_model_error() {
        let mut server = setup();
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(404)
            .with_body(
                r#"{
                "error": {
                    "message": "The model 'invalid-model-name' does not exist",
                    "type": "invalid_request_error",
                    "param": "model",
                    "code": "model_not_found"
                }
            }"#,
            )
            .create();

        // Also mock the models endpoint for fetch_available_models
        let models_mock = server
            .mock("GET", "/v1/models")
            .match_header("Authorization", "Bearer test-api-key")
            .with_status(200)
            .with_body(
                r#"{
                "data": [
                    {"id": "gpt-4o", "object": "model"},
                    {"id": "gpt-4", "object": "model"},
                    {"id": "gpt-3.5-turbo", "object": "model"},
                    {"id": "text-embedding-ada-002", "object": "model"}
                ]
            }"#,
            )
            .create();

        let provider = OpenAiProvider::new();
        let result = provider.complete_structured(
            "invalid-model-name",
            1.0,
            "test system prompt",
            "test user prompt",
        );

        // Verify that an error is returned
        assert!(result.is_err());

        // Check that the error is an InvalidModel error
        let error = result.unwrap_err();
        let error_string = error.to_string();
        assert!(error_string.contains("invalid-model-name"));

        // Downcast the error to check if it's an InvalidModel error
        let is_invalid_model = error
            .downcast_ref::<AiError>()
            .map(|e| matches!(e, AiError::InvalidModel { .. }))
            .unwrap_or(false);
        assert!(is_invalid_model, "Expected InvalidModel error");

        // Test that fetch_available_models returns the expected models
        let models = provider.fetch_available_models().unwrap();
        assert!(!models.is_empty());
        assert!(models.contains(&"gpt-4o".to_string()));
        assert!(models.contains(&"gpt-4".to_string()));
        assert!(models.contains(&"gpt-3.5-turbo".to_string()));
        assert!(!models.contains(&"text-embedding-ada-002".to_string())); // Should be filtered out

        mock.assert();
        models_mock.assert();
    }
}
