#[cfg(feature = "cli")]
use clap::{Parser, ValueEnum};

use anyhow::{Context, Result};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use time::{Date, OffsetDateTime};

use crate::file_utils::get_time_tracking_dir_with_override;
use crate::{
    DefaultDisplayFormatter, DisplayFormatter, MarkdownDisplayFormatter, PlainDisplayFormatter,
};

#[allow(unused_imports)]
use crate::DATE_FORMAT;
#[allow(unused_imports)]
use tracing::error;

static CONFIG: OnceLock<Config> = OnceLock::new();
static CONFIG_LOAD_ERR: OnceLock<String> = OnceLock::new();

#[cfg(feature = "cli")]
#[derive(ValueEnum, Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Formatter {
    Default,
    Markdown,
    Plain,
}
#[cfg(not(feature = "cli"))]
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Formatter {
    Default,
    Markdown,
    Plain,
}

impl From<Formatter> for String {
    fn from(value: Formatter) -> Self {
        match value {
            Formatter::Default => "default",
            Formatter::Markdown => "markdown",
            Formatter::Plain => "plain",
        }
        .to_owned()
    }
}

impl Formatter {
    /// The concrete [`DisplayFormatter`] this variant selects.
    ///
    /// The one mapping from a configured formatter to its implementation,
    /// shared by [`Config::get_formatter`] and the TUI's yank commands so the
    /// two render the same setting the same way rather than drifting onto a
    /// second copy of this match.
    pub fn build(&self) -> Box<dyn DisplayFormatter> {
        match self {
            Formatter::Default => Box::new(DefaultDisplayFormatter),
            Formatter::Markdown => Box::new(MarkdownDisplayFormatter),
            Formatter::Plain => Box::new(PlainDisplayFormatter),
        }
    }
}

/// Time tracking CLI utility
#[cfg(feature = "cli")]
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Date in YYYY-MM-DD format (defaults to today)
    #[arg(short, long)]
    date: Option<String>,

    /// Read from stdin and generate a report with the specified formatter
    #[arg(long)]
    stdin: bool,

    /// Date as a positional argument in YYYY-MM-DD format
    #[arg(value_name = "DATE")]
    positional_date: Option<String>,

    /// Show weekly summary for the week containing the specified date
    #[arg(short, long)]
    week: bool,

    /// Day of the week to start the week (Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday)
    #[arg(long, value_name = "DAY")]
    week_start_day: Option<String>,

    /// Directory where time tracking files are stored
    #[arg(long, value_name = "DIR")]
    data_directory: Option<String>,

    /// Path to a template file to use when creating new time tracking files
    #[arg(long, value_name = "FILE")]
    template_file: Option<String>,

    /// Output formatter type (default, plain, markdown)
    #[arg(long, value_name = "FORMAT", value_enum)]
    formatter: Option<Formatter>,

    /// Skip launching the editor, just display the summary
    #[arg(long)]
    noedit: bool,

    /// Launch web server mode
    #[cfg(feature = "webapp")]
    #[arg(long)]
    serve: bool,

    #[cfg(feature = "tui")]
    #[arg(long)]
    tui: bool,

    /// Port for web server (default: 3000)
    #[cfg(feature = "webapp")]
    #[arg(long)]
    port: Option<u16>,
}

fn today_date() -> Date {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .date()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Formatter to use ("plain", "markdown", "default")
    pub formatter: Option<Formatter>,
    /// Day of the week to start the week (Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday)
    pub week_start_day: Option<String>,
    /// Directory where time tracking files are stored (defaults to ~/.time-tracking)
    pub data_directory: Option<String>,
    /// Path to a template file to use when creating new time tracking files
    pub template_file: Option<String>,
    /// Optional prefix to search for before beginning parsing time entries (e.g. ```timetracking)
    pub prefix: Option<String>,
    /// Optional suffix to search for, after which stop parsing time entries (e.g. ``` )
    pub suffix: Option<String>,
    /// TUI colour palette preset: "dark", "light", or "none". An unset or
    /// unrecognised value falls back to "dark".
    pub theme: Option<String>,
    /// Hours of tracked time that count as a full day. Drives the TUI
    /// weekly bar chart's y-axis ceiling and goal line. Defaults to 8.0.
    pub daily_target_hours: Option<f64>,
    /// Whether we should read from stdin or not
    #[serde(skip)]
    pub stdin: bool,
    /// Whether to launch the web server
    pub serve: Option<bool>,
    /// Effective date
    #[serde(skip, default = "today_date")]
    pub date: Date,

    /// Whether the tui is enabled or not
    #[cfg(feature = "tui")]
    pub tui: Option<bool>,

    #[cfg(feature = "webapp")]
    #[serde(default = "default_port")]
    pub port: Option<u16>,

    #[serde(skip)]
    pub noedit: bool,

    #[serde(skip)]
    pub week: bool,
}

#[cfg(feature = "webapp")]
const fn default_port() -> Option<u16> {
    Some(3000)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            formatter: Some(Formatter::Default),
            week_start_day: Some("Saturday".to_string()),
            data_directory: Some(
                get_time_tracking_dir_with_override(None)
                    .unwrap()
                    .display()
                    .to_string(),
            ),
            template_file: None,
            prefix: None,
            suffix: None,
            theme: Some("dark".to_string()),
            daily_target_hours: Some(8.0),
            stdin: false,
            serve: Some(false),
            date: today_date(),
            #[cfg(feature = "tui")]
            tui: None,
            #[cfg(feature = "webapp")]
            port: Some(3000),
            noedit: false,
            week: false,
        }
    }
}

impl Config {
    fn try_init(use_args: bool) -> anyhow::Result<&'static Config> {
        if let Some(config) = CONFIG.get() {
            return Ok(config);
        }
        if let Some(err) = CONFIG_LOAD_ERR.get() {
            return Err(anyhow::anyhow!("{}", err));
        }
        match Config::load(use_args) {
            Ok(config) => {
                let _ = CONFIG.set(config);
                Ok(CONFIG.get().unwrap())
            }
            Err(e) => {
                let err_str = e.to_string();
                let _ = CONFIG_LOAD_ERR.set(err_str);
                Err(e)
            }
        }
    }

    fn init(use_args: bool) -> &'static Config {
        CONFIG.get_or_init(|| Config::load(use_args).expect("Could not load configuration"))
    }

    pub fn get_no_args() -> &'static Config {
        Self::init(false)
    }

    pub fn try_get_no_args() -> anyhow::Result<&'static Config> {
        Self::try_init(false)
    }

    pub fn get() -> &'static Config {
        Self::init(true)
    }

    #[cfg(feature = "cli")]
    fn load(use_args: bool) -> Result<Config> {
        let args = if use_args {
            Args::parse()
        } else {
            Args {
                date: None,
                stdin: false,
                positional_date: None,
                week: false,
                week_start_day: None,
                data_directory: None,
                template_file: None,
                formatter: None,
                noedit: false,
                #[cfg(feature = "webapp")]
                serve: false,
                #[cfg(feature = "tui")]
                tui: false,
                #[cfg(feature = "webapp")]
                port: None,
            }
        };

        let config_path = get_config_path()?;

        let mut config = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            config
        } else {
            // Create default config file
            let default_config = Config::default();
            fs::create_dir_all(
                config_path
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("config path has no parent directory"))?,
            )?;
            let toml_content = toml::to_string_pretty(&default_config)?;
            fs::write(&config_path, toml_content)?;
            let mut file = OpenOptions::new().append(true).open(&config_path)?;
            write_config_comments(&mut file)?;
            default_config
        };

        if let Some(week_start_day) = args.week_start_day {
            config.week_start_day = Some(week_start_day);
        }
        if let Some(data_directory) = args.data_directory {
            config.data_directory = Some(data_directory);
        }
        if let Some(template_file) = args.template_file {
            config.template_file = Some(template_file);
        }
        if let Some(formatter) = args.formatter {
            config.formatter = Some(formatter);
        }
        if args.stdin {
            config.stdin = true;
        }

        #[cfg(feature = "webapp")]
        if args.serve {
            config.serve = Some(true);
        }
        if args.week {
            config.week = true;
        }

        {
            let date_str = args.date.or(args.positional_date);

            let date = match date_str {
                Some(date_str) => {
                    // Parse the provided date

                    use interim::{Dialect, parse_date_string};
                    let now =
                        OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
                    let date_time = parse_date_string(&date_str, now, Dialect::Us);
                    match date_time {
                        Ok(date_time) => date_time.date(),
                        Err(e) => {
                            eprintln!("Could not parse provided date: '{date_str}' - {:?}", e);
                            today_date()
                        }
                    }
                }
                None => {
                    // Use today's date
                    today_date()
                }
            };
            config.date = date;
        }

        #[cfg(feature = "tui")]
        if args.tui {
            config.tui = Some(true);
        }

        if args.noedit {
            config.noedit = true;
        }

        #[cfg(feature = "webapp")]
        if let Some(port) = args.port {
            config.port = Some(port);
        }

        Ok(config)
    }
    #[cfg(not(feature = "cli"))]
    fn load(_use_args: bool) -> Result<Config> {
        let config_path = get_config_path()?;

        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            // Create default config file
            let default_config = Config::default();
            fs::create_dir_all(
                config_path
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("config path has no parent directory"))?,
            )?;
            let toml_content = toml::to_string_pretty(&default_config)?;
            fs::write(&config_path, toml_content)?;
            let mut file = OpenOptions::new().append(true).open(&config_path)?;
            write_config_comments(&mut file)?;
            Ok(default_config)
        }
    }

    /// Get the week start day with fallback to default
    pub fn get_week_start_day(&self) -> &str {
        self.week_start_day.as_deref().unwrap_or("Saturday")
    }

    /// Get the data directory as Option<&str>
    pub fn get_data_directory(&self) -> Option<&str> {
        self.data_directory.as_deref()
    }

    /// Get the template file as Option<&str>
    pub fn get_template_file(&self) -> Option<&str> {
        self.template_file.as_deref()
    }

    pub fn get_configured_formatter(&self) -> Option<&Formatter> {
        self.formatter.as_ref()
    }

    /// Get the prefix as Option<&str>
    pub fn get_prefix(&self) -> Option<&str> {
        self.prefix.as_deref()
    }

    /// Get the suffix as Option<&str>
    pub fn get_suffix(&self) -> Option<&str> {
        self.suffix.as_deref()
    }
    /// The formatter this config selects, built via [`Formatter::build`].
    pub fn get_formatter(&self) -> Box<dyn DisplayFormatter> {
        self.get_configured_formatter()
            .unwrap_or(&Formatter::Default)
            .build()
    }
}

fn write_config_comments(file: &mut impl Write) -> Result<()> {
    file.write_all(b"\n# Optional template file which will be used to create each new day note\n")?;
    file.write_all(b"#template_file = ~/.time-tracking/template.md\n")?;
    file.write_all(
        b"\n# Optional prefix to look for in a file before starting to parse time notes\n",
    )?;
    file.write_all(b"\n# Useful if you want to keep other notes in the same file\n")?;
    file.write_all(b"#prefix = \"```timetracking\"\n")?;
    file.write_all(b"\n# Corresponding closing tag to stop parsing, if you want notes after this. Must specify prefix too\n")?;
    file.write_all(b"#suffix = \"```\"\n")?;
    file.write_all(b"\n# TUI theme preset: \"dark\", \"light\", or \"none\".\n")?;
    file.write_all(b"# \"none\" emits no colors so your terminal palette shows through.\n")?;
    file.write_all(b"# NO_COLOR in the environment forces \"none\".\n")?;
    file.write_all(b"#theme = \"dark\"\n")?;
    file.write_all(b"\n# Hours of tracked time that count as a full day, for the TUI weekly bar chart's y-axis ceiling and goal line.\n")?;
    file.write_all(b"#daily_target_hours = 8.0\n")?;
    Ok(())
}

fn get_config_path() -> Result<PathBuf> {
    if let Some(config_dir) = dirs::config_dir() {
        Ok(config_dir.join("time-tracking-cli").join("config.toml"))
    } else {
        // Fallback to home directory
        let home = home_dir().context("Could not determine home directory")?;
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
            formatter: Some(Formatter::Default),
            week_start_day: Some("Monday".to_string()),
            data_directory: Some("/test/data".to_string()),
            template_file: Some("/test/template.md".to_string()),
            prefix: None,
            suffix: None,
            ..Config::default()
        }
    }

    fn create_empty_config() -> Config {
        Config {
            formatter: Some(Formatter::Default),
            week_start_day: None,
            data_directory: None,
            template_file: None,
            prefix: None,
            suffix: None,
            ..Config::default()
        }
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.week_start_day, Some("Saturday".to_string()));
        assert!(config.data_directory.unwrap().ends_with("/.time-tracking"));
        assert_eq!(config.template_file, None);
        assert_eq!(config.daily_target_hours, Some(8.0));
    }

    #[test]
    fn test_daily_target_hours_roundtrip() {
        let config = Config {
            daily_target_hours: Some(6.5),
            ..Config::default()
        };
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("daily_target_hours = 6.5"));

        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.daily_target_hours, Some(6.5));
    }

    #[test]
    fn test_daily_target_hours_missing_key_deserializes_to_none() {
        let config: Config = toml::from_str(r#"week_start_day = "Tuesday""#).unwrap();
        assert_eq!(config.daily_target_hours, None);
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

        let result: Result<Config> = toml::from_str(invalid_toml).context("reading toml");
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
    fn test_config_with_lowercase_formatter() {
        let toml_str = r#"
            formatter = "plain"
        "#;

        // TOML parsing should ignore unknown fields by default with serde
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.formatter, Some(Formatter::Plain));
    }

    #[test]
    fn test_week_start_day_fallback_behavior() {
        // Test that when week_start_day is None, we get the default
        let config = Config {
            week_start_day: None,
            data_directory: Some("/data".to_string()),
            template_file: Some("/template.md".to_string()),
            ..Config::default()
        };

        assert_eq!(config.get_week_start_day(), "Saturday");

        // Test with empty string (should return empty string, not default)
        let config = Config {
            week_start_day: Some("".to_string()),
            data_directory: None,
            template_file: None,
            ..Config::default()
        };

        assert_eq!(config.get_week_start_day(), "");
    }
}
