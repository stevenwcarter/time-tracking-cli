use anyhow::Result;
use clap::Parser;
use time::{Date, OffsetDateTime};
use time_tracking_cli::{
    Config, DATE_FORMAT, DefaultDisplayFormatter, DisplayFormatter, MarkdownDisplayFormatter,
    PlainDisplayFormatter, parse_weekday, show_single_day, show_weekly_summary,
};

#[cfg(feature = "webapp")]
use time_tracking_cli::run_server;

/// Time tracking CLI utility
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Date in YYYY-MM-DD format (defaults to today)
    #[arg(short, long)]
    date: Option<String>,

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
    #[arg(long, value_name = "FORMAT", default_value = "default")]
    formatter: String,

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
    #[arg(long, default_value = "3000")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    main_impl().await
}

async fn main_impl() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Load configuration and apply CLI argument overrides
    let config = Config::load()?.with_args_applied(
        args.week_start_day,
        args.data_directory,
        args.template_file,
    );

    let week_start_weekday = parse_weekday(&config.get_week_start_day())?;

    // Handle serve mode
    #[cfg(feature = "webapp")]
    if args.serve {
        println!("🚀 Starting Time Tracking Web Server...");
        return run_server(args.port, config).await;
    }

    // Determine the date to use - prioritize flag over positional, then default to today
    let date_str = args.date.or(args.positional_date);

    let date = match date_str {
        Some(date_str) => {
            // Parse the provided date
            Date::parse(&date_str, DATE_FORMAT)
                .map_err(|_| "Invalid date format. Please use YYYY-MM-DD")?
        }
        None => {
            // Use today's date
            OffsetDateTime::now_local().unwrap().date()
        }
    };

    let formatter = parse_formatter(&args.formatter);

    #[cfg(feature = "tui")]
    if args.tui {
        use tracing::info;

        info!("🚀 Starting Time Tracking TUI...");
        let _ = time_tracking_cli::tui::tui(&config, date, formatter).await;
        return Ok(());
    }

    // Select the appropriate formatter

    if args.week {
        // Show weekly summary
        show_weekly_summary(&date, week_start_weekday, formatter.as_ref(), &config).await?;
    } else {
        // Show single day (existing functionality)
        show_single_day(&date, formatter.as_ref(), &config, args.noedit).await?;
    }

    Ok(())
}

fn parse_formatter(formatter: &str) -> Box<dyn DisplayFormatter> {
    match formatter {
        "plain" => Box::new(PlainDisplayFormatter),
        "markdown" => Box::new(MarkdownDisplayFormatter),
        _ => Box::new(DefaultDisplayFormatter),
    }
}
