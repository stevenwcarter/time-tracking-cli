use chrono::{Local, NaiveDate, Weekday};
use clap::Parser;
use std::fs;
use time_tracking_cli::{
    Config, DefaultDisplayFormatter, PlainDisplayFormatter, MarkdownDisplayFormatter, DisplayFormatter,
    create_template_content, format_day_with_date, get_time_tracking_dir_with_override, get_week_dates,
    open_in_editor, parse_weekday,
};

#[cfg(feature = "webapp")]
use time_tracking_cli::run_server;
use time_tracking_parser::parse_time_tracking_data;

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

    /// Port for web server (default: 3000)
    #[cfg(feature = "webapp")]
    #[arg(long, default_value = "3000")]
    port: u16,
}

#[cfg(feature = "webapp")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    main_impl().await
}

#[cfg(not(feature = "webapp"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    futures::executor::block_on(main_impl())
}

async fn main_impl() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Load configuration
    let config = Config::load()?;

    // Determine the week start day (priority: CLI arg > config file > default)
    let week_start_day = args
        .week_start_day
        .or(config.week_start_day)
        .unwrap_or_else(|| "Saturday".to_string());

    let week_start_weekday = parse_weekday(&week_start_day)?;

    // Determine the data directory (priority: CLI arg > config file > default)
    let resolved_data_directory = args
        .data_directory
        .or(config.data_directory);
    let data_directory = resolved_data_directory.as_deref(); // Convert Option<String> to Option<&str>

    // Determine the template file (priority: CLI arg > config file > default)
    let resolved_template_file = args
        .template_file
        .or(config.template_file);
    let template_file = resolved_template_file.as_deref(); // Convert Option<String> to Option<&str>

    // Handle serve mode
    #[cfg(feature = "webapp")]
    if args.serve {
        println!("🚀 Starting Time Tracking Web Server...");
        return run_server(args.port, resolved_data_directory).await;
    }

    // Determine the date to use - prioritize flag over positional, then default to today
    let date_str = args.date.or(args.positional_date);

    let date = match date_str {
        Some(date_str) => {
            // Parse the provided date
            NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|_| "Invalid date format. Please use YYYY-MM-DD")?
        }
        None => {
            // Use today's date
            Local::now().date_naive()
        }
    };

    // Select the appropriate formatter
    let formatter: Box<dyn DisplayFormatter> = match args.formatter.as_str() {
        "plain" => Box::new(PlainDisplayFormatter),
        "markdown" => Box::new(MarkdownDisplayFormatter),
        _ => Box::new(DefaultDisplayFormatter), // default case
    };

    if args.week {
        // Show weekly summary
        show_weekly_summary(&date, week_start_weekday, formatter.as_ref(), data_directory)?;
    } else {
        // Show single day (existing functionality)
        show_single_day(&date, formatter.as_ref(), data_directory, args.noedit, template_file)?;
    }

    Ok(())
}

fn show_single_day(date: &NaiveDate, formatter: &dyn DisplayFormatter, data_directory: Option<&str>, noedit: bool, template_file: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    // Create the time tracking directory
    let time_tracking_dir = get_time_tracking_dir_with_override(data_directory)?;
    fs::create_dir_all(&time_tracking_dir)?;

    // Create the filename for the date
    let filename = format!("{}.md", date.format("%Y-%m-%d"));
    let file_path = time_tracking_dir.join(&filename);

    // Create the file if it doesn't exist
    if !file_path.exists() {
        let template_content = create_template_content(date, template_file)?;
        fs::write(&file_path, template_content)?;
        if !noedit {
            println!("Created new time tracking file: {}", file_path.display());
        }
    } else if !noedit {
        println!(
            "Opening existing time tracking file: {}",
            file_path.display()
        );
    }

    // Open the file in the default editor only if noedit is false
    if !noedit {
        open_in_editor(&file_path)?;
    }

    // Parse and display the results
    let content = fs::read_to_string(&file_path)?;
    formatter.display_day_summary(&content, "");

    Ok(())
}

fn show_weekly_summary(
    date: &NaiveDate,
    week_start_day: Weekday,
    formatter: &dyn DisplayFormatter,
    data_directory: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let week_dates = get_week_dates(date, week_start_day);
    let time_tracking_dir = get_time_tracking_dir_with_override(data_directory)?;

    formatter.display_weekly_header(
        &format_day_with_date(&week_dates[0]),
        &format_day_with_date(&week_dates[6]),
    );

    let mut total_week_minutes = 0;
    let mut total_week_dead_minutes = 0;
    let mut week_warnings = Vec::new();
    let mut week_projects: std::collections::HashMap<String, (u32, Vec<String>)> =
        std::collections::HashMap::new();
    let mut daily_data = Vec::new();

    // First pass: collect all data
    for day_date in &week_dates {
        let filename = format!("{}.md", day_date.format("%Y-%m-%d"));
        let file_path = time_tracking_dir.join(&filename);

        if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            let data = parse_time_tracking_data(&content);

            // Add to weekly totals
            total_week_minutes += data.total_minutes;
            total_week_dead_minutes += data.dead_time_minutes;

            // Collect warnings
            for warning in &data.warnings {
                if !warning.contains("Error parsing time range '#'") {
                    // Skip markdown header warnings
                    week_warnings.push(format!("{}: {}", format_day_with_date(day_date), warning));
                }
            }

            // Aggregate projects
            for project in &data.projects {
                let entry = week_projects
                    .entry(project.name.clone())
                    .or_insert((0, Vec::new()));
                entry.0 += project.total_minutes;
                for note in &project.notes {
                    entry
                        .1
                        .push(format!("{}: {}", format_day_with_date(day_date), note));
                }
            }

            daily_data.push((*day_date, content, Some(data)));
        } else {
            daily_data.push((*day_date, String::new(), None));
        }
    }

    // Display weekly summary at the top
    formatter.display_weekly_totals(total_week_minutes, total_week_dead_minutes);

    if !week_warnings.is_empty() {
        println!("\n⚠️  WEEKLY WARNINGS");
        for warning in &week_warnings {
            println!("  ⚠ {}", warning);
        }
    }

    if !week_projects.is_empty() {
        let mut projects: Vec<_> = week_projects.iter().collect();
        projects.sort_by(|a, b| b.1.0.cmp(&a.1.0)); // Sort by total minutes descending
        formatter.display_weekly_projects(&projects);
    }

    // Now display detailed daily summaries
    formatter.display_daily_breakdowns_header();

    for (day_date, content, data_opt) in daily_data {
        formatter.display_day_header(&format_day_with_date(&day_date));

        if let Some(data) = data_opt {
            if data.total_minutes > 0 {
                formatter.display_day_summary(&content, "  ");
            } else {
                formatter.display_no_data_found("  ");
            }
        } else {
            formatter.display_no_file_found("  ");
        }
    }

    println!("\n{}", "=".repeat(80));

    Ok(())
}
