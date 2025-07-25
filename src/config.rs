use std::fs;
use std::path::{Path, PathBuf};
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