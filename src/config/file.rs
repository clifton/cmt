use std::fs;
use std::io;
use std::path::PathBuf;

use super::defaults;
use super::ConfigError;

/// Create a new configuration file at the specified path
pub fn create_config_file(path: Option<&str>) -> Result<PathBuf, ConfigError> {
    let config_path = if let Some(path) = path {
        PathBuf::from(path)
    } else {
        PathBuf::from(defaults::DEFAULT_CONFIG_FILENAME)
    };

    // Check if file already exists
    if config_path.exists() {
        return Err(ConfigError::IoError(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Configuration file already exists at {:?}", config_path),
        )));
    }

    // Create parent directories if needed
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    // Write example config
    fs::write(&config_path, defaults::example_config())?;

    Ok(config_path)
}

/// Get the global configuration directory
pub fn global_config_dir() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        Some(PathBuf::from(home).join(defaults::GLOBAL_CONFIG_DIRNAME))
    } else {
        None
    }
}

/// Get the global configuration file path
pub fn global_config_file() -> Option<PathBuf> {
    global_config_dir().map(|dir| dir.join(defaults::GLOBAL_CONFIG_FILENAME))
}

/// Create the global configuration directory and file
pub fn create_global_config() -> Result<PathBuf, ConfigError> {
    let global_dir = global_config_dir().ok_or_else(|| {
        ConfigError::IoError(io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine home directory",
        ))
    })?;

    // Create directory if it doesn't exist
    if !global_dir.exists() {
        fs::create_dir_all(&global_dir)?;
    }

    let global_file = global_dir.join(defaults::GLOBAL_CONFIG_FILENAME);

    // Create file if it doesn't exist
    if !global_file.exists() {
        fs::write(&global_file, defaults::example_config())?;
    }

    Ok(global_file)
}

/// Find the project configuration file by walking up the directory tree
pub fn find_project_config() -> Option<PathBuf> {
    let current_dir = std::env::current_dir().ok()?;
    let mut dir = current_dir.as_path();

    loop {
        let config_path = dir.join(defaults::DEFAULT_CONFIG_FILENAME);
        if config_path.exists() {
            return Some(config_path);
        }

        if let Some(parent) = dir.parent() {
            dir = parent;
        } else {
            break;
        }
    }

    None
}

/// Get the template directory
pub fn template_dir() -> Option<PathBuf> {
    global_config_dir().map(|dir| dir.join("templates"))
}

/// Create the template directory and default templates
pub fn create_template_dir() -> Result<PathBuf, ConfigError> {
    let template_dir = template_dir().ok_or_else(|| {
        ConfigError::IoError(io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine template directory",
        ))
    })?;

    // Create directory if it doesn't exist
    if !template_dir.exists() {
        fs::create_dir_all(&template_dir)?;
    }

    // Create default templates
    let simple_path = template_dir.join("simple.hbs");
    if !simple_path.exists() {
        fs::write(&simple_path, defaults::simple_template())?;
    }

    let conventional_path = template_dir.join("conventional.hbs");
    if !conventional_path.exists() {
        fs::write(&conventional_path, defaults::conventional_template())?;
    }

    let detailed_path = template_dir.join("detailed.hbs");
    if !detailed_path.exists() {
        fs::write(&detailed_path, defaults::detailed_template())?;
    }

    Ok(template_dir)
}

/// Get a list of available templates
pub fn list_templates() -> Result<Vec<String>, ConfigError> {
    let template_dir = template_dir().ok_or_else(|| {
        ConfigError::IoError(io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine template directory",
        ))
    })?;

    if !template_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(template_dir)?;
    let mut templates = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension == "hbs" {
                    if let Some(name) = path.file_stem() {
                        if let Some(name_str) = name.to_str() {
                            templates.push(name_str.to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(templates)
}

/// Get the path to a template, prioritizing file system templates over defaults
pub fn get_template_path(name: &str) -> Result<PathBuf, ConfigError> {
    // First check if the template exists in the file system
    if let Some(template_dir) = template_dir() {
        let template_path = template_dir.join(format!("{}.hbs", name));
        if template_path.exists() {
            return Ok(template_path);
        }
    }

    // If not found in file system, check if it's a built-in template
    match name {
        "simple" | "conventional" | "detailed" => {
            // For built-in templates, we don't have a real path, so we create a placeholder
            // This indicates it's a built-in template that should be handled specially
            Ok(PathBuf::from(format!("__builtin__/{}.hbs", name)))
        }
        _ => Err(ConfigError::IoError(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Template '{}' not found", name),
        ))),
    }
}

/// Get the content of a template
pub fn get_template(name: &str) -> Result<String, ConfigError> {
    // Get the template path, which prioritizes file system templates
    let template_path = get_template_path(name)?;

    // Check if it's a built-in template
    if let Some(path_str) = template_path.to_str() {
        if path_str.starts_with("__builtin__/") {
            // It's a built-in template, return the appropriate content
            match name {
                "simple" => return Ok(defaults::simple_template()),
                "conventional" => return Ok(defaults::conventional_template()),
                "detailed" => return Ok(defaults::detailed_template()),
                _ => {} // This shouldn't happen given the logic in get_template_path
            }
        }
    }

    // It's a file system template, read its content
    match fs::read_to_string(&template_path) {
        Ok(content) => Ok(content),
        Err(e) => Err(ConfigError::IoError(e)),
    }
}

/// Save a template
pub fn save_template(name: &str, content: &str) -> Result<(), ConfigError> {
    let template_dir = template_dir().ok_or_else(|| {
        ConfigError::IoError(io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine template directory",
        ))
    })?;

    // Create directory if it doesn't exist
    if !template_dir.exists() {
        fs::create_dir_all(&template_dir)?;
    }

    let template_path = template_dir.join(format!("{}.hbs", name));
    fs::write(template_path, content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    #[serial_test::serial]
    fn test_get_template_from_file() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();

        // Override the HOME environment variable to use our temp directory
        let original_home = env::var("HOME").unwrap_or_default();
        env::set_var("HOME", temp_dir.path());

        // Create the config directory structure
        let config_dir = temp_dir.path().join(defaults::GLOBAL_CONFIG_DIRNAME);
        let template_dir = config_dir.join("templates");
        fs::create_dir_all(&template_dir).unwrap();

        // Create a test template file with a unique name that doesn't conflict with built-ins
        let template_name = "custom-test-template";
        let template_content = "Test template content";
        let template_path = template_dir.join(format!("{}.hbs", template_name));
        fs::write(&template_path, template_content).unwrap();

        // Test getting the template
        let result = get_template(template_name);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), template_content);

        // Restore the original HOME environment variable
        if original_home.is_empty() {
            env::remove_var("HOME");
        } else {
            env::set_var("HOME", original_home);
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_get_builtin_template() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();

        // Override the HOME environment variable to use our temp directory
        // This ensures we don't find any existing template files
        let original_home = env::var("HOME").unwrap_or_default();
        env::set_var("HOME", temp_dir.path());

        // Test getting built-in templates
        let simple_result = get_template("simple");
        assert!(simple_result.is_ok());
        assert_eq!(simple_result.unwrap(), defaults::simple_template());

        let conventional_result = get_template("conventional");
        assert!(conventional_result.is_ok());
        assert_eq!(
            conventional_result.unwrap(),
            defaults::conventional_template()
        );

        let detailed_result = get_template("detailed");
        assert!(detailed_result.is_ok());
        assert_eq!(detailed_result.unwrap(), defaults::detailed_template());

        // Restore the original HOME environment variable
        if original_home.is_empty() {
            env::remove_var("HOME");
        } else {
            env::set_var("HOME", original_home);
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_get_nonexistent_template() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();

        // Override the HOME environment variable to use our temp directory
        let original_home = env::var("HOME").unwrap_or_default();
        env::set_var("HOME", temp_dir.path());

        // Test getting a non-existent template
        let result = get_template("nonexistent-template");
        assert!(result.is_err());
        let error = result.unwrap_err();
        match error {
            ConfigError::IoError(e) => {
                assert_eq!(e.kind(), io::ErrorKind::NotFound);
                assert!(e
                    .to_string()
                    .contains("Template 'nonexistent-template' not found"));
            }
            _ => panic!("Expected IoError, got {:?}", error),
        }

        // Restore the original HOME environment variable
        if original_home.is_empty() {
            env::remove_var("HOME");
        } else {
            env::set_var("HOME", original_home);
        }
    }
}
