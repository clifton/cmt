use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;

use handlebars::Handlebars;
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

/// Template data for rendering
#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateData {
    pub r#type: String,
    pub subject: String,
    pub details: Option<String>,
    pub issues: Option<String>,
    pub breaking: Option<String>,
    pub scope: Option<String>,
}

impl Default for TemplateData {
    fn default() -> Self {
        Self {
            r#type: "feat".to_string(),
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
        data: &TemplateData,
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

        let data = TemplateData {
            r#type: "feat".to_string(),
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
        let data_with_scope = TemplateData {
            r#type: "feat".to_string(),
            subject: "add new feature".to_string(),
            scope: Some("ui".to_string()),
            ..Default::default()
        };

        let rendered = manager.render("test", &data_with_scope).unwrap();
        assert_eq!(rendered, "feat: add new feature (ui)\n\n");

        // Without scope
        let data_without_scope = TemplateData {
            r#type: "feat".to_string(),
            subject: "add new feature".to_string(),
            ..Default::default()
        };

        let rendered = manager.render("test", &data_without_scope).unwrap();
        assert_eq!(rendered, "feat: add new feature\n\n");
    }
}
