use std::fs;
use std::path::{PathBuf};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::io;

/// Configuration structure for the application
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// The selected model for transcription
    pub selected_model: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            selected_model: "base".to_string(),
        }
    }
}

/// Get the configuration directory path
fn get_config_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "voice_input").map(|dirs| dirs.config_dir().to_path_buf())
}

/// Get the data directory path for storing model weights
pub fn get_data_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "voice_input").map(|dirs| dirs.data_dir().to_path_buf())
}

/// Get the models directory path
pub fn get_models_dir() -> Option<PathBuf> {
    get_data_dir().map(|dir| dir.join("models"))
}

/// Get the configuration file path
fn get_config_file_path() -> Option<PathBuf> {
    get_config_dir().map(|dir| dir.join("config.json"))
}

/// Ensure the configuration directory exists
fn ensure_config_dir() -> io::Result<PathBuf> {
    let config_dir = get_config_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine configuration directory",
        )
    })?;

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
    }

    Ok(config_dir)
}

/// Ensure the models directory exists
pub fn ensure_models_dir() -> io::Result<PathBuf> {
    let models_dir = get_models_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine models directory",
        )
    })?;

    if !models_dir.exists() {
        fs::create_dir_all(&models_dir)?;
    }

    Ok(models_dir)
}

/// Load the configuration from the file
pub fn load_config() -> Config {
    if let Some(config_path) = get_config_file_path() {
        if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(contents) => match serde_json::from_str(&contents) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!("Failed to parse config file: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!("Failed to read config file: {}", e);
                }
            }
        }
    }

    // Return default config if loading fails
    Config::default()
}

/// Save the configuration to the file
pub fn save_config(config: &Config) -> io::Result<()> {
    let config_dir = ensure_config_dir()?;
    let config_path = config_dir.join("config.json");

    let json = serde_json::to_string_pretty(config)?;
    fs::write(config_path, json)?;

    Ok(())
}

/// Save just the selected model
pub fn save_selected_model(model: &str) -> io::Result<()> {
    let mut config = load_config();
    config.selected_model = model.to_string();
    save_config(&config)
}

/// Get the selected model
pub fn get_selected_model() -> String {
    load_config().selected_model
}

/// Get the full path for a model file
/// If the model file exists in the XDG data directory, return that path
/// If not, check if it exists in the current directory (for backward compatibility)
/// Returns None if the model file doesn't exist in either location
pub fn get_model_path(model_name: &str) -> Option<PathBuf> {
    // First check in XDG data directory
    if let Some(models_dir) = get_models_dir() {
        let xdg_path = models_dir.join(model_name);
        if xdg_path.exists() {
            return Some(xdg_path);
        }
    }
    
    // Then check in current directory (for backward compatibility)
    let current_dir_path = PathBuf::from(model_name);
    if current_dir_path.exists() {
        return Some(current_dir_path);
    }
    
    None
}

/// Get the path where a model file should be saved
/// This will be in the XDG data directory
pub fn get_model_save_path(model_name: &str) -> io::Result<PathBuf> {
    let models_dir = ensure_models_dir()?;
    Ok(models_dir.join(model_name))
}