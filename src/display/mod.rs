use std::fmt::Debug;
use std::io;
use std::io::Read;
use std::path::PathBuf;

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

use crate::{
    Config, DataService, data_svc::WeeklyProject, format_day_with_date, get_week_dates,
    open_in_editor,
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

    /// Display the weekly warnings block.
    ///
    /// Each formatter renders this, rather than the shared weekly renderer
    /// printing one hardcoded shape: `plain` exists to emit no emoji, and this
    /// block was the last part of the weekly output that ignored that.
    /// Mirrors the day path's `warnings_header` / `warning_bullet` styles.
    fn weekly_warnings(&self, warnings: &[String]) -> String;
    fn display_weekly_warnings(&self, warnings: &[String]);

    /// Display weekly projects summary
    fn weekly_projects(&self, projects: &[WeeklyProject]) -> String;
    fn display_weekly_projects(&self, projects: &[WeeklyProject]);

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

/// Print a week's summary using the process-wide [`DataService`].
pub async fn show_weekly_summary(
    date: &Date,
    week_start_day: Weekday,
    formatter: &dyn DisplayFormatter,
) -> Result<()> {
    show_weekly_summary_with(DataService::get(), date, week_start_day, formatter).await
}

/// Print a week's summary using `data_service`.
///
/// Both halves of this render — the aggregate that `get_weekly_summary`
/// computes and the per-day breakdowns the formatter parses itself — take
/// their prefix/suffix markers from `data_service`. They must: reading the
/// per-day markers from [`Config::get`] instead would silently disagree with
/// the aggregate for any service built by
/// [`DataService::new_with_dir`], and the two views of the same file would
/// then be bounded by different fences. `weekly_render_parses_both_halves_with_the_service_settings`
/// pins that.
pub async fn show_weekly_summary_with(
    data_service: &DataService,
    date: &Date,
    week_start_day: Weekday,
    formatter: &dyn DisplayFormatter,
) -> Result<()> {
    let week_dates = get_week_dates(date, week_start_day);

    formatter.display_weekly_header(
        &format_day_with_date(&week_dates[0]),
        &format_day_with_date(&week_dates[6]),
    );

    let summary = data_service.get_weekly_summary(&week_dates).await?;

    // Display weekly summary at the top
    formatter.display_weekly_totals(summary.total_minutes, summary.dead_time_minutes);

    if !summary.warnings.is_empty() {
        formatter.display_weekly_warnings(&summary.warnings);
    }

    if !summary.projects.is_empty() {
        formatter.display_weekly_projects(&summary.projects);
    }

    // Now display detailed daily summaries
    formatter.display_daily_breakdowns_header();

    let parse_settings = data_service.parse_settings();
    for (day_date, content, data_opt) in summary.days {
        formatter.display_day_header(&format_day_with_date(&day_date));

        if let Some(data) = data_opt {
            if data.total_minutes > 0 {
                formatter.display_day_summary(
                    &content,
                    "  ",
                    parse_settings.prefix.as_deref(),
                    parse_settings.suffix.as_deref(),
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
    use std::sync::Mutex;

    use time::macros::date;

    use super::*;
    use crate::data_svc::ParseSettings;

    fn project(name: &str, total_minutes: u32, notes: &[&str]) -> WeeklyProject {
        WeeklyProject {
            name: name.to_string(),
            total_minutes,
            notes: notes.iter().map(|n| (*n).to_string()).collect(),
        }
    }

    /// One per-day breakdown exactly as the renderer handed it to the
    /// formatter, markers included.
    #[derive(Debug)]
    struct RenderedDay {
        content: String,
        prefix: Option<String>,
        suffix: Option<String>,
    }

    /// Records what the weekly render hands each formatter method instead of
    /// printing it, so a test can assert on the arguments rather than on
    /// captured stdout.
    #[derive(Debug, Default)]
    struct SpyFormatter {
        weekly_totals: Mutex<Vec<(u32, u32)>>,
        day_summaries: Mutex<Vec<RenderedDay>>,
    }

    impl DisplayFormatter for SpyFormatter {
        fn day_summary(
            &self,
            _content: &str,
            _indent: &str,
            _prefix: Option<&str>,
            _suffix: Option<&str>,
        ) -> String {
            String::new()
        }
        fn display_day_summary(
            &self,
            content: &str,
            _indent: &str,
            prefix: Option<&str>,
            suffix: Option<&str>,
        ) {
            self.day_summaries
                .lock()
                .expect("spy lock")
                .push(RenderedDay {
                    content: content.to_string(),
                    prefix: prefix.map(str::to_owned),
                    suffix: suffix.map(str::to_owned),
                });
        }

        fn weekly_header(&self, _week_start: &str, _week_end: &str) -> String {
            String::new()
        }
        fn display_weekly_header(&self, _week_start: &str, _week_end: &str) {}

        fn weekly_totals(&self, _total_minutes: u32, _dead_minutes: u32) -> String {
            String::new()
        }
        fn display_weekly_totals(&self, total_minutes: u32, dead_minutes: u32) {
            self.weekly_totals
                .lock()
                .expect("spy lock")
                .push((total_minutes, dead_minutes));
        }

        fn weekly_warnings(&self, _warnings: &[String]) -> String {
            String::new()
        }
        fn display_weekly_warnings(&self, _warnings: &[String]) {}

        fn weekly_projects(&self, _projects: &[WeeklyProject]) -> String {
            String::new()
        }
        fn display_weekly_projects(&self, _projects: &[WeeklyProject]) {}

        fn daily_breakdowns_header(&self) -> String {
            String::new()
        }
        fn display_daily_breakdowns_header(&self) {}

        fn day_header(&self, _day_with_date: &str) -> String {
            String::new()
        }
        fn display_day_header(&self, _day_with_date: &str) {}

        fn display_no_file_found(&self, _indent: &str) {}
        fn display_no_data_found(&self, _indent: &str) {}
    }

    /// The weekly render has two parse paths: the aggregate, which
    /// `DataService::get_weekly_summary` computes with the service's own
    /// markers, and the per-day breakdowns, which the formatter parses from
    /// raw content using markers the renderer supplies. They must come from
    /// the same place. Before this was pinned they agreed only because
    /// `show_weekly_summary` hardcoded the process-wide service, whose markers
    /// happen to be `Config::get()`'s -- so the first injected service would
    /// have parsed the two halves of the same file with different fences, with
    /// nothing failing.
    #[tokio::test]
    async fn weekly_render_parses_both_halves_with_the_service_settings() {
        let dir = tempfile::tempdir().expect("temp dir");
        let settings = ParseSettings {
            prefix: Some("```timetracking".to_string()),
            suffix: Some("```".to_string()),
            template_file: None,
        };
        let service = DataService::new_with_dir(60, dir.path().to_path_buf(), settings.clone());
        std::fs::write(
            dir.path().join("2026-08-24.md"),
            "9:00-10:00 ignored-before-the-fence\n\
             ```timetracking\n\
             10:00-11:30 admin\n\
             ```\n\
             11:30-12:00 ignored-after-the-fence\n",
        )
        .expect("write day file");

        let spy = SpyFormatter::default();
        show_weekly_summary_with(&service, &date!(2026 - 08 - 24), Weekday::Saturday, &spy)
            .await
            .expect("weekly render");

        assert_eq!(
            *spy.weekly_totals.lock().expect("spy lock"),
            vec![(90, 0)],
            "the aggregate half must honour the injected fence"
        );

        let day_summaries = spy.day_summaries.lock().expect("spy lock");
        assert_eq!(
            day_summaries.len(),
            1,
            "only the one populated day renders a breakdown"
        );
        let rendered = &day_summaries[0];
        assert_eq!(
            rendered.prefix, settings.prefix,
            "per-day prefix comes from the service"
        );
        assert_eq!(
            rendered.suffix, settings.suffix,
            "per-day suffix comes from the service"
        );
        assert_eq!(
            parse_time_tracking_data(
                &rendered.content,
                rendered.prefix.as_deref(),
                rendered.suffix.as_deref()
            )
            .total_minutes,
            90,
            "the per-day half must reach the same 90 minutes as the aggregate"
        );
        assert_ne!(
            parse_time_tracking_data(&rendered.content, None, None).total_minutes,
            90,
            "the fixture must be one where the markers change the answer, \
             otherwise this test would pass even with the halves disagreeing"
        );
    }

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
        formatter.display_weekly_projects(&[]);

        // Test with sample projects
        formatter.display_weekly_projects(&[project("test_project", 120, &["Note 1", "Note 2"])]);
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

        formatter.display_weekly_projects(&[]);
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

        formatter.display_weekly_projects(&[project("markdown_project", 180, &["Markdown note"])]);
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
        formatter.display_weekly_projects(&[]);

        // Project with no notes
        formatter.display_weekly_projects(&[project("no_notes_project", 60, &[])]);

        // Project with many notes
        formatter.display_weekly_projects(&[WeeklyProject {
            name: "many_notes_project".to_string(),
            total_minutes: 300,
            notes: (1..10).map(|i| format!("Note {}", i)).collect(),
        }]);

        // Multiple projects
        formatter.display_weekly_projects(&[
            project("project1", 120, &["Note 1"]),
            project("project2", 180, &["Note A", "Note B"]),
        ]);
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
