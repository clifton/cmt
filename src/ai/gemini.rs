use crate::ai::http::{handle_request_error, parse_json_response};
use crate::ai::{
    parse_commit_template_json, AiError, AiProvider, DEFAULT_MAX_TOKENS, DEFAULT_TEMPERATURE,
};
use crate::templates::CommitTemplate;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{env, error::Error};

#[derive(Debug)]
pub struct GeminiProvider;

impl Default for GeminiProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiProvider {
    pub fn new() -> Self {
        Self {}
    }

    fn api_base_url() -> String {
        env::var("GEMINI_API_BASE")
            .unwrap_or_else(|_| "https://generativelanguage.googleapis.com".to_string())
    }

    fn get_api_key() -> Result<String, AiError> {
        env::var("GEMINI_API_KEY")
            .or_else(|_| env::var("GOOGLE_API_KEY"))
            .map_err(|_| AiError::ProviderNotAvailable {
                provider_name: "gemini".to_string(),
                message: "GEMINI_API_KEY or GOOGLE_API_KEY environment variable not set"
                    .to_string(),
            })
    }
}

impl AiProvider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    fn supports_streaming(&self) -> bool {
        false
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
    ) -> Result<CommitTemplate, Box<dyn Error>> {
        let api_key = Self::get_api_key()?;
        let client = Client::new();

        // Get the schema from the trait method
        let schema = self.get_commit_template_schema();

        // Convert the schema to a pretty-printed string for the system prompt
        let schema_str = serde_json::to_string_pretty(&schema).unwrap_or_default();

        // Create a system prompt that instructs the model to return JSON
        let json_system_prompt = format!(
            "{}\n\nYou MUST respond with a valid JSON object that matches this schema:\n\
            {}\n\
            Do not include any explanations or text outside of the JSON object.",
            system_prompt, schema_str
        );

        // Gemini API uses a different endpoint structure
        let url = format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            Self::api_base_url(),
            model,
            api_key
        );

        let response = client
            .post(&url)
            .header("content-type", "application/json")
            .json(&json!({
                "contents": [{
                    "parts": [{
                        "text": format!("{}\n\n{}", json_system_prompt, user_prompt)
                    }]
                }],
                "generationConfig": {
                    "temperature": temperature,
                    "maxOutputTokens": DEFAULT_MAX_TOKENS,
                    "responseMimeType": "application/json"
                }
            }))
            .send()
            .map_err(handle_request_error)?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());

            // Check if this is a model-related error
            if error_text.contains("model")
                && (status.as_u16() == 404
                    || error_text.contains("not found")
                    || error_text.contains("not supported"))
            {
                return Err(Box::new(AiError::InvalidModel {
                    model: model.to_string(),
                }));
            }

            // Provide clearer error messages for common HTTP errors
            let error_msg = match status.as_u16() {
                520..=524 => {
                    format!(
                        "Cloudflare/API gateway error (status {}): {}. This is usually transient - please try again.",
                        status.as_u16(),
                        error_text
                    )
                }
                429 => {
                    format!(
                        "Rate limit exceeded (status {}): {}. Please wait a moment and try again.",
                        status.as_u16(),
                        error_text
                    )
                }
                503 => {
                    format!(
                        "Service unavailable (status {}): {}. The API may be temporarily down - please try again.",
                        status.as_u16(),
                        error_text
                    )
                }
                _ => format!("API error (status {}): {}", status.as_u16(), error_text),
            };

            return Err(Box::new(AiError::ApiError {
                code: status.as_u16(),
                message: error_msg,
            }));
        }

        let json: Value = parse_json_response(response)?;

        // Extract text from Gemini response format
        if let Some(content) = json
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|candidate| candidate.get("content"))
            .and_then(|content| content.get("parts"))
            .and_then(|parts| parts.as_array())
            .and_then(|arr| arr.first())
            .and_then(|part| part.get("text"))
            .and_then(|text| text.as_str())
        {
            let content = content.trim();
            let template_data = parse_commit_template_json(content)?;
            Ok(template_data)
        } else {
            Err(Box::new(AiError::ApiError {
                code: 500,
                message: "Failed to extract content from Gemini response".to_string(),
            }))
        }
    }

    fn default_model(&self) -> &str {
        crate::config::defaults::DEFAULT_GEMINI_MODEL
    }

    fn default_temperature(&self) -> f32 {
        DEFAULT_TEMPERATURE
    }

    fn check_available(&self) -> Result<(), Box<dyn Error>> {
        Self::get_api_key()?;
        Ok(())
    }

    fn fetch_available_models(&self) -> Result<Vec<String>, Box<dyn Error>> {
        let api_key = Self::get_api_key()?;
        let client = Client::new();

        let url = format!("{}/v1beta/models?key={}", Self::api_base_url(), api_key);

        let response = client
            .get(&url)
            .header("content-type", "application/json")
            .send()
            .map_err(handle_request_error)?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Box::new(AiError::ApiError {
                code: status.as_u16(),
                message: format!("Failed to fetch models: {}", error_text),
            }));
        }

        let json: Value = parse_json_response(response)?;

        // Extract model names from the response, filtering for generative models
        let models = json
            .get("models")
            .and_then(|m| m.as_array())
            .map(|models_array| {
                models_array
                    .iter()
                    .filter_map(|model| {
                        model.get("name").and_then(|name| name.as_str()).map(|n| {
                            // Remove "models/" prefix if present
                            n.strip_prefix("models/").unwrap_or(n).to_string()
                        })
                    })
                    // Filter for models that support generateContent
                    .filter(|name| {
                        name.starts_with("gemini")
                            && !name.contains("embedding")
                            && !name.contains("aqa")
                    })
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        if models.is_empty() {
            return Err(Box::new(AiError::ApiError {
                code: 404,
                message: "No Gemini models found".to_string(),
            }));
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
        env::set_var("GEMINI_API_KEY", "test-api-key");
        env::set_var("GEMINI_API_BASE", server.url());
        server
    }

    #[test]
    #[serial]
    fn test_successful_commit_message_generation() {
        let mut server = setup();
        let mock = server
            .mock("POST", "/v1beta/models/gemini-3-flash-preview:generateContent")
            .match_query(mockito::Matcher::UrlEncoded("key".into(), "test-api-key".into()))
            .with_status(200)
            .with_body(
                r#"{
                "candidates": [{
                    "content": {
                        "parts": [{
                            "text": "{\"type\": \"feat\", \"subject\": \"add new feature\", \"details\": \"- Implement cool functionality\", \"issues\": null, \"breaking\": null, \"scope\": null}"
                        }]
                    }
                }]
            }"#,
            )
            .create();

        let provider = GeminiProvider::new();
        let result = provider.complete_structured(
            "gemini-3-flash-preview",
            0.7,
            "test system prompt",
            "test user prompt",
        );

        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(message.r#type, crate::templates::CommitType::Feat);
        assert_eq!(message.subject, "add new feature");

        mock.assert();
    }

    #[test]
    #[serial]
    fn test_api_error_handling() {
        let mut server = setup();
        let mock = server
            .mock(
                "POST",
                "/v1beta/models/gemini-3-flash-preview:generateContent",
            )
            .match_query(mockito::Matcher::UrlEncoded(
                "key".into(),
                "test-api-key".into(),
            ))
            .with_status(400)
            .with_body(r#"{"error": {"message": "Invalid request"}}"#)
            .create();

        let provider = GeminiProvider::new();
        let result = provider.complete_structured(
            "gemini-3-flash-preview",
            0.7,
            "test system prompt",
            "test user prompt",
        );

        assert!(result.is_err());
        mock.assert();
    }

    #[test]
    #[serial]
    fn test_fetch_available_models() {
        let mut server = setup();
        let models_mock = server
            .mock("GET", "/v1beta/models")
            .match_query(mockito::Matcher::UrlEncoded(
                "key".into(),
                "test-api-key".into(),
            ))
            .with_status(200)
            .with_body(
                r#"{
                "models": [
                    {"name": "models/gemini-3-flash-preview"},
                    {"name": "models/gemini-3-pro"},
                    {"name": "models/gemini-2.0-flash"},
                    {"name": "models/text-embedding-004"}
                ]
            }"#,
            )
            .create();

        let provider = GeminiProvider::new();
        let models = provider.fetch_available_models().unwrap();

        assert!(models.contains(&"gemini-3-flash-preview".to_string()));
        assert!(models.contains(&"gemini-3-pro".to_string()));
        assert!(models.contains(&"gemini-2.0-flash".to_string()));
        // Embedding models should be filtered out
        assert!(!models.contains(&"text-embedding-004".to_string()));

        models_mock.assert();
    }

    #[test]
    fn test_provider_name() {
        let provider = GeminiProvider::new();
        assert_eq!(provider.name(), "gemini");
    }

    #[test]
    fn test_default_model() {
        let provider = GeminiProvider::new();
        assert_eq!(
            provider.default_model(),
            crate::config::defaults::DEFAULT_GEMINI_MODEL
        );
    }
}
