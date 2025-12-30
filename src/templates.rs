use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use handlebars::Handlebars;
use rstructor::Instructor;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config;

/// Template error type
#[derive(Debug)]
pub enum TemplateError {
    IoError(std::io::Error),
    RenderError(String),
    NotFound(String),
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemplateError::IoError(e) => write!(f, "IO error: {}", e),
            TemplateError::RenderError(e) => write!(f, "Render error: {}", e),
            TemplateError::NotFound(e) => write!(f, "Template not found: {}", e),
        }
    }
}

impl Error for TemplateError {}

impl From<std::io::Error> for TemplateError {
    fn from(error: std::io::Error) -> Self {
        TemplateError::IoError(error)
    }
}

impl From<handlebars::RenderError> for TemplateError {
    fn from(error: handlebars::RenderError) -> Self {
        TemplateError::RenderError(error.to_string())
    }
}

// Enum for commit types
// Priority order (highest to lowest): fix > feat > perf > refactor > test > build > ci > chore > style > docs
// Note: Using serde rename + alias because rstructor schema shows PascalCase variants
// but we need lowercase for output. The alias accepts both forms from LLM.
#[derive(Debug, Serialize, Deserialize, PartialEq, Instructor)]
#[llm(
    description = "The type of a commit message. Choose based on the PRIMARY purpose using priority: fix > feat > perf > refactor > test > build > ci > chore > style > docs. If a commit fixes a bug AND updates docs, use 'fix'."
)]
pub enum CommitType {
    #[serde(rename = "fix", alias = "Fix")]
    #[llm(
        description = "PRIORITY 1: Bug fix or error correction. Use if ANY bug is fixed, even with other changes."
    )]
    Fix,
    #[serde(rename = "feat", alias = "Feat")]
    #[llm(
        description = "PRIORITY 2: New feature or enhancement to functionality (not docs/readme)."
    )]
    Feat,
    #[serde(rename = "perf", alias = "Perf")]
    #[llm(
        description = "PRIORITY 3: Performance improvements. Use when the primary goal is optimization."
    )]
    Perf,
    #[serde(rename = "refactor", alias = "Refactor")]
    #[llm(
        description = "PRIORITY 4: Code restructuring WITHOUT behavior change. Only use if no bugs fixed and no features added."
    )]
    Refactor,
    #[serde(rename = "test", alias = "Test")]
    #[llm(
        description = "PRIORITY 5: Test additions or updates. Use when changes are primarily about test coverage."
    )]
    Test,
    #[serde(rename = "build", alias = "Build")]
    #[llm(
        description = "PRIORITY 6: Build system or external dependency changes. E.g., Dockerfile, Makefile."
    )]
    Build,
    #[serde(rename = "ci", alias = "Ci")]
    #[llm(description = "PRIORITY 7: CI/CD configuration changes. E.g., GitHub Actions, Jenkins.")]
    Ci,
    #[serde(rename = "chore", alias = "Chore")]
    #[llm(description = "PRIORITY 8: Maintenance tasks, internal dependency updates, tooling.")]
    Chore,
    #[serde(rename = "style", alias = "Style")]
    #[llm(description = "PRIORITY 9: Formatting or stylistic changes ONLY. No logic changes.")]
    Style,
    #[serde(rename = "docs", alias = "Docs")]
    #[llm(
        description = "PRIORITY 10 (LOWEST): Documentation ONLY. Use ONLY when there are NO code logic changes."
    )]
    Docs,
}

// Struct for commit template with JSON-friendly fields
// Note: Using commit_type field name because rstructor doesn't yet support #[serde(rename)] on fields
// The alias accepts "commit_type" from LLM while rename serializes to "type" for output
#[derive(Debug, Serialize, Deserialize, PartialEq, Instructor)]
#[llm(
    description = "Commit message data. Format: '{commit_type}: {subject}'. Keep first line under 50 chars. Do NOT use scope."
)]
pub struct CommitTemplate {
    #[serde(rename = "type", alias = "commit_type")]
    #[llm(
        description = "The type of the commit message. Select from CommitType based on the change."
    )]
    pub commit_type: CommitType,

    #[llm(
        description = "Brief subject line, ideally under 50 chars total with type prefix. Start with lowercase verb (add, fix, update). Be specific.",
        example = "add user login endpoint"
    )]
    pub subject: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[llm(
        description = "Optional details as bullet points (max 79 chars each). Start each bullet with '- ' followed by present tense verb. Focus on explaining the change's purpose and impact.",
        example = "- Add JWT auth for security\n- Update tests for coverage"
    )]
    pub details: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[llm(
        description = "Optional issue/ticket references. Format: '#123' or 'Fixes #456'",
        example = "#123"
    )]
    pub issues: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[llm(
        description = "Optional breaking change description. Include when your change breaks backward compatibility.",
        example = "Drop support for old API"
    )]
    pub breaking: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[llm(
        description = "LEAVE NULL. Only set for monorepos with packages/apps directories. Do not use for single projects.",
        example = "auth"
    )]
    pub scope: Option<String>,
}

impl Default for CommitTemplate {
    fn default() -> Self {
        Self {
            commit_type: CommitType::Feat,
            subject: "".to_string(),
            details: None,
            issues: None,
            breaking: None,
            scope: None,
        }
    }
}

/// Template manager for handling commit message templates
pub struct TemplateManager {
    handlebars: Handlebars<'static>,
    templates: HashMap<String, String>,
}

impl TemplateManager {
    /// Create a new template manager
    pub fn new() -> Result<Self, TemplateError> {
        let mut manager = Self {
            handlebars: Handlebars::new(),
            templates: HashMap::new(),
        };

        // Load built-in templates
        for &template_name in config::defaults::AVAILABLE_TEMPLATES {
            let template_content = match template_name {
                "simple" => config::defaults::simple_template(),
                "conventional" => config::defaults::conventional_template(),
                "detailed" => config::defaults::detailed_template(),
                _ => continue,
            };
            manager.register_template(template_name, &template_content)?;
        }

        // Load custom templates from template directory
        if let Some(template_dir) = config::file::template_dir() {
            if template_dir.exists() {
                manager.load_from_dir(&template_dir)?;
            }
        }

        Ok(manager)
    }

    /// Register a template with the manager
    pub fn register_template(&mut self, name: &str, content: &str) -> Result<(), TemplateError> {
        self.handlebars
            .register_template_string(name, content)
            .map_err(|e| TemplateError::RenderError(e.to_string()))?;

        self.templates.insert(name.to_string(), content.to_string());
        Ok(())
    }

    /// Load templates from a directory
    pub fn load_from_dir(&mut self, dir: &Path) -> Result<(), TemplateError> {
        if !dir.exists() || !dir.is_dir() {
            return Err(TemplateError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Directory not found: {:?}", dir),
            )));
        }

        let entries = fs::read_dir(dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "hbs" {
                        if let Some(name) = path.file_stem() {
                            if let Some(name_str) = name.to_str() {
                                let content = fs::read_to_string(&path)?;
                                self.register_template(name_str, &content)?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Render a template with the given data
    pub fn render(
        &self,
        template_name: &str,
        data: &CommitTemplate,
    ) -> Result<String, TemplateError> {
        if !self.handlebars.has_template(template_name) {
            return Err(TemplateError::NotFound(format!(
                "Template '{}' not found",
                template_name
            )));
        }

        let rendered = self.handlebars.render(template_name, &json!(data))?;
        Ok(rendered)
    }

    /// Get a list of available templates
    pub fn list_templates(&self) -> Vec<String> {
        self.templates.keys().cloned().collect()
    }

    /// Get the content of a template
    pub fn get_template(&self, name: &str) -> Option<&str> {
        self.templates.get(name).map(|s| s.as_str())
    }

    /// Save a template to the template directory
    pub fn save_template(&mut self, name: &str, content: &str) -> Result<(), TemplateError> {
        // Register the template
        self.register_template(name, content)?;

        // Save to file
        config::file::save_template(name, content)
            .map_err(|e| TemplateError::IoError(std::io::Error::other(e.to_string())))?;

        Ok(())
    }

    /// Delete a template
    pub fn delete_template(&mut self, name: &str) -> Result<(), TemplateError> {
        // Check if template exists
        if !self.templates.contains_key(name) {
            return Err(TemplateError::NotFound(format!(
                "Template '{}' not found",
                name
            )));
        }

        // Remove from handlebars
        self.handlebars.unregister_template(name);

        // Remove from templates map
        self.templates.remove(name);

        // Remove from file system
        if let Some(template_dir) = config::file::template_dir() {
            let template_path = template_dir.join(format!("{}.hbs", name));
            if template_path.exists() {
                fs::remove_file(template_path)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_rendering() {
        let mut manager = TemplateManager {
            handlebars: Handlebars::new(),
            templates: HashMap::new(),
        };

        let template = "{{type}}: {{subject}}\n\n{{#if details}}{{details}}{{/if}}";
        manager.register_template("test", template).unwrap();

        let data = CommitTemplate {
            commit_type: CommitType::Feat,
            subject: "add new feature".to_string(),
            details: Some("- Implement cool functionality\n- Update tests".to_string()),
            ..Default::default()
        };

        let rendered = manager.render("test", &data).unwrap();
        assert_eq!(
            rendered,
            "feat: add new feature\n\n- Implement cool functionality\n- Update tests"
        );
    }

    #[test]
    fn test_conditional_rendering() {
        let mut manager = TemplateManager {
            handlebars: Handlebars::new(),
            templates: HashMap::new(),
        };

        let template = "{{type}}: {{subject}}{{#if scope}} ({{scope}}){{/if}}\n\n{{#if details}}{{details}}{{/if}}";
        manager.register_template("test", template).unwrap();

        // With scope
        let data_with_scope = CommitTemplate {
            commit_type: CommitType::Feat,
            subject: "add new feature".to_string(),
            scope: Some("ui".to_string()),
            ..Default::default()
        };

        let rendered = manager.render("test", &data_with_scope).unwrap();
        assert_eq!(rendered, "feat: add new feature (ui)\n\n");

        // Without scope
        let data_without_scope = CommitTemplate {
            commit_type: CommitType::Feat,
            subject: "add new feature".to_string(),
            ..Default::default()
        };

        let rendered = manager.render("test", &data_without_scope).unwrap();
        assert_eq!(rendered, "feat: add new feature\n\n");
    }

    #[test]
    fn test_commit_template_serialization() {
        // Create a valid CommitTemplate instance
        let template = CommitTemplate {
            commit_type: CommitType::Feat,
            subject: "add schema validation test".to_string(),
            details: Some("- Test schema validation\n- Ensure examples work".to_string()),
            issues: Some("#123".to_string()),
            breaking: None,
            scope: Some("schema".to_string()),
        };

        // Serialize the template to JSON
        let template_json = serde_json::to_value(&template).unwrap();

        // Check type field (serde renames commit_type to "type")
        assert_eq!(
            template_json.get("type").and_then(|v| v.as_str()),
            Some("feat")
        );

        // Check subject field
        assert_eq!(
            template_json.get("subject").and_then(|v| v.as_str()),
            Some("add schema validation test")
        );

        // Check details field
        assert_eq!(
            template_json.get("details").and_then(|v| v.as_str()),
            Some("- Test schema validation\n- Ensure examples work")
        );

        // Check issues field
        assert_eq!(
            template_json.get("issues").and_then(|v| v.as_str()),
            Some("#123")
        );

        // Check breaking field is omitted (skip_serializing_if)
        assert!(template_json.get("breaking").is_none());

        // Check scope field
        assert_eq!(
            template_json.get("scope").and_then(|v| v.as_str()),
            Some("schema")
        );
    }

    #[test]
    fn test_commit_template_deserialization() {
        // Test valid JSON (serde expects "type" due to rename)
        let valid_json = r#"{
            "type": "feat",
            "subject": "test subject",
            "details": null,
            "issues": null,
            "breaking": null,
            "scope": null
        }"#;

        let result: Result<CommitTemplate, _> = serde_json::from_str(valid_json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().commit_type, CommitType::Feat);

        // Test invalid commit type
        let invalid_type_json = r#"{
            "type": "invalid_type",
            "subject": "this has an invalid type",
            "details": null,
            "issues": null,
            "breaking": null,
            "scope": null
        }"#;

        let result: Result<CommitTemplate, _> = serde_json::from_str(invalid_type_json);
        assert!(result.is_err(), "Should reject invalid commit type");
    }
}
