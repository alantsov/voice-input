use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

fn normalize_selected_model(model: &str) -> String {
    match model {
        "base" | "tiny" => "small".to_string(),
        other => other.to_string(),
    }
}

/// Configuration structure for the application
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// The selected model for transcription
    pub selected_model: String,

    /// Whether to translate (to English) instead of transcribe
    #[serde(default)]
    pub translate: bool,

    /// Compute device preference for whisper: "cpu" or "gpu" (gpu requires cuda build)
    #[serde(default = "default_device")] 
    pub device: String,
}

fn default_device() -> String {
    if cfg!(feature = "cuda") {
        "gpu".to_string()
    } else {
        "cpu".to_string()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            selected_model: "small".to_string(),
            translate: false,
            device: default_device(),
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
                Ok(contents) => match serde_json::from_str::<Config>(&contents) {
                    Ok(mut config) => {
                        // Normalize deprecated model selections
                        let normalized = normalize_selected_model(&config.selected_model);
                        if normalized != config.selected_model {
                            config.selected_model = normalized;
                            // Try to persist the migration silently
                            if let Err(e) = save_config(&config) {
                                eprintln!("Failed to save migrated config: {}", e);
                            }
                        }
                        return config;
                    }
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
    config.selected_model = normalize_selected_model(model);
    save_config(&config)
}

/// Get the selected model
pub fn get_selected_model() -> String {
    let cfg = load_config();
    normalize_selected_model(&cfg.selected_model)
}

/// Save just the translate flag
pub fn save_translate_enabled(translate: bool) -> io::Result<()> {
    let mut config = load_config();
    config.translate = translate;
    save_config(&config)
}

/// Get the translate flag
pub fn get_translate_enabled() -> bool {
    load_config().translate
}

/// Save just the compute device ("cpu" or "gpu"). When built without CUDA, always saves/returns "cpu".
pub fn save_device(device: &str) -> io::Result<()> {
    let mut cfg = load_config();
    let dev = device.to_lowercase();
    let normalized = match dev.as_str() {
        "gpu" => "gpu",
        _ => "cpu",
    };
    // If cuda feature is not enabled, force cpu regardless of requested value
    let final_dev = if cfg!(feature = "cuda") {
        normalized
    } else {
        "cpu"
    };
    cfg.device = final_dev.to_string();
    save_config(&cfg)
}

/// Get the compute device string ("cpu" or "gpu"). When built without CUDA, always "cpu".
pub fn get_device() -> String {
    let cfg = load_config();
    if cfg!(feature = "cuda") {
        match cfg.device.to_lowercase().as_str() {
            "gpu" => "gpu".to_string(),
            _ => "cpu".to_string(),
        }
    } else {
        "cpu".to_string()
    }
}

/// Convenience: whether GPU acceleration should be used in this build
pub fn use_gpu() -> bool {
    cfg!(feature = "cuda") && matches!(get_device().as_str(), "gpu")
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
