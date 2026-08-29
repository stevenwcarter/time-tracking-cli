use time_tracking_parser::Time;

use super::{DaySummaryStyle, DisplayFormatter, WeeklyProject, format_day_summary_impl};

const PLAIN_STYLE: DaySummaryStyle = DaySummaryStyle {
    overview_header: "TIME OVERVIEW",
    working_header: "WORKING TIME",
    dead_header: "DEAD TIME",
    no_dead_msg: "No dead time (gaps) found",
    dead_warn: "WARNING",
    dead_error: "ERROR",
    dead_sep: ": ",
    warnings_header: "WARNINGS",
    warning_bullet: "  - ",
    projects_header: "PROJECTS",
    project_bullet: "  * ",
    note_bullet: "    - ",
    extra_spacing: false,
};

/// Plain text display formatter (no emojis)
#[derive(Debug, Clone)]
pub struct PlainDisplayFormatter;

impl DisplayFormatter for PlainDisplayFormatter {
    fn day_summary(
        &self,
        content: &str,
        indent: &str,
        prefix: Option<&str>,
        suffix: Option<&str>,
    ) -> String {
        format_day_summary_impl(content, indent, prefix, suffix, &PLAIN_STYLE)
    }
    fn display_day_summary(
        &self,
        content: &str,
        indent: &str,
        prefix: Option<&str>,
        suffix: Option<&str>,
    ) {
        println!("{}", self.day_summary(content, indent, prefix, suffix));
    }

    fn weekly_header(&self, week_start: &str, week_end: &str) -> String {
        let mut msg = String::new();
        msg.push_str(&format!("\n{}\n", "=".repeat(80)));
        msg.push_str("WEEKLY TIME TRACKING SUMMARY\n");
        msg.push_str(&format!("Week of {} to {}\n", week_start, week_end));
        msg.push_str(&format!("{}\n", "=".repeat(80)));
        msg
    }
    fn display_weekly_header(&self, week_start: &str, week_end: &str) {
        println!("{}", self.weekly_header(week_start, week_end));
    }

    fn weekly_totals(&self, total_minutes: u32, dead_minutes: u32) -> String {
        let mut msg = String::new();
        msg.push_str("\nWEEKLY TOTALS\n");
        msg.push_str(&"-\n".repeat(40));

        msg.push_str(&format!(
            "Total Working Time: {} ({} hrs)\n",
            Time::format_duration_minutes(total_minutes),
            Time::format_duration_decimal(total_minutes),
        ));

        if dead_minutes > 0 {
            msg.push_str(&format!(
                "Total Dead Time: {} ({} hrs)\n",
                Time::format_duration_minutes(dead_minutes),
                Time::format_duration_decimal(dead_minutes),
            ));
        } else {
            msg.push_str("Total Dead Time: None\n");
        }
        msg
    }
    fn display_weekly_totals(&self, total_minutes: u32, dead_minutes: u32) {
        println!("{}", self.weekly_totals(total_minutes, dead_minutes));
    }

    fn weekly_warnings(&self, warnings: &[String]) -> String {
        let mut msg = String::new();
        if !warnings.is_empty() {
            msg.push_str("\nWEEKLY WARNINGS");
            for warning in warnings {
                msg.push_str(&format!("\n  - {warning}"));
            }
        }
        msg
    }
    fn display_weekly_warnings(&self, warnings: &[String]) {
        println!("{}", self.weekly_warnings(warnings));
    }

    fn weekly_projects(&self, projects: &[WeeklyProject]) -> String {
        let mut msg = String::new();
        if !projects.is_empty() {
            msg.push_str("\nWEEKLY PROJECTS SUMMARY\n\n");

            for project in projects {
                msg.push_str(&format!(
                    "  * {} - {} ({} hrs)\n",
                    project.name,
                    Time::format_duration_minutes(project.total_minutes),
                    Time::format_duration_decimal(project.total_minutes),
                ));

                if !project.notes.is_empty() {
                    for note in &project.notes {
                        msg.push_str(&format!("    - {}\n", note));
                    }
                }
            }
        }
        msg
    }
    fn display_weekly_projects(&self, projects: &[WeeklyProject]) {
        println!("{}", self.weekly_projects(projects));
    }

    fn daily_breakdowns_header(&self) -> String {
        let mut msg = String::new();
        msg.push_str(&format!("\n{}\n", "=".repeat(80)));
        msg.push_str("DAILY BREAKDOWNS\n");
        msg.push_str(&"=".repeat(80));
        msg.push('\n');
        msg
    }
    fn display_daily_breakdowns_header(&self) {
        println!("{}", self.daily_breakdowns_header());
    }

    fn day_header(&self, day_with_date: &str) -> String {
        let mut msg = String::new();
        msg.push_str(&format!("\n{}\n", day_with_date));
        msg.push_str(&"=".repeat(60));
        msg.push('\n');
        msg
    }
    fn display_day_header(&self, day_with_date: &str) {
        println!("{}", self.day_header(day_with_date));
    }

    fn display_no_file_found(&self, indent: &str) {
        println!("{}No time tracking file found", indent);
    }

    fn display_no_data_found(&self, indent: &str) {
        println!("{}No time tracking data found", indent);
    }
}
