use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use handlebars::Handlebars;
use schemars::schema::{Metadata, Schema};
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
#[derive(Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(description = "The type of a commit message. Choose based on the nature of the change.")]
#[schemars(title = "Commit Type")]
pub enum CommitType {
    #[schemars(
        description = "A new feature or enhancement (not docs/readme). E.g., adding a login system."
    )]
    Feat,
    #[schemars(description = "A bug fix or error correction. E.g., fixing a crash in the parser.")]
    Fix,
    #[schemars(
        description = "Code restructuring without behavior change. E.g., splitting a large function."
    )]
    Refactor,
    #[schemars(
        description = "Routine maintenance or updates (e.g., dependency bumps). E.g., updating serde."
    )]
    Chore,
    #[schemars(
        description = "Documentation updates (e.g., README, comments). E.g., adding API docs."
    )]
    Docs,
    #[schemars(
        description = "Formatting or stylistic changes (e.g., linting). E.g., fixing whitespace."
    )]
    Style,
    #[schemars(description = "Test additions or updates. E.g., adding unit tests for a feature.")]
    Test,
    #[schemars(description = "Build system or script changes. E.g., updating the Dockerfile.")]
    Build,
    #[schemars(
        description = "CI/CD configuration updates. E.g., modifying a GitHub Actions workflow."
    )]
    Ci,
    #[schemars(description = "Performance improvements. E.g., optimizing a query execution time.")]
    Perf,
}

// Helper function to add examples and title to schema
fn schema_with_examples<T: JsonSchema>(
    gen: &mut schemars::gen::SchemaGenerator,
    examples: Vec<serde_json::Value>,
    title: &str,
) -> Schema {
    let mut schema = T::json_schema(gen);
    if let Schema::Object(obj) = &mut schema {
        let metadata = obj
            .metadata
            .get_or_insert_with(|| Box::new(Metadata::default()));
        metadata.examples = examples;
        metadata.title = Some(title.to_string());
    }
    schema
}

// Macro to generate schema functions with examples and titles
macro_rules! define_schema_fns {
    ($(
        $fn_name:ident: $type:ty => {
            title: $title:expr,
            examples: [$($example:expr),+]
        }
    ),*) => {
        $(
            fn $fn_name(gen: &mut schemars::gen::SchemaGenerator) -> Schema {
                schema_with_examples::<$type>(gen, vec![$($example),+], $title)
            }
        )*
    };
}

// Define all schema functions using the macro
define_schema_fns! {
    subject_schema: String => {
        title: "Subject",
        examples: [
            json!("add user login endpoint"),
            json!("fix memory leak in image processing")
        ]
    },
    details_schema: Option<String> => {
        title: "Details",
        examples: [
            json!("- Add JWT auth for security\n- Update tests for coverage"),
            json!("- Fix memory leak when processing large images\n- Add unit tests to prevent regression")
        ]
    },
    issues_schema: Option<String> => {
        title: "Issues",
        examples: [
            json!("#123"),
            json!("Fixes #456"),
            json!("Resolves #789, #101")
        ]
    },
    breaking_schema: Option<String> => {
        title: "Breaking Changes",
        examples: [
            json!("Drop support for old API"),
            json!("Change authentication flow")
        ]
    },
    scope_schema: Option<String> => {
        title: "Scope",
        examples: [
            json!("auth"),
            json!("ui"),
            json!("api"),
            json!("db")
        ]
    }
}

// Struct for commit template with JSON-friendly fields
#[derive(Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[schemars(
    description = "The data for a commit message template. Format: '{type}({scope}): {subject}' (max 50 chars) on the first line, followed by optional bullet points (max 80 chars each) describing meaningful changes."
)]
pub struct CommitTemplate {
    #[serde(rename = "type")]
    #[schemars(
        description = "The type of the commit message. Select from CommitType based on the change.",
        title = "Type"
    )]
    pub r#type: CommitType,

    #[schemars(
        description = "A concise summary of the change (max 50 chars with type). Start with lowercase verb in present tense (e.g., 'add', 'fix', 'update'). Focus on 'what' and 'why', not 'how'.",
        schema_with = "subject_schema"
    )]
    pub subject: String,

    #[schemars(
        description = "Optional details as bullet points (max 80 chars each). Start each bullet with '- ' followed by present tense verb. Focus on explaining the change's purpose and impact. Include context that's not obvious from the code.",
        schema_with = "details_schema"
    )]
    pub details: Option<String>,

    #[schemars(
        description = "Optional issue/ticket references. Format: '#123' or 'Fixes #456' or 'Resolves #789, #101'",
        schema_with = "issues_schema"
    )]
    pub issues: Option<String>,

    #[schemars(
        description = "Optional breaking change description. Include this when your change breaks backward compatibility. Explain what breaks and how users should migrate.",
        schema_with = "breaking_schema"
    )]
    pub breaking: Option<String>,

    #[schemars(
        description = "Optional scope of the change (component affected). Use lowercase with hyphens if needed (e.g., 'auth', 'ui', 'api', 'db').",
        schema_with = "scope_schema"
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
        for &template_name in config::defaults::defaults::AVAILABLE_TEMPLATES {
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
        config::file::save_template(name, content).map_err(|e| {
            TemplateError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

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
    fn test_instruct_macro_serialization() {
        let schema = schemars::schema_for!(CommitTemplate);
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
    }
}
