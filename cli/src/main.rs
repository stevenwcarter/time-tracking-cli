use anyhow::{Context, Result};
use time::Weekday;
use time_tracking_cli::{
    Config, display::show_single_day_stdin, logging::init_tracing, parse_weekday, show_single_day,
    show_weekly_summary,
};
use tokio::task::JoinSet;
use tracing::error;

#[tokio::main]
async fn main() -> Result<()> {
    main_impl().await
}

/// The mode flags `--stdin` silently drops, in the order they are documented.
///
/// `main_impl` answers `--stdin` and returns before `serve`/`week`/`tui` are
/// ever consulted, so `ttcli --stdin --serve --port 3000` started no server
/// and printed nothing to say so. `--noedit` is deliberately absent: stdin
/// mode launches no editor at all, so its intent is satisfied rather than
/// ignored, and naming it would alarm anyone passing it as a safety habit.
fn ignored_stdin_flags(config: &Config) -> Vec<&'static str> {
    let mut ignored = Vec::new();
    if config.serve == Some(true) {
        ignored.push("--serve");
    }
    if config.week {
        ignored.push("--week");
    }
    #[cfg(feature = "tui")]
    if config.tui == Some(true) {
        ignored.push("--tui");
    }
    ignored
}

async fn main_impl() -> Result<()> {
    // The guard must be held for the process lifetime to ensure logs flush on shutdown.
    let _tracing_guard = init_tracing()
        .await
        .context("Coult not initialize tracing")?;
    // Load configuration and apply CLI argument overrides.
    // `try_get`, not `get`: a mistyped `--date` used to be swallowed and
    // replaced with today, exiting 0 with a report for the wrong day. It is
    // now a load error, and it should reach the user as a message and a
    // non-zero exit rather than as a panic out of `Config::get`'s `.expect`.
    let config = Config::try_get()?;

    if config.stdin {
        let ignored = ignored_stdin_flags(config);
        if !ignored.is_empty() {
            let message = format!(
                "--stdin takes precedence; these flags were ignored: {}",
                ignored.join(", ")
            );
            // Logged for durability, and also printed to stderr: the default
            // `RUST_LOG` records only `ERROR` events, and even those go to
            // `~/.local/share/time-tracking-cli/log.txt`, not the terminal —
            // so `tracing::warn!` alone reaches nobody. stdin mode writes its
            // report to stdout, so that stream stays reserved for the
            // caller; stderr is a separate stream and no TUI can be running
            // on this path (this function returns before any TUI spawn), so
            // there is no alternate-screen hazard in writing to it here.
            tracing::warn!("{message}");
            eprintln!("{message}");
        }

        let formatter = config.get_formatter();
        show_single_day_stdin(formatter.as_ref())
            .await
            .context("generating report from stdin")?;

        return Ok(());
    }

    let week_start_weekday =
        parse_weekday(config.get_week_start_day()).context("Could not parse weekday")?;

    let mut set: JoinSet<()> = JoinSet::new();

    #[cfg(all(feature = "webapp", feature = "tui"))]
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    #[cfg(all(feature = "webapp", not(feature = "tui")))]
    let (_tx, rx) = tokio::sync::oneshot::channel::<()>();

    // Handle serve mode
    #[cfg(feature = "webapp")]
    let webserver_running = spawn_webserver_if_configured(config, &mut set, rx);

    #[cfg(not(feature = "webapp"))]
    let webserver_running = false;

    #[cfg(feature = "tui")]
    if let Some(true) = config.tui {
        use tracing::info;

        info!("🚀 Starting Time Tracking TUI...");
        set.spawn(async move {
            if let Err(e) = time_tracking_cli::tui::tui().await {
                error!("Error running TUI: {}", e);
                eprintln!("Error running TUI: {}", e);
            }
            #[cfg(feature = "webapp")]
            let _ = tx.send(());
        });
    } else {
        show_report(config, week_start_weekday).await?;
    }

    #[cfg(not(feature = "tui"))]
    show_report(config, week_start_weekday).await?;

    wait_for_background_tasks(set, webserver_running).await
}

/// Print the requested report: the week containing `config.date`, or that
/// single day. The two call sites (`tui` feature compiled in but not
/// running the TUI, and `tui` compiled out entirely) differ only in
/// whether the `tui` feature is enabled at compile time.
async fn show_report(config: &Config, week_start_weekday: Weekday) -> Result<()> {
    let formatter = config.get_formatter();
    if config.week {
        show_weekly_summary(&config.date, week_start_weekday, formatter.as_ref()).await?;
    } else {
        show_single_day(&config.date, formatter.as_ref(), config.noedit).await?;
    }
    Ok(())
}

/// Spawn the web server task if `--serve` and `--port` are both set,
/// returning whether it was started.
#[cfg(feature = "webapp")]
fn spawn_webserver_if_configured(
    config: &Config,
    set: &mut JoinSet<()>,
    rx: tokio::sync::oneshot::Receiver<()>,
) -> bool {
    use tracing::info;

    // Whether the TUI will run must be read from `config` before it is
    // cloned and moved into the spawned task below. Stderr is safe to write
    // to only when the TUI is not running: `--serve` and `--tui` are
    // independent flags with no `conflicts_with`, so this task can be
    // running while the TUI owns the alternate screen and raw mode, and
    // ratatui's diff renderer does not know a region something else wrote to
    // has changed. With no TUI running there is no such hazard.
    #[cfg(feature = "tui")]
    let tui_will_run = config.tui == Some(true);
    #[cfg(not(feature = "tui"))]
    let tui_will_run = false;

    let mut running = false;
    if let Some(true) = config.serve
        && let Some(port) = config.port
    {
        running = true;
        info!("🚀 Starting Time Tracking Web Server...");
        let config = config.clone();
        set.spawn(async move {
            if let Err(e) = time_tracking_cli::web::run_server(port, config, rx).await {
                // Always logged to the file, the way every other task
                // failure in this file is.
                error!("Error running web server: {}", e);
                // Also surfaced on stderr when it is safe to (see above):
                // the default `RUST_LOG` records only `ERROR` events, and
                // even those go to `~/.local/share/time-tracking-cli/log.txt`,
                // not the terminal, so `tracing::error!` alone would leave a
                // failed `--serve` completely invisible.
                if !tui_will_run {
                    eprintln!("Error running web server: {}", e);
                }
            }
        });
    }
    running
}

/// Wait for any spawned background tasks (webserver and/or TUI) to finish,
/// logging individual task failures rather than propagating them.
async fn wait_for_background_tasks(mut set: JoinSet<()>, webserver_running: bool) -> Result<()> {
    if set.is_empty() {
        return Ok(());
    }

    if webserver_running {
        // `info!`, not `println!`: this fires on every combined
        // `--serve --tui` launch, where writing to stdout races the TUI's
        // own `ratatui::init()` alternate-screen entry. Feedback that must
        // reach a user while the TUI is up belongs on its status line, the
        // way `LoadFailed` and the clipboard failures already do.
        tracing::info!("Background tasks are running; press ctrl-c to quit");
    }
    while let Some(res) = set.join_next().await {
        if let Err(e) = res {
            error!("Task failed: {}", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // `Config` already arrives via the file's own
    // `use time_tracking_cli::{Config, ...}`; importing it again here is an
    // E0252 duplicate-import error.
    use super::*;

    fn stdin_config() -> Config {
        Config {
            stdin: true,
            ..Config::default()
        }
    }

    #[test]
    fn a_plain_stdin_run_reports_nothing_ignored() {
        assert!(ignored_stdin_flags(&stdin_config()).is_empty());
    }

    #[test]
    fn stdin_names_the_mode_flags_it_drops() {
        // `main_impl` returns straight after `show_single_day_stdin`, before
        // serve/week/tui are ever consulted, so
        // `ttcli --stdin --serve --port 3000` started no server and said so
        // nowhere.
        let config = Config {
            serve: Some(true),
            week: true,
            ..stdin_config()
        };
        let ignored = ignored_stdin_flags(&config);
        assert!(ignored.contains(&"--serve"), "{ignored:?}");
        assert!(ignored.contains(&"--week"), "{ignored:?}");
    }

    #[cfg(feature = "tui")]
    #[test]
    fn stdin_names_a_dropped_tui_flag() {
        let config = Config {
            tui: Some(true),
            ..stdin_config()
        };
        assert_eq!(ignored_stdin_flags(&config), vec!["--tui"]);
    }

    #[test]
    fn flags_that_are_off_are_not_reported() {
        let config = Config {
            serve: Some(false),
            week: false,
            ..stdin_config()
        };
        assert!(ignored_stdin_flags(&config).is_empty());
    }
}
