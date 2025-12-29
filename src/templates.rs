use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use handlebars::Handlebars;
use schemars::JsonSchema;
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

// Enum for commit types with lowercase serialization
// Priority order (highest to lowest): fix > feat > perf > refactor > test > build > ci > chore > style > docs
#[derive(Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(
    description = "The type of a commit message. Choose based on the PRIMARY purpose using priority: fix > feat > perf > refactor > test > build > ci > chore > style > docs. If a commit fixes a bug AND updates docs, use 'fix'.",
    title = "Commit Type"
)]
pub enum CommitType {
    #[schemars(
        description = "PRIORITY 1: Bug fix or error correction. Use if ANY bug is fixed, even with other changes. E.g., fixing null pointer, crash, incorrect behavior."
    )]
    Fix,
    #[schemars(
        description = "PRIORITY 2: New feature or enhancement to functionality (not docs/readme). Use when adding new capabilities, APIs, or user-facing behavior."
    )]
    Feat,
    #[schemars(
        description = "PRIORITY 3: Performance improvements. Use when the primary goal is optimization. E.g., caching, algorithm improvements."
    )]
    Perf,
    #[schemars(
        description = "PRIORITY 4: Code restructuring WITHOUT behavior change. Only use if no bugs fixed and no features added. E.g., renaming, extracting functions."
    )]
    Refactor,
    #[schemars(
        description = "PRIORITY 5: Test additions or updates. Use when changes are primarily about test coverage."
    )]
    Test,
    #[schemars(
        description = "PRIORITY 6: Build system or external dependency changes. E.g., Dockerfile, Makefile, external deps."
    )]
    Build,
    #[schemars(
        description = "PRIORITY 7: CI/CD configuration changes. E.g., GitHub Actions, Jenkins, CircleCI."
    )]
    Ci,
    #[schemars(
        description = "PRIORITY 8: Maintenance tasks, internal dependency updates, tooling. E.g., updating internal deps, config files."
    )]
    Chore,
    #[schemars(
        description = "PRIORITY 9: Formatting or stylistic changes ONLY. No logic changes. E.g., linting, whitespace, code style."
    )]
    Style,
    #[schemars(
        description = "PRIORITY 10 (LOWEST): Documentation ONLY. Use ONLY when there are NO code logic changes. E.g., README, comments, API docs."
    )]
    Docs,
}

// Struct for commit template with JSON-friendly fields
#[derive(Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[schemars(
    description = "Commit message data. Format: '{type}: {subject}'. Keep first line under 50 chars. Do NOT use scope."
)]
pub struct CommitTemplate {
    #[serde(rename = "type")]
    #[schemars(
        description = "The type of the commit message. Select from CommitType based on the change.",
        title = "Type"
    )]
    pub r#type: CommitType,

    #[schemars(
        description = "Brief subject line, ideally under 50 chars total with type prefix. Start with lowercase verb (add, fix, update). Be specific.",
        title = "Subject",
        example = &"add user login endpoint",
        example = &"fix null check in auth"
    )]
    pub subject: String,

    #[schemars(
        description = "Optional details as bullet points (max 79 chars each). Start each bullet with '- ' followed by present tense verb. Focus on explaining the change's purpose and impact. Include context that's not obvious from the code.",
        title = "Details",
        example = &"- Add JWT auth for security\n- Update tests for coverage"
    )]
    pub details: Option<String>,

    #[schemars(
        description = "Optional issue/ticket references. Format: '#123' or 'Fixes #456' or 'Resolves #789, #101'",
        title = "Issues",
        example = &"#123",
        example = &"Fixes #456"
    )]
    pub issues: Option<String>,

    #[schemars(
        description = "Optional breaking change description. Include this when your change breaks backward compatibility. Explain what breaks and how users should migrate.",
        title = "Breaking Changes",
        example = &"Drop support for old API"
    )]
    pub breaking: Option<String>,

    #[schemars(
        description = "LEAVE NULL. Only set for monorepos with packages/apps directories. Do not use for single projects.",
        title = "Scope",
        example = &"auth",
        example = &"api"
    )]
    pub scope: Option<String>,
}

impl Default for CommitTemplate {
    fn default() -> Self {
        Self {
            r#type: CommitType::Feat,
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
            r#type: CommitType::Feat,
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
            r#type: CommitType::Feat,
            subject: "add new feature".to_string(),
            scope: Some("ui".to_string()),
            ..Default::default()
        };

        let rendered = manager.render("test", &data_with_scope).unwrap();
        assert_eq!(rendered, "feat: add new feature (ui)\n\n");

        // Without scope
        let data_without_scope = CommitTemplate {
            r#type: CommitType::Feat,
            subject: "add new feature".to_string(),
            ..Default::default()
        };

        let rendered = manager.render("test", &data_without_scope).unwrap();
        assert_eq!(rendered, "feat: add new feature\n\n");
    }

    #[test]
    fn test_commit_template_json_schema() {
        // Generate the JSON schema for CommitTemplate
        let schema = schemars::schema_for!(CommitTemplate);
        let schema_str = serde_json::to_string_pretty(&schema).unwrap();

        // Verify schema has expected structure
        assert!(schema_str.contains("CommitTemplate"));
        assert!(schema_str.contains("description"));
        assert!(schema_str.contains("type"));
        assert!(schema_str.contains("subject"));

        // Print schema for debugging
        println!("Generated schema:\n{}", schema_str);
    }

    #[test]
    fn test_schema_validates_commit_template() {
        // Create a valid CommitTemplate instance
        let template = CommitTemplate {
            r#type: CommitType::Feat,
            subject: "add schema validation test".to_string(),
            details: Some("- Test schema validation\n- Ensure examples work".to_string()),
            issues: Some("#123".to_string()),
            breaking: None,
            scope: Some("schema".to_string()),
        };

        // Serialize the template to JSON
        let template_json = serde_json::to_value(&template).unwrap();

        // Check type field
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

        // Check breaking field is null
        assert!(template_json.get("breaking").unwrap().is_null());

        // Check scope field
        assert_eq!(
            template_json.get("scope").and_then(|v| v.as_str()),
            Some("schema")
        );
    }

    #[test]
    fn test_schema_rejects_invalid_commit_template() {
        // Test 1: Invalid commit type
        let invalid_type_json = r#"{
            "type": "invalid_type",
            "subject": "this has an invalid type",
            "details": null,
            "issues": null,
            "breaking": null,
            "scope": null
        }"#;

        let result: Result<CommitTemplate, _> = serde_json::from_str(invalid_type_json);
        assert!(result.is_err(), "Schema should reject invalid commit type");
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("invalid_type"),
            "Error should mention the invalid type"
        );

        // Test 2: Missing required field (subject)
        let missing_subject_json = r#"{
            "type": "feat",
            "details": null,
            "issues": null,
            "breaking": null,
            "scope": null
        }"#;

        let result: Result<CommitTemplate, _> = serde_json::from_str(missing_subject_json);
        assert!(
            result.is_err(),
            "Schema should reject missing required field"
        );
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("subject") || error.contains("missing field"),
            "Error should mention the missing field"
        );

        // Test 3: Wrong data type for a field
        let wrong_type_json = r#"{
            "type": "feat",
            "subject": "valid subject",
            "details": 12345,
            "issues": null,
            "breaking": null,
            "scope": null
        }"#;

        let result: Result<CommitTemplate, _> = serde_json::from_str(wrong_type_json);
        assert!(result.is_err(), "Schema should reject wrong data type");
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("details") || error.contains("expected a string"),
            "Error should mention the field with wrong type"
        );
    }
}
