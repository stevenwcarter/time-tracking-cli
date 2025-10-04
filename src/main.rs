use chrono::{Local, NaiveDate};
use clap::Parser;
use dirs::home_dir;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use time_tracking_parser::{parse_time_tracking_data, Time};

/// Time tracking CLI utility
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Date in YYYY-MM-DD format (defaults to today)
    #[arg(short, long)]
    date: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Determine the date to use
    let date = match args.date {
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
    
    // Create the time tracking directory
    let time_tracking_dir = get_time_tracking_dir()?;
    fs::create_dir_all(&time_tracking_dir)?;
    
    // Create the filename for the date
    let filename = format!("{}.md", date.format("%Y-%m-%d"));
    let file_path = time_tracking_dir.join(&filename);
    
    // Create the file if it doesn't exist
    if !file_path.exists() {
        fs::write(&file_path, create_template_content(&date))?;
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
    println!("\n{}", "=".repeat(60));
    println!("TIME TRACKING SUMMARY");
    println!("{}", "=".repeat(60));
    
    let data = parse_time_tracking_data(content);
    
    // Display overview
    println!("\n📅 TIME OVERVIEW");
    println!("Start Time: {}", data.formatted_start_time());
    println!("End Time:   {}", data.formatted_end_time());
    
    // Display total working time
    println!("\n⏱️  WORKING TIME");
    println!("Total: {} ({} hours)", 
        data.formatted_total_minutes(), 
        data.formatted_total_decimal()
    );
    
    // Display dead time
    println!("\n⏸️  DEAD TIME");
    if data.dead_time_minutes == 0 {
        println!("✅ No dead time (gaps) found");
    } else {
        let status_icon = if data.dead_time_minutes < 90 { "⚠️" } else { "❌" };
        println!("{} {} ({} hours)", 
            status_icon,
            data.formatted_dead_time_minutes(), 
            data.formatted_dead_decimal()
        );
    }
    
    // Display warnings
    if !data.warnings.is_empty() {
        println!("\n⚠️  WARNINGS");
        for warning in &data.warnings {
            println!("  ⚠ {}", warning);
        }
    }
    
    // Display projects
    if !data.projects.is_empty() {
        println!("\n📋 PROJECTS");
        for project in &data.projects {
            println!("\n  📌 {} - {} ({} hrs)",
                project.name,
                Time::format_duration_minutes(project.total_minutes),
                Time::format_duration_decimal(project.total_minutes)
            );
            
            if !project.notes.is_empty() {
                for note in &project.notes {
                    println!("     • {}", note);
                }
            }
        }
    } else {
        println!("\n📋 PROJECTS");
        println!("  No projects found. Make sure to enter time tracking data in the format:");
        println!("  11:45-12:15 project_code");
        println!("  - Comment explaining what you did");
    }
    
    println!("\n{}", "=".repeat(60));
}
