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
    /// directory of its own, and a palette that emits no colour so rendered
    /// buffers can be compared as plain text.
    ///
    /// The directory is unique per call and is *not* created, so two tests
    /// never see each other's day files. A test that needs the directory to
    /// exist should create it (or override `data_dir` with a `tempfile`
    /// handle it keeps alive for the duration of the test).
    #[cfg(test)]
    pub fn for_test() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::{env, process};

        // The counter separates contexts within one test binary; the pid
        // separates concurrent `cargo test` runs.
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        Self {
            week_start_day: Weekday::Saturday,
            data_dir: env::temp_dir().join(format!("ttcli-test-{}-{id}", process::id())),
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
        assert!(ctx.data_dir.starts_with(std::env::temp_dir()));
    }

    #[test]
    fn app_can_be_constructed_outside_a_tokio_runtime() {
        // `EventHandler::new` only wires up channels; the crossterm poller is
        // spawned by `EventHandler::start`, which `App::run` calls. Before that
        // split this panicked with "there is no reactor running", and every
        // later async test would have spawned a tty reader flooding a channel
        // nobody drains.
        let app = App::new(TuiContext::for_test());
        assert!(app.running);
    }

    #[test]
    fn each_test_context_gets_its_own_data_dir() {
        assert_ne!(
            TuiContext::for_test().data_dir,
            TuiContext::for_test().data_dir
        );
    }
}
