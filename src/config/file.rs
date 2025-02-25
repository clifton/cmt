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
        PathBuf::from(defaults::defaults::DEFAULT_CONFIG_FILENAME)
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
        Some(PathBuf::from(home).join(defaults::defaults::GLOBAL_CONFIG_DIRNAME))
    } else {
        None
    }
}

/// Get the global configuration file path
pub fn global_config_file() -> Option<PathBuf> {
    global_config_dir().map(|dir| dir.join(defaults::defaults::GLOBAL_CONFIG_FILENAME))
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

    let global_file = global_dir.join(defaults::defaults::GLOBAL_CONFIG_FILENAME);

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
        let config_path = dir.join(defaults::defaults::DEFAULT_CONFIG_FILENAME);
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

/// Get the content of a template
pub fn get_template(name: &str) -> Result<String, ConfigError> {
    let template_dir = template_dir().ok_or_else(|| {
        ConfigError::IoError(io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine template directory",
        ))
    })?;

    let template_path = template_dir.join(format!("{}.hbs", name));

    if !template_path.exists() {
        return Err(ConfigError::IoError(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Template '{}' not found", name),
        )));
    }

    let content = fs::read_to_string(template_path)?;
    Ok(content)
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
