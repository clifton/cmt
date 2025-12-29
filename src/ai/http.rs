//! Shared HTTP utilities for AI providers.

use super::AiError;
use reqwest::blocking::Response;
use serde_json::Value;
use std::error::Error;

/// Convert a reqwest error into an AiError with helpful messages.
pub fn handle_request_error(e: reqwest::Error) -> Box<dyn Error> {
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
}

/// Parse a JSON response, returning an appropriate error on failure.
pub fn parse_json_response(response: Response) -> Result<Value, Box<dyn Error>> {
    response.json().map_err(|e| {
        Box::new(AiError::JsonError {
            message: format!("Failed to parse JSON: {}", e),
        }) as Box<dyn Error>
    })
}
