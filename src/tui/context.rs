use anyhow::{Context, Result};
use std::path::PathBuf;
use time::Weekday;

use super::theme::Theme;
use crate::config::{Config, Formatter};
use crate::file_utils::get_time_tracking_dir_with_override;
use crate::time_utils::parse_weekday;

/// Hours of tracked time that count as a full day.
// Task 23 will read this from the configuration file instead.
const DEFAULT_DAILY_TARGET_HOURS: f64 = 8.0;

/// Everything the TUI needs from the environment, resolved once at start-up.
///
/// The widgets and [`App`](super::app::App) read this instead of the global
/// [`Config`] singleton. [`Config::get`] runs `Args::parse()` on first use, so
/// touching it from a test parses the test harness's own argv; taking the
/// context as a constructor argument keeps the TUI constructible in a test.
#[derive(Clone, Debug)]
pub struct TuiContext {
    /// First day of the week for the calendar and the weekly bar chart.
    pub week_start_day: Weekday,
    /// Directory the day files are read from.
    pub data_dir: PathBuf,
    /// Hours of tracked time that count as a full day.
    pub daily_target_hours: f64,
    /// Formatter used when the TUI hands content to the display layer.
    pub formatter: Formatter,
    /// The styles every widget draws with.
    pub theme: Theme,
}

impl TuiContext {
    /// Resolve a context from the loaded configuration.
    pub fn from_config(config: &Config) -> Result<Self> {
        Ok(Self {
            week_start_day: parse_weekday(config.get_week_start_day())
                .context("could not parse week start day")?,
            data_dir: get_time_tracking_dir_with_override(config.get_data_directory())?,
            daily_target_hours: DEFAULT_DAILY_TARGET_HOURS,
            formatter: config
                .get_configured_formatter()
                .cloned()
                .unwrap_or(Formatter::Default),
            // Task 22 resolves the theme from the config plus NO_COLOR/COLORTERM.
            theme: Theme::dark(),
        })
    }

    /// A deterministic context for tests: Saturday weeks, a scratch data
    /// directory, and a palette that emits no colour so rendered buffers can be
    /// compared as plain text.
    #[cfg(test)]
    pub fn for_test() -> Self {
        use std::env;

        Self {
            week_start_day: Weekday::Saturday,
            data_dir: env::temp_dir().join("ttcli-test"),
            daily_target_hours: DEFAULT_DAILY_TARGET_HOURS,
            formatter: Formatter::Plain,
            theme: Theme::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::App;

    // `App::new` builds an `EventHandler`, which spawns the crossterm poller,
    // so construction needs a runtime — but no longer a parsed `Config`.
    #[tokio::test]
    async fn app_can_be_constructed_without_parsing_argv() {
        // Regression: App::new() used to reach for the Config singleton, whose
        // first use runs Args::parse() against the test harness's own argv.
        let ctx = TuiContext::for_test();
        let app = App::new(ctx);
        assert!(app.running);
        assert!(!app.loading);
    }

    #[test]
    fn for_test_context_uses_saturday_and_no_color() {
        let ctx = TuiContext::for_test();
        assert_eq!(ctx.week_start_day, Weekday::Saturday);
        assert_eq!(ctx.theme.populated_date.fg, None);
        assert_eq!(ctx.formatter, Formatter::Plain);
        assert_eq!(ctx.daily_target_hours, 8.0);
        assert!(ctx.data_dir.ends_with("ttcli-test"));
    }
}
