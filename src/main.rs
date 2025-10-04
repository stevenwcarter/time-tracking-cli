use chrono::{Local, NaiveDate, Datelike, Duration, Weekday};
use clap::Parser;
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use time_tracking_parser::{parse_time_tracking_data, Time};

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    /// Day of the week to start the week (Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday)
    week_start_day: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            week_start_day: Some("Saturday".to_string()),
        }
    }
}

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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Load configuration
    let config = load_config()?;
    
    // Determine the week start day (priority: CLI arg > config file > default)
    let week_start_day = args.week_start_day
        .or(config.week_start_day)
        .unwrap_or_else(|| "Saturday".to_string());
    
    let week_start_weekday = parse_weekday(&week_start_day)?;
    
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
    
    if args.week {
        // Show weekly summary
        show_weekly_summary(&date, week_start_weekday)?;
    } else {
        // Show single day (existing functionality)
        show_single_day(&date)?;
    }
    
    Ok(())
}

fn show_single_day(date: &NaiveDate) -> Result<(), Box<dyn std::error::Error>> {
    // Create the time tracking directory
    let time_tracking_dir = get_time_tracking_dir()?;
    fs::create_dir_all(&time_tracking_dir)?;
    
    // Create the filename for the date
    let filename = format!("{}.md", date.format("%Y-%m-%d"));
    let file_path = time_tracking_dir.join(&filename);
    
    // Create the file if it doesn't exist
    if !file_path.exists() {
        fs::write(&file_path, create_template_content(date))?;
        println!("Created new time tracking file: {}", file_path.display());
    } else {
        println!("Opening existing time tracking file: {}", file_path.display());
    }
    
    // Open the file in the default editor
    open_in_editor(&file_path)?;
    
    // After editor closes, parse and display the results
    let content = fs::read_to_string(&file_path)?;
    display_time_tracking_results(&content);
    
    Ok(())
}

fn show_weekly_summary(date: &NaiveDate, week_start_day: Weekday) -> Result<(), Box<dyn std::error::Error>> {
    let week_dates = get_week_dates(date, week_start_day);
    let time_tracking_dir = get_time_tracking_dir()?;
    
    println!("\n{}", "=".repeat(80));
    println!("WEEKLY TIME TRACKING SUMMARY");
    println!("Week of {} to {}", 
        format_day_with_date(&week_dates[0]), 
        format_day_with_date(&week_dates[6])
    );
    println!("{}", "=".repeat(80));
    
    let mut total_week_minutes = 0;
    let mut total_week_dead_minutes = 0;
    let mut week_warnings = Vec::new();
    let mut week_projects: std::collections::HashMap<String, (u32, Vec<String>)> = std::collections::HashMap::new();
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
                if !warning.contains("Error parsing time range '#'") { // Skip markdown header warnings
                    week_warnings.push(format!("{}: {}", format_day_with_date(day_date), warning));
                }
            }
            
            // Aggregate projects
            for project in &data.projects {
                let entry = week_projects.entry(project.name.clone()).or_insert((0, Vec::new()));
                entry.0 += project.total_minutes;
                for note in &project.notes {
                    entry.1.push(format!("{}: {}", format_day_with_date(day_date), note));
                }
            }
            
            daily_data.push((*day_date, content, Some(data)));
        } else {
            daily_data.push((*day_date, String::new(), None));
        }
    }
    
    // Display weekly summary at the top
    println!("\n📊 WEEKLY TOTALS");
    println!("{}", "-".repeat(40));
    
    println!("⏱️  Total Working Time: {} ({} hrs)", 
        Time::format_duration_minutes(total_week_minutes),
        Time::format_duration_decimal(total_week_minutes)
    );
    
    if total_week_dead_minutes > 0 {
        println!("⏸️  Total Dead Time: {} ({} hrs)", 
            Time::format_duration_minutes(total_week_dead_minutes),
            Time::format_duration_decimal(total_week_dead_minutes)
        );
    } else {
        println!("⏸️  Total Dead Time: ✅ None");
    }
    
    if !week_warnings.is_empty() {
        println!("\n⚠️  WEEKLY WARNINGS");
        for warning in &week_warnings {
            println!("  ⚠ {}", warning);
        }
    }
    
    if !week_projects.is_empty() {
        println!("\n📋 WEEKLY PROJECTS SUMMARY");
        let mut projects: Vec<_> = week_projects.iter().collect();
        projects.sort_by(|a, b| b.1.0.cmp(&a.1.0)); // Sort by total minutes descending
        
        for (project_name, (total_minutes, notes)) in projects {
            println!("  📌 {} - {} ({} hrs)",
                project_name,
                Time::format_duration_minutes(*total_minutes),
                Time::format_duration_decimal(*total_minutes)
            );
            
            if !notes.is_empty() {
                for note in notes {
                    println!("     • {}", note);
                }
            }
        }
    }
    
    // Now display detailed daily summaries
    println!("\n{}", "=".repeat(80));
    println!("DAILY BREAKDOWNS");
    println!("{}", "=".repeat(80));
    
    for (day_date, content, data_opt) in daily_data {
        println!("\n📅 {}", format_day_with_date(&day_date));
        println!("{}", "=".repeat(60));
        
        if let Some(data) = data_opt {
            if data.total_minutes > 0 {
                display_time_tracking_results_with_indent(&content, "  ");
            } else {
                println!("  💤 No time tracking data found");
            }
        } else {
            println!("  📄 No time tracking file found");
        }
    }
    
    println!("\n{}", "=".repeat(80));
    
    Ok(())
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
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

fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(config_dir) = dirs::config_dir() {
        Ok(config_dir.join("time-tracking-rs").join("config.toml"))
    } else {
        // Fallback to home directory
        let home = home_dir().ok_or("Could not determine home directory")?;
        Ok(home.join(".config").join("time-tracking-rs").join("config.toml"))
    }
}

fn parse_weekday(day_str: &str) -> Result<Weekday, Box<dyn std::error::Error>> {
    match day_str.to_lowercase().as_str() {
        "monday" | "mon" => Ok(Weekday::Mon),
        "tuesday" | "tue" => Ok(Weekday::Tue),
        "wednesday" | "wed" => Ok(Weekday::Wed),
        "thursday" | "thu" => Ok(Weekday::Thu),
        "friday" | "fri" => Ok(Weekday::Fri),
        "saturday" | "sat" => Ok(Weekday::Sat),
        "sunday" | "sun" => Ok(Weekday::Sun),
        _ => Err(format!("Invalid weekday: '{}'. Valid options are: Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday", day_str).into()),
    }
}

fn get_week_dates(date: &NaiveDate, week_start_day: Weekday) -> Vec<NaiveDate> {
    // Calculate how many days to go back to reach the week start day
    let current_weekday = date.weekday();
    let days_since_week_start = (current_weekday.num_days_from_monday() as i32 
        - week_start_day.num_days_from_monday() as i32 + 7) % 7;
    
    let week_start = *date - Duration::days(days_since_week_start as i64);
    
    // Generate all 7 days of the week
    (0..7)
        .map(|i| week_start + Duration::days(i))
        .collect()
}

fn format_day_with_date(date: &NaiveDate) -> String {
    let day_name = match date.weekday().num_days_from_monday() {
        0 => "Monday",
        1 => "Tuesday", 
        2 => "Wednesday",
        3 => "Thursday",
        4 => "Friday",
        5 => "Saturday",
        6 => "Sunday",
        _ => unreachable!(),
    };
    
    format!("{} {}", day_name, date.format("%Y-%m-%d"))
}

fn get_time_tracking_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(".time-tracking"))
}

fn create_template_content(date: &NaiveDate) -> String {
    format!(
        "# Time Tracking - {}\n\n",
        date.format("%Y-%m-%d")
    )
}

fn get_editor() -> String {
    env::var("EDITOR")
        .or_else(|_| env::var("VISUAL"))
        .unwrap_or_else(|_| {
            // Default editors by platform
            if cfg!(target_os = "macos") {
                "nano".to_string()
            } else if cfg!(target_os = "windows") {
                "notepad".to_string()
            } else {
                "nano".to_string()
            }
        })
}

fn open_in_editor(file_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let editor = get_editor();
    
    let mut command = Command::new(&editor);
    command.arg(file_path);
    
    // For some editors like vim/nano, we need to inherit stdio
    command.stdin(Stdio::inherit());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());
    
    let status = command.status()?;
    
    if !status.success() {
        return Err(format!("Editor '{}' exited with non-zero status", editor).into());
    }
    
    Ok(())
}

fn display_time_tracking_results(content: &str) {
    display_time_tracking_results_with_indent(content, "");
}

fn display_time_tracking_results_with_indent(content: &str, indent: &str) {
    let data = parse_time_tracking_data(content);
    
    // Display overview
    println!("{}📅 TIME OVERVIEW", indent);
    println!("{}Start Time: {}", indent, data.formatted_start_time());
    println!("{}End Time:   {}", indent, data.formatted_end_time());
    
    // Display total working time
    println!("{}⏱️  WORKING TIME", indent);
    println!("{}Total: {} ({} hours)", 
        indent,
        data.formatted_total_minutes(), 
        data.formatted_total_decimal()
    );
    
    // Display dead time
    println!("{}⏸️  DEAD TIME", indent);
    if data.dead_time_minutes == 0 {
        println!("{}✅ No dead time (gaps) found", indent);
    } else {
        let status_icon = if data.dead_time_minutes < 90 { "⚠️" } else { "❌" };
        println!("{}{} {} ({} hours)", 
            indent,
            status_icon,
            data.formatted_dead_time_minutes(), 
            data.formatted_dead_decimal()
        );
    }
    
    // Display warnings
    if !data.warnings.is_empty() {
        println!("{}⚠️  WARNINGS", indent);
        for warning in &data.warnings {
            println!("{}  ⚠ {}", indent, warning);
        }
    }
    
    // Display projects
    if !data.projects.is_empty() {
        println!("{}📋 PROJECTS", indent);
        for project in &data.projects {
            println!("{}  📌 {} - {} ({} hrs)",
                indent,
                project.name,
                Time::format_duration_minutes(project.total_minutes),
                Time::format_duration_decimal(project.total_minutes)
            );
            
            if !project.notes.is_empty() {
                for note in &project.notes {
                    println!("{}     • {}", indent, note);
                }
            }
        }
    } else {
        println!("{}📋 PROJECTS", indent);
        println!("{}  No projects found. Make sure to enter time tracking data in the format:", indent);
        println!("{}  11:45-12:15 project_code", indent);
        println!("{}  - Comment explaining what you did", indent);
    }
}
