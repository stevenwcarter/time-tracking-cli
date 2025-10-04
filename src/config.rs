use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Day of the week to start the week (Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday)
    pub week_start_day: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            week_start_day: Some("Saturday".to_string()),
        }
    }
}

impl Config {
    pub fn load() -> Result<Config, Box<dyn std::error::Error>> {
        let config_path = get_config_path()?;
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            // Create default config file
            let default_config = Config::default();
            fs::create_dir_all(config_path.parent().unwrap())?;
            let toml_content = toml::to_string_pretty(&default_config)?;
            fs::write(&config_path, toml_content)?;
            Ok(default_config)
        }
    }
}

fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(config_dir) = dirs::config_dir() {
        Ok(config_dir.join("time-tracking-cli").join("config.toml"))
    } else {
        // Fallback to home directory
        let home = home_dir().ok_or("Could not determine home directory")?;
        Ok(home
            .join(".config")
            .join("time-tracking-cli")
            .join("config.toml"))
    }
}
