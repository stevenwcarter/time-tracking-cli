use std::fmt::Debug;
use std::io::Read;
use std::path::PathBuf;
use std::{clone::Clone, io};

mod default;
mod markdown;
mod plain;

pub use default::DefaultDisplayFormatter;
pub use markdown::MarkdownDisplayFormatter;
pub use plain::PlainDisplayFormatter;

use anyhow::Result;
use time::{Date, Weekday};
use time_tracking_parser::{Time, TimeTrackingData, format_time_option, parse_time_tracking_data};
use tracing::info;

use crate::{Config, DataService, format_day_with_date, get_week_dates, open_in_editor};

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

pub(super) struct DaySummaryStyle {
    pub overview_header: &'static str,
    pub working_header: &'static str,
    pub dead_header: &'static str,
    pub no_dead_msg: &'static str,
    pub dead_warn: &'static str,
    pub dead_error: &'static str,
    /// Separator between the status label and the dead-time content (" " vs ": ")
    pub dead_sep: &'static str,
    pub warnings_header: &'static str,
    pub warning_bullet: &'static str,
    pub projects_header: &'static str,
    pub project_bullet: &'static str,
    pub note_bullet: &'static str,
    /// When true, insert extra blank lines between sections (emoji/default style)
    pub extra_spacing: bool,
}

pub(super) fn format_day_summary_impl(
    content: &str,
    indent: &str,
    prefix: Option<&str>,
    suffix: Option<&str>,
    style: &DaySummaryStyle,
) -> String {
    let data = parse_time_tracking_data(content, prefix, suffix);
    let mut msg = String::new();

    // Overview section
    msg.push_str(&format!("{}{}\n", indent, style.overview_header));
    msg.push_str(&format!(
        "{}Start Time: {}\n",
        indent,
        format_time_option(data.start_time.as_ref(), "N/A")
    ));
    msg.push_str(&format!(
        "{}End Time:   {}\n",
        indent,
        format_time_option(data.end_time.as_ref(), "N/A")
    ));
    if style.extra_spacing {
        msg.push('\n');
    }

    // Working time section
    msg.push_str(&format!("{}{}\n", indent, style.working_header));
    msg.push_str(&format!(
        "{}Total: {} ({} hours)\n",
        indent,
        data.formatted_total_minutes(),
        data.formatted_total_decimal(),
    ));
    if style.extra_spacing {
        msg.push('\n');
    }

    // Dead time section
    msg.push_str(&format!("{}{}\n", indent, style.dead_header));
    if data.dead_time_minutes == 0 {
        msg.push_str(&format!("{}{}\n", indent, style.no_dead_msg));
    } else {
        let status = if data.dead_time_minutes < 90 {
            style.dead_warn
        } else {
            style.dead_error
        };
        msg.push_str(&format!(
            "{}{}{}{} ({} hours)\n",
            indent,
            status,
            style.dead_sep,
            data.formatted_dead_time_minutes(),
            data.formatted_dead_decimal(),
        ));
    }
    if style.extra_spacing {
        msg.push('\n');
        msg.push('\n');
    }

    // Warnings section
    if !data.warnings.is_empty() {
        msg.push_str(&format!("{}{}\n", indent, style.warnings_header));
        for warning in &data.warnings {
            msg.push_str(&format!("{}{}{}\n", indent, style.warning_bullet, warning));
        }
        if style.extra_spacing {
            msg.push('\n');
        }
    }

    // Projects section
    if !data.projects.is_empty() {
        msg.push_str(&format!("{}{}\n", indent, style.projects_header));
        for project in &data.projects {
            msg.push_str(&format!(
                "{}{}{} - {} ({} hrs)\n",
                indent,
                style.project_bullet,
                project.name,
                Time::format_duration_minutes(project.total_minutes),
                Time::format_duration_decimal(project.total_minutes),
            ));
            for note in &project.notes {
                msg.push_str(&format!("{}{}{}\n", indent, style.note_bullet, note));
            }
        }
    } else {
        msg.push_str(&format!("{}{}\n", indent, style.projects_header));
        msg.push_str(&format!(
            "{}  No projects found. Make sure to enter time tracking data in the format:\n",
            indent,
        ));
        msg.push_str(&format!("{}  11:45-12:15 project_code\n", indent));
        msg.push_str(&format!("{}  - Comment explaining what you did\n", indent));
    }
    if style.extra_spacing {
        msg.push('\n');
    }

    msg
}

pub async fn show_weekly_summary(
    date: &Date,
    week_start_day: Weekday,
    formatter: &dyn DisplayFormatter,
) -> Result<()> {
    let data_service = DataService::get();
    let week_dates = get_week_dates(date, week_start_day);

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
        if let Some(content) = data_service.read_day(day_date).await? {
            let config = Config::get();
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
        projects.sort_by_key(|a| std::cmp::Reverse(a.1.0)); // Sort by total minutes descending
        formatter.display_weekly_projects(&projects);
    }

    // Now display detailed daily summaries
    formatter.display_daily_breakdowns_header();

    let config = Config::get();
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

pub async fn get_file_path(date: Date) -> Result<PathBuf> {
    DataService::get().get_file_path(date).await
}

pub async fn read_day(date: &Date) -> Result<Option<String>> {
    DataService::get().read_day(date).await
}

pub async fn parse_day(date: &Date) -> Result<Option<TimeTrackingData>> {
    DataService::get().parse_day(date).await
}
pub async fn show_single_day_stdin(formatter: &dyn DisplayFormatter) -> anyhow::Result<()> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    let config = Config::get();

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
    noedit: bool,
) -> Result<()> {
    let data_service = DataService::get();

    // Create the file if it doesn't exist
    let file_path = data_service.create_day_file_if_not_exists(date).await?;

    if !noedit {
        info!("Opening time tracking file: {}", file_path.display());

        // Open the file in the default editor
        open_in_editor(&file_path)?;

        // Invalidate cache since we just edited the file
        data_service.invalidate_date(date).await;
    }

    // Parse and display the results
    let content = data_service.read_day(date).await?;
    let config = Config::get();
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
