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

async fn main_impl() -> Result<()> {
    // The guard must be held for the process lifetime to ensure logs flush on shutdown.
    let _tracing_guard = init_tracing()
        .await
        .context("Coult not initialize tracing")?;
    // Load configuration and apply CLI argument overrides.
    let config = Config::get();

    if config.stdin {
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

    let mut running = false;
    if let Some(true) = config.serve
        && let Some(port) = config.port
    {
        running = true;
        info!("🚀 Starting Time Tracking Web Server...");
        let config = config.clone();
        set.spawn(async move {
            if let Err(e) = time_tracking_cli::web::run_server(port, config, rx).await {
                error!("Error running web server: {}", e);
                eprintln!("Error running web server: {}", e);
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
        println!("Other jobs are running (webserver or tui), press ctrl-c to quit (webserver)");
    }
    while let Some(res) = set.join_next().await {
        if let Err(e) = res {
            error!("Task failed: {}", e);
        }
    }

    Ok(())
}
