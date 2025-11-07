use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Day of the week to start the week (Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday)
    pub week_start_day: Option<String>,
    /// Directory where time tracking files are stored (defaults to ~/.time-tracking)
    pub data_directory: Option<String>,
    /// Path to a template file to use when creating new time tracking files
    pub template_file: Option<String>,
    /// Optional prefix
    /// Optional suffix
}

impl Default for Config {
    fn default() -> Self {
        Self {
            week_start_day: Some("Saturday".to_string()),
            data_directory: None,
            template_file: None,
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

    /// Apply CLI arguments to this config, returning a new config with CLI overrides applied
    /// CLI arguments take priority over config file values
    pub fn with_args_applied(
        mut self,
        week_start_day: Option<String>,
        data_directory: Option<String>,
        template_file: Option<String>,
    ) -> Self {
        if let Some(week_start_day) = week_start_day {
            self.week_start_day = Some(week_start_day);
        }
        if let Some(data_directory) = data_directory {
            self.data_directory = Some(data_directory);
        }
        if let Some(template_file) = template_file {
            self.template_file = Some(template_file);
        }
        self
    }

    /// Get the week start day with fallback to default
    pub fn get_week_start_day(&self) -> String {
        self.week_start_day
            .clone()
            .unwrap_or_else(|| "Saturday".to_string())
    }

    /// Get the data directory as Option<&str>
    pub fn get_data_directory(&self) -> Option<&str> {
        self.data_directory.as_deref()
    }

    /// Get the template file as Option<&str>
    pub fn get_template_file(&self) -> Option<&str> {
        self.template_file.as_deref()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config() -> Config {
        Config {
            week_start_day: Some("Monday".to_string()),
            data_directory: Some("/test/data".to_string()),
            template_file: Some("/test/template.md".to_string()),
        }
    }

    fn create_empty_config() -> Config {
        Config {
            week_start_day: None,
            data_directory: None,
            template_file: None,
        }
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.week_start_day, Some("Saturday".to_string()));
        assert_eq!(config.data_directory, None);
        assert_eq!(config.template_file, None);
    }

    #[test]
    fn test_with_args_applied_override_all() {
        let config = create_test_config();
        let updated = config.with_args_applied(
            Some("Sunday".to_string()),
            Some("/new/data".to_string()),
            Some("/new/template.md".to_string()),
        );

        assert_eq!(updated.week_start_day, Some("Sunday".to_string()));
        assert_eq!(updated.data_directory, Some("/new/data".to_string()));
        assert_eq!(updated.template_file, Some("/new/template.md".to_string()));
    }

    #[test]
    fn test_with_args_applied_partial_override() {
        let config = create_test_config();
        let updated = config.with_args_applied(
            Some("Sunday".to_string()),
            None, // Don't override data_directory
            Some("/new/template.md".to_string()),
        );

        assert_eq!(updated.week_start_day, Some("Sunday".to_string()));
        assert_eq!(updated.data_directory, Some("/test/data".to_string())); // Original value
        assert_eq!(updated.template_file, Some("/new/template.md".to_string()));
    }

    #[test]
    fn test_with_args_applied_no_override() {
        let config = create_test_config();
        let updated = config.with_args_applied(None, None, None);

        assert_eq!(updated.week_start_day, Some("Monday".to_string()));
        assert_eq!(updated.data_directory, Some("/test/data".to_string()));
        assert_eq!(updated.template_file, Some("/test/template.md".to_string()));
    }

    #[test]
    fn test_with_args_applied_empty_config() {
        let config = create_empty_config();
        let updated = config.with_args_applied(
            Some("Friday".to_string()),
            Some("/args/data".to_string()),
            Some("/args/template.md".to_string()),
        );

        assert_eq!(updated.week_start_day, Some("Friday".to_string()));
        assert_eq!(updated.data_directory, Some("/args/data".to_string()));
        assert_eq!(updated.template_file, Some("/args/template.md".to_string()));
    }

    #[test]
    fn test_get_week_start_day_with_value() {
        let config = create_test_config();
        assert_eq!(config.get_week_start_day(), "Monday");
    }

    #[test]
    fn test_get_week_start_day_with_none() {
        let config = create_empty_config();
        assert_eq!(config.get_week_start_day(), "Saturday"); // Default fallback
    }

    #[test]
    fn test_get_data_directory_with_value() {
        let config = create_test_config();
        assert_eq!(config.get_data_directory(), Some("/test/data"));
    }

    #[test]
    fn test_get_data_directory_with_none() {
        let config = create_empty_config();
        assert_eq!(config.get_data_directory(), None);
    }

    #[test]
    fn test_get_template_file_with_value() {
        let config = create_test_config();
        assert_eq!(config.get_template_file(), Some("/test/template.md"));
    }

    #[test]
    fn test_get_template_file_with_none() {
        let config = create_empty_config();
        assert_eq!(config.get_template_file(), None);
    }

    #[test]
    fn test_config_serialization() {
        let config = create_test_config();
        let toml_str = toml::to_string(&config).unwrap();

        assert!(toml_str.contains("week_start_day = \"Monday\""));
        assert!(toml_str.contains("data_directory = \"/test/data\""));
        assert!(toml_str.contains("template_file = \"/test/template.md\""));
    }

    #[test]
    fn test_config_deserialization() {
        let toml_str = r#"
            week_start_day = "Tuesday"
            data_directory = "/custom/path"
            template_file = "/custom/template.md"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.week_start_day, Some("Tuesday".to_string()));
        assert_eq!(config.data_directory, Some("/custom/path".to_string()));
        assert_eq!(
            config.template_file,
            Some("/custom/template.md".to_string())
        );
    }

    #[test]
    fn test_config_deserialization_partial() {
        let toml_str = r#"
            week_start_day = "Wednesday"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.week_start_day, Some("Wednesday".to_string()));
        assert_eq!(config.data_directory, None);
        assert_eq!(config.template_file, None);
    }

    #[test]
    fn test_config_deserialization_empty() {
        let toml_str = "";

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.week_start_day, None);
        assert_eq!(config.data_directory, None);
        assert_eq!(config.template_file, None);
    }

    #[test]
    fn test_config_roundtrip() {
        let original = create_test_config();
        let toml_str = toml::to_string(&original).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(original.week_start_day, deserialized.week_start_day);
        assert_eq!(original.data_directory, deserialized.data_directory);
        assert_eq!(original.template_file, deserialized.template_file);
    }

    #[test]
    fn test_config_clone() {
        let config1 = create_test_config();
        let config2 = config1.clone();

        assert_eq!(config1.week_start_day, config2.week_start_day);
        assert_eq!(config1.data_directory, config2.data_directory);
        assert_eq!(config1.template_file, config2.template_file);
    }

    #[test]
    fn test_chaining_with_args_applied() {
        let config = Config::default()
            .with_args_applied(Some("Monday".to_string()), None, None)
            .with_args_applied(None, Some("/data".to_string()), None)
            .with_args_applied(None, None, Some("/template.md".to_string()));

        assert_eq!(config.get_week_start_day(), "Monday");
        assert_eq!(config.get_data_directory(), Some("/data"));
        assert_eq!(config.get_template_file(), Some("/template.md"));
    }

    // Integration test that creates actual config file
    #[test]
    fn test_config_serialization_roundtrip_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        let original_config = create_test_config();

        // Serialize to string then write to file
        let toml_content = toml::to_string(&original_config).unwrap();
        fs::write(&config_path, &toml_content).unwrap();

        // Read from file and deserialize
        let file_content = fs::read_to_string(&config_path).unwrap();
        let loaded_config: Config = toml::from_str(&file_content).unwrap();

        // Verify roundtrip
        assert_eq!(original_config.week_start_day, loaded_config.week_start_day);
        assert_eq!(original_config.data_directory, loaded_config.data_directory);
        assert_eq!(original_config.template_file, loaded_config.template_file);
    }

    #[test]
    fn test_config_partial_override_complex_scenario() {
        // Start with default config
        let config = Config::default();
        assert_eq!(config.get_week_start_day(), "Saturday");
        assert_eq!(config.get_data_directory(), None);
        assert_eq!(config.get_template_file(), None);

        // Apply some args
        let config = config.with_args_applied(
            None, // Keep default week start
            Some("/new/data".to_string()),
            None,
        );
        assert_eq!(config.get_week_start_day(), "Saturday");
        assert_eq!(config.get_data_directory(), Some("/new/data"));
        assert_eq!(config.get_template_file(), None);

        // Override week start day but keep others
        let config = config.with_args_applied(Some("Monday".to_string()), None, None);
        assert_eq!(config.get_week_start_day(), "Monday");
        assert_eq!(config.get_data_directory(), Some("/new/data"));
        assert_eq!(config.get_template_file(), None);

        // Add template file
        let config = config.with_args_applied(None, None, Some("/template.md".to_string()));
        assert_eq!(config.get_week_start_day(), "Monday");
        assert_eq!(config.get_data_directory(), Some("/new/data"));
        assert_eq!(config.get_template_file(), Some("/template.md"));
    }

    #[test]
    fn test_config_empty_string_handling() {
        let toml_str = r#"
            week_start_day = ""
            data_directory = ""
            template_file = ""
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.week_start_day, Some("".to_string()));
        assert_eq!(config.data_directory, Some("".to_string()));
        assert_eq!(config.template_file, Some("".to_string()));

        // Empty strings should still be returned as Some("")
        assert_eq!(config.get_week_start_day(), ""); // Empty string, not default
        assert_eq!(config.get_data_directory(), Some(""));
        assert_eq!(config.get_template_file(), Some(""));
    }

    #[test]
    fn test_config_invalid_toml_handling() {
        let invalid_toml = r#"
            week_start_day = Monday  // Missing quotes
            data_directory = /path/without/quotes
        "#;

        let result: Result<Config, _> = toml::from_str(invalid_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_with_extra_fields() {
        let toml_str = r#"
            week_start_day = "Tuesday"
            data_directory = "/custom/path"
            template_file = "/custom/template.md"
            unknown_field = "should be ignored"
            another_field = 42
        "#;

        // TOML parsing should ignore unknown fields by default with serde
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.week_start_day, Some("Tuesday".to_string()));
        assert_eq!(config.data_directory, Some("/custom/path".to_string()));
        assert_eq!(
            config.template_file,
            Some("/custom/template.md".to_string())
        );
    }

    #[test]
    fn test_week_start_day_fallback_behavior() {
        // Test that when week_start_day is None, we get the default
        let config = Config {
            week_start_day: None,
            data_directory: Some("/data".to_string()),
            template_file: Some("/template.md".to_string()),
        };

        assert_eq!(config.get_week_start_day(), "Saturday");

        // Test with empty string (should return empty string, not default)
        let config = Config {
            week_start_day: Some("".to_string()),
            data_directory: None,
            template_file: None,
        };

        assert_eq!(config.get_week_start_day(), "");
    }

    #[test]
    fn test_config_builder_pattern_usage() {
        // Test that we can use with_args_applied in a builder-like pattern
        let config = Config {
            week_start_day: Some("Monday".to_string()),
            data_directory: None,
            template_file: None,
        }
        .with_args_applied(
            None, // Keep Monday
            Some("/builder/data".to_string()),
            None,
        )
        .with_args_applied(
            Some("Friday".to_string()), // Override to Friday
            None,                       // Keep /builder/data
            Some("/builder/template.md".to_string()),
        );

        assert_eq!(config.get_week_start_day(), "Friday");
        assert_eq!(config.get_data_directory(), Some("/builder/data"));
        assert_eq!(config.get_template_file(), Some("/builder/template.md"));
    }
}
