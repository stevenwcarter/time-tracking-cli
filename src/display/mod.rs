use std::fmt::Debug;
use std::fs;
use std::io::Read;
use std::{clone::Clone, io};

mod default;
mod markdown;
mod plain;

pub use default::DefaultDisplayFormatter;
pub use markdown::MarkdownDisplayFormatter;
pub use plain::PlainDisplayFormatter;

use anyhow::Result;
use time::{Date, Weekday, macros::format_description};
use time_tracking_parser::parse_time_tracking_data;

use crate::{
    Config, DATE_FORMAT, create_template_content, format_day_with_date,
    get_time_tracking_dir_with_override, get_week_dates, open_in_editor,
};

/// Trait for formatting and displaying time tracking data
pub trait DisplayFormatter: Debug + Send + Sync {
    /// Display a single day's time tracking results
    fn day_summary(
        &self,
        content: &str,
        indent: &str,
        prefix: Option<&str>,
        suffix: Option<&str>,
    ) -> String;
    fn display_day_summary(
        &self,
        content: &str,
        indent: &str,
        prefix: Option<&str>,
        suffix: Option<&str>,
    );

    /// Display the weekly summary header
    fn weekly_header(&self, week_start: &str, week_end: &str) -> String;
    fn display_weekly_header(&self, week_start: &str, week_end: &str);

    /// Display weekly totals
    fn weekly_totals(&self, total_minutes: u32, dead_minutes: u32) -> String;
    fn display_weekly_totals(&self, total_minutes: u32, dead_minutes: u32);

    /// Display weekly projects summary
    fn weekly_projects(&self, projects: &[(&String, &(u32, Vec<String>))]) -> String;
    fn display_weekly_projects(&self, projects: &[(&String, &(u32, Vec<String>))]);

    /// Display daily breakdowns header
    fn daily_breakdowns_header(&self) -> String;
    fn display_daily_breakdowns_header(&self);

    /// Display a single day header in weekly view
    fn day_header(&self, day_with_date: &str) -> String;
    fn display_day_header(&self, day_with_date: &str);

    /// Display message for missing file
    fn display_no_file_found(&self, indent: &str);

    /// Display message for no data
    fn display_no_data_found(&self, indent: &str);
}

pub async fn show_weekly_summary(
    date: &Date,
    week_start_day: Weekday,
    formatter: &dyn DisplayFormatter,
    config: &Config,
) -> Result<()> {
    let week_dates = get_week_dates(date, week_start_day);
    let time_tracking_dir = get_time_tracking_dir_with_override(config.get_data_directory())?;

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
        let filename = format!("{}.md", day_date.format(DATE_FORMAT).unwrap());
        let file_path = time_tracking_dir.join(&filename);

        if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            let data = parse_time_tracking_data(&content, config.get_prefix(), config.get_suffix());

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
                formatter.display_day_summary(
                    &content,
                    "  ",
                    config.get_prefix(),
                    config.get_suffix(),
                );
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

pub async fn read_day(date: &Date, config: &Config) -> Result<Option<String>> {
    // Create the time tracking directory
    let time_tracking_dir = get_time_tracking_dir_with_override(config.get_data_directory())?;
    fs::create_dir_all(&time_tracking_dir)?;

    let custom_format = format_description!("[year]-[month]-[day]");
    // Create the filename for the date
    let filename = format!("{}.md", date.format(&custom_format)?);
    let file_path = time_tracking_dir.join(&filename);

    // Create the file if it doesn't exist
    if !file_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&file_path)?;

    Ok(Some(content))
}
pub async fn show_single_day_stdin(
    formatter: &dyn DisplayFormatter,
    config: &Config,
) -> anyhow::Result<()> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    formatter.display_day_summary(
        buffer.as_str(),
        "",
        config.get_prefix(),
        config.get_suffix(),
    );

    Ok(())
}
pub async fn show_single_day(
    date: &Date,
    formatter: &dyn DisplayFormatter,
    config: &Config,
    noedit: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create the time tracking directory
    let time_tracking_dir = get_time_tracking_dir_with_override(config.get_data_directory())?;
    fs::create_dir_all(&time_tracking_dir)?;

    // Create the filename for the date
    let filename = format!(
        "{}.md",
        date.format(&format_description!("[year]-[month]-[day]"))?
    );
    let file_path = time_tracking_dir.join(&filename);

    // Create the file if it doesn't exist
    if !file_path.exists() {
        let template_content = create_template_content(date, config.get_template_file())?;
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
    let content = read_day(date, config).await?;
    if let Some(content) = content {
        formatter.display_day_summary(&content, "", config.get_prefix(), config.get_suffix());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_display_formatter_creation() {
        let formatter = DefaultDisplayFormatter;

        // Test that we can create the formatter
        // This mainly tests that the struct can be instantiated
        let _formatter_ref: &dyn DisplayFormatter = &formatter;
    }

    #[test]
    fn test_plain_display_formatter_creation() {
        let formatter = PlainDisplayFormatter;

        // Test that we can create the formatter
        let _formatter_ref: &dyn DisplayFormatter = &formatter;
    }

    #[test]
    fn test_markdown_display_formatter_creation() {
        let formatter = MarkdownDisplayFormatter;

        // Test that we can create the formatter
        let _formatter_ref: &dyn DisplayFormatter = &formatter;
    }

    #[tokio::test]
    async fn test_display_formatter_trait_methods() {
        let formatter = DefaultDisplayFormatter;

        // Test that all trait methods can be called without panicking
        formatter.display_weekly_header("2023-10-09", "2023-10-15");
        formatter.display_weekly_totals(480, 30); // 8 hours work, 30 min dead time
        formatter.display_daily_breakdowns_header();
        formatter.display_day_header("Monday 2023-10-09");
        formatter.display_no_file_found("  ");
        formatter.display_no_data_found("  ");

        // Test with empty projects
        let empty_projects: Vec<(&String, &(u32, Vec<String>))> = vec![];
        formatter.display_weekly_projects(&empty_projects);

        // Test with sample projects
        let project_name = "test_project".to_string();
        let project_data = (120u32, vec!["Note 1".to_string(), "Note 2".to_string()]);
        let projects = vec![(&project_name, &project_data)];
        formatter.display_weekly_projects(&projects);
    }

    #[tokio::test]
    async fn test_plain_display_formatter_methods() {
        let formatter = PlainDisplayFormatter;

        // Test that all trait methods can be called without panicking
        formatter.display_weekly_header("2023-10-09", "2023-10-15");
        formatter.display_weekly_totals(480, 0); // 8 hours work, no dead time
        formatter.display_daily_breakdowns_header();
        formatter.display_day_header("Tuesday 2023-10-10");
        formatter.display_no_file_found("");
        formatter.display_no_data_found("");

        let empty_projects: Vec<(&String, &(u32, Vec<String>))> = vec![];
        formatter.display_weekly_projects(&empty_projects);
    }

    #[tokio::test]
    async fn test_markdown_display_formatter_methods() {
        let formatter = MarkdownDisplayFormatter;

        // Test that all trait methods can be called without panicking
        formatter.display_weekly_header("2023-10-09", "2023-10-15");
        formatter.display_weekly_totals(240, 15); // 4 hours work, 15 min dead time
        formatter.display_daily_breakdowns_header();
        formatter.display_day_header("Wednesday 2023-10-11");
        formatter.display_no_file_found("    ");
        formatter.display_no_data_found("    ");

        let project_name = "markdown_project".to_string();
        let project_data = (180u32, vec!["Markdown note".to_string()]);
        let projects = vec![(&project_name, &project_data)];
        formatter.display_weekly_projects(&projects);
    }

    #[tokio::test]
    async fn test_display_day_summary_with_sample_data() {
        let formatter = DefaultDisplayFormatter;

        // Test with various content strings
        let test_contents = [
            "",                                                                            // Empty content
            "9:00-10:00 project1\n- Task description", // Basic time entry
            "9:00-12:00 project1\n- Morning work\n13:00-17:00 project2\n- Afternoon work", // Multiple entries
            "Invalid time entry", // Invalid format
        ];

        for content in &test_contents {
            // Test that the method doesn't panic with various inputs
            formatter.display_day_summary(content, "  ", None, None);
        }
    }

    #[tokio::test]
    async fn test_display_methods_with_different_indents() {
        let formatter = DefaultDisplayFormatter;

        let test_indents = ["", "  ", "    ", "\t", ">>"];

        for indent in &test_indents {
            formatter.display_no_file_found(indent);
            formatter.display_no_data_found(indent);
            formatter.display_day_summary("9:00-10:00 test", indent, None, None);
        }
    }

    #[tokio::test]
    async fn test_weekly_totals_edge_cases() {
        let formatter = DefaultDisplayFormatter;

        // Test edge cases for weekly totals
        formatter.display_weekly_totals(0, 0); // No time at all
        formatter.display_weekly_totals(1, 0); // 1 minute work
        formatter.display_weekly_totals(0, 1); // 1 minute dead time
        formatter.display_weekly_totals(u32::MAX, u32::MAX); // Maximum values
    }

    #[tokio::test]
    async fn test_projects_display_edge_cases() {
        let formatter = DefaultDisplayFormatter;

        // Empty projects
        let empty_projects: Vec<(&String, &(u32, Vec<String>))> = vec![];
        formatter.display_weekly_projects(&empty_projects);

        // Project with no notes
        let project_name = "no_notes_project".to_string();
        let project_data = (60u32, vec![]);
        let projects_no_notes = vec![(&project_name, &project_data)];
        formatter.display_weekly_projects(&projects_no_notes);

        // Project with many notes
        let project_name_many = "many_notes_project".to_string();
        let many_notes = (1..10).map(|i| format!("Note {}", i)).collect();
        let project_data_many = (300u32, many_notes);
        let projects_many_notes = vec![(&project_name_many, &project_data_many)];
        formatter.display_weekly_projects(&projects_many_notes);

        // Multiple projects
        let project1_name = "project1".to_string();
        let project1_data = (120u32, vec!["Note 1".to_string()]);
        let project2_name = "project2".to_string();
        let project2_data = (180u32, vec!["Note A".to_string(), "Note B".to_string()]);
        let multiple_projects = vec![
            (&project1_name, &project1_data),
            (&project2_name, &project2_data),
        ];
        formatter.display_weekly_projects(&multiple_projects);
    }

    #[tokio::test]
    async fn test_formatter_polymorphism() {
        let formatters: Vec<Box<dyn DisplayFormatter>> = vec![
            Box::new(DefaultDisplayFormatter),
            Box::new(PlainDisplayFormatter),
            Box::new(MarkdownDisplayFormatter),
        ];

        for formatter in formatters {
            // Test that we can use each formatter polymorphically
            formatter.display_weekly_header("2023-10-09", "2023-10-15");
            formatter.display_weekly_totals(480, 30);
            formatter.display_no_file_found("  ");
        }
    }

    #[tokio::test]
    async fn test_date_string_handling() {
        let formatter = DefaultDisplayFormatter;

        // Test with various date string formats
        let date_strings = [
            "Monday 2023-10-09",
            "2023-10-09",
            "Oct 9, 2023",
            "Invalid date",
            "",
        ];

        for date_str in &date_strings {
            formatter.display_day_header(date_str);
        }

        // Test week range formatting
        let week_ranges = [
            ("2023-10-09", "2023-10-15"),
            ("2023-12-25", "2023-12-31"),
            ("", ""),
            ("Invalid", "Also Invalid"),
        ];

        for (start, end) in &week_ranges {
            formatter.display_weekly_header(start, end);
        }
    }
}
