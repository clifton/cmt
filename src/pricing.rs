//! Pricing module for estimating API costs
//!
//! Fetches and caches model pricing from LiteLLM's pricing database.

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};

const PRICING_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
const CACHE_MAX_AGE_SECS: u64 = 86400; // 24 hours

/// Model pricing information
/// Note: We use Value for flexible parsing since the JSON has mixed types
#[derive(Debug, Deserialize, Clone)]
pub struct ModelPricing {
    #[serde(default)]
    pub input_cost_per_token: Option<f64>,
    #[serde(default)]
    pub output_cost_per_token: Option<f64>,
}

/// Raw JSON value for flexible parsing
type RawPricingData = HashMap<String, serde_json::Value>;

/// Parse pricing data from raw JSON, filtering out invalid entries
fn parse_pricing_data(raw: RawPricingData) -> HashMap<String, ModelPricing> {
    raw.into_iter()
        .filter_map(|(key, value)| {
            // Skip sample_spec and other non-model entries
            if key.starts_with("sample_") {
                return None;
            }
            // Try to extract pricing fields
            let input_cost = value.get("input_cost_per_token").and_then(|v| v.as_f64());
            let output_cost = value.get("output_cost_per_token").and_then(|v| v.as_f64());

            // Only include if we have pricing data
            if input_cost.is_some() || output_cost.is_some() {
                Some((
                    key,
                    ModelPricing {
                        input_cost_per_token: input_cost,
                        output_cost_per_token: output_cost,
                    },
                ))
            } else {
                None
            }
        })
        .collect()
}

/// Pricing cache that fetches data in the background
pub struct PricingCache {
    receiver: Option<mpsc::Receiver<HashMap<String, ModelPricing>>>,
    data: Option<HashMap<String, ModelPricing>>,
}

impl PricingCache {
    /// Start fetching pricing data in the background
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        // Spawn background thread to fetch/load pricing
        thread::spawn(move || {
            if let Some(pricing) = load_or_fetch_pricing() {
                let _ = tx.send(pricing);
            }
        });

        Self {
            receiver: Some(rx),
            data: None,
        }
    }

    /// Try to get pricing data (non-blocking)
    pub fn try_get(&mut self) -> Option<&HashMap<String, ModelPricing>> {
        // If we already have data, return it
        if self.data.is_some() {
            return self.data.as_ref();
        }

        // Try to receive from background thread (non-blocking)
        if let Some(ref rx) = self.receiver {
            if let Ok(data) = rx.try_recv() {
                self.data = Some(data);
                self.receiver = None; // Done with receiver
                return self.data.as_ref();
            }
        }

        None
    }

    /// Wait for pricing data with timeout
    pub fn wait_get(&mut self, timeout: Duration) -> Option<&HashMap<String, ModelPricing>> {
        if self.data.is_some() {
            return self.data.as_ref();
        }

        if let Some(ref rx) = self.receiver {
            if let Ok(data) = rx.recv_timeout(timeout) {
                self.data = Some(data);
                self.receiver = None;
                return self.data.as_ref();
            }
        }

        None
    }

    /// Get pricing for a specific model
    pub fn get_model_pricing(&mut self, provider: &str, model: &str) -> Option<ModelPricing> {
        let data = self.try_get()?;

        // Try various key formats that LiteLLM uses
        let keys_to_try = generate_model_keys(provider, model);

        for key in keys_to_try {
            if let Some(pricing) = data.get(&key) {
                if pricing.input_cost_per_token.is_some() {
                    return Some(pricing.clone());
                }
            }
        }

        None
    }
}

impl Default for PricingCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate possible keys for looking up a model in the pricing data
fn generate_model_keys(provider: &str, model: &str) -> Vec<String> {
    let mut keys = Vec::new();

    // Provider-prefixed formats
    match provider {
        "gemini" => {
            keys.push(format!("gemini/{}", model));
            keys.push(format!("google/{}", model));
            keys.push(format!("vertex_ai/{}", model));
            // Try without -preview suffix
            if model.ends_with("-preview") {
                let base = model.trim_end_matches("-preview");
                keys.push(format!("gemini/{}", base));
            }
        }
        "claude" => {
            keys.push(format!("claude/{}", model));
            keys.push(format!("anthropic/{}", model));
            keys.push(model.to_string());
        }
        "openai" => {
            keys.push(model.to_string());
            keys.push(format!("openai/{}", model));
        }
        _ => {
            keys.push(model.to_string());
            keys.push(format!("{}/{}", provider, model));
        }
    }

    // Also try exact model name
    keys.push(model.to_string());

    keys
}

/// Get the cache directory path (~/.cache/cmt on all platforms)
fn cache_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|p| p.join(".cache").join("cmt"))
}

/// Get the cache file path
fn cache_file() -> Option<PathBuf> {
    cache_dir().map(|p| p.join("model_pricing.json"))
}

/// Check if cache is still valid
fn is_cache_valid(path: &PathBuf) -> bool {
    if let Ok(metadata) = fs::metadata(path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(age) = SystemTime::now().duration_since(modified) {
                return age.as_secs() < CACHE_MAX_AGE_SECS;
            }
        }
    }
    false
}

/// Load pricing from cache or fetch from network
fn load_or_fetch_pricing() -> Option<HashMap<String, ModelPricing>> {
    let cache_path = cache_file()?;

    // Try loading from cache first
    if is_cache_valid(&cache_path) {
        if let Ok(mut file) = fs::File::open(&cache_path) {
            let mut contents = String::new();
            if file.read_to_string(&mut contents).is_ok() {
                if let Ok(raw) = serde_json::from_str::<RawPricingData>(&contents) {
                    return Some(parse_pricing_data(raw));
                }
            }
        }
    }

    // Fetch from network
    fetch_and_cache_pricing(&cache_path)
}

/// Fetch pricing from network and cache it
fn fetch_and_cache_pricing(cache_path: &PathBuf) -> Option<HashMap<String, ModelPricing>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?;

    let response = match client.get(PRICING_URL).send() {
        Ok(r) => r,
        Err(_e) => {
            #[cfg(test)]
            eprintln!("Fetch failed: {:?}", _e);
            return None;
        }
    };

    if !response.status().is_success() {
        #[cfg(test)]
        eprintln!("Bad status: {}", response.status());
        return None;
    }

    let text = match response.text() {
        Ok(t) => t,
        Err(_e) => {
            #[cfg(test)]
            eprintln!("Text read failed: {:?}", _e);
            return None;
        }
    };

    // Parse as raw JSON first, then extract pricing
    let raw: RawPricingData = match serde_json::from_str(&text) {
        Ok(d) => d,
        Err(_e) => {
            #[cfg(test)]
            eprintln!("JSON parse failed: {:?}", _e);
            return None;
        }
    };
    let data = parse_pricing_data(raw);

    // Cache for next time
    if let Some(dir) = cache_dir() {
        let _ = fs::create_dir_all(&dir);
        if let Ok(mut file) = fs::File::create(cache_path) {
            let _ = file.write_all(text.as_bytes());
        }
    }

    Some(data)
}

/// Calculate estimated cost
pub fn calculate_cost(
    pricing: &ModelPricing,
    input_tokens: u64,
    output_tokens: u64,
) -> Option<f64> {
    let input_cost = pricing.input_cost_per_token? * input_tokens as f64;
    let output_cost = pricing.output_cost_per_token? * output_tokens as f64;
    Some(input_cost + output_cost)
}

/// Format cost for display
pub fn format_cost(cost: f64) -> String {
    if cost < 0.0001 {
        format!("${:.6}", cost)
    } else if cost < 0.01 {
        format!("${:.4}", cost)
    } else {
        format!("${:.2}", cost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_model_keys() {
        let keys = generate_model_keys("gemini", "gemini-3-flash-preview");
        assert!(keys.contains(&"gemini/gemini-3-flash-preview".to_string()));
        assert!(keys.contains(&"gemini/gemini-3-flash".to_string()));

        let keys = generate_model_keys("openai", "gpt-5.2");
        assert!(keys.contains(&"gpt-5.2".to_string()));
        assert!(keys.contains(&"openai/gpt-5.2".to_string()));

        let keys = generate_model_keys("claude", "claude-sonnet-4-5-20250929");
        assert!(keys.contains(&"claude-sonnet-4-5-20250929".to_string()));
        assert!(keys.contains(&"anthropic/claude-sonnet-4-5-20250929".to_string()));
    }

    #[test]
    fn test_format_cost() {
        assert_eq!(format_cost(0.000001), "$0.000001");
        assert_eq!(format_cost(0.001), "$0.0010");
        assert_eq!(format_cost(0.15), "$0.15");
    }

    #[test]
    fn test_calculate_cost() {
        let pricing = ModelPricing {
            input_cost_per_token: Some(0.000001),
            output_cost_per_token: Some(0.000002),
        };

        let cost = calculate_cost(&pricing, 1000, 500);
        assert_eq!(cost, Some(0.002)); // 1000 * 0.000001 + 500 * 0.000002
    }

    #[test]
    fn test_cache_dir() {
        let dir = super::cache_dir();
        assert!(dir.is_some(), "Cache dir should be available");
        println!("Cache dir: {:?}", dir);
    }

    #[test]
    #[ignore] // Network test
    fn test_fetch_pricing() {
        use std::time::Duration;
        let mut cache = PricingCache::new();
        // Wait up to 15s for network
        let data = cache.wait_get(Duration::from_secs(15));
        assert!(data.is_some(), "Should fetch pricing data");

        // Check we can find a known model
        let gemini_pricing = cache.get_model_pricing("gemini", "gemini-2.0-flash");
        println!("Gemini pricing: {:?}", gemini_pricing);
    }
}
