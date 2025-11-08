use super::DisplayFormatter;
use time_tracking_parser::Time;

/// Markdown-formatted display formatter
#[derive(Debug, Clone)]
pub struct MarkdownDisplayFormatter;

impl DisplayFormatter for MarkdownDisplayFormatter {
    fn day_summary(
        &self,
        content: &str,
        indent: &str,
        prefix: Option<&str>,
        suffix: Option<&str>,
    ) -> String {
        let data = time_tracking_parser::parse_time_tracking_data(content, prefix, suffix);
        let mut msg = String::new();

        msg.push_str(&format!(
            "{}**Total Time:** {} hours\n",
            indent,
            data.formatted_total_decimal(),
        ));

        if data.dead_time_minutes > 0 {
            msg.push_str(&format!(
                "{}**Dead Time:** {} hours\n",
                indent,
                data.formatted_dead_decimal(),
            ));
        }

        // Display warnings
        if !data.warnings.is_empty() {
            msg.push_str(&format!("{}**Warnings:**\n", indent));
            for warning in &data.warnings {
                msg.push_str(&format!("{}  - ⚠️ {}\n", indent, warning));
            }
        }

        // Display projects
        if !data.projects.is_empty() {
            msg.push_str(&format!("{}**Projects:**\n", indent));
            for project in &data.projects {
                msg.push_str(&format!(
                    "{}  - **{}** - {} hours",
                    indent,
                    project.name,
                    Time::format_duration_decimal(project.total_minutes),
                ));
                for note in &project.notes {
                    msg.push_str(&format!("{}    - {}\n", indent, note));
                }
            }
        }
        msg
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
        msg.push_str("# Weekly Summary\n");
        msg.push_str(&format!("**Period:** {} to {}\n\n", week_start, week_end));
        msg
    }
    fn display_weekly_header(&self, week_start: &str, week_end: &str) {
        println!("{}", self.weekly_header(week_start, week_end));
    }

    fn weekly_totals(&self, total_minutes: u32, dead_minutes: u32) -> String {
        let mut msg = String::new();
        msg.push_str("## Summary\n");
        msg.push_str(&format!(
            "- **Total Time:** {} hours\n",
            Time::format_duration_decimal(total_minutes),
        ));
        if dead_minutes > 0 {
            msg.push_str(&format!(
                "- **Dead Time:** {} hours\n",
                Time::format_duration_decimal(dead_minutes),
            ));
        }
        msg.push('\n');
        msg
    }
    fn display_weekly_totals(&self, total_minutes: u32, dead_minutes: u32) {
        println!("{}", self.weekly_totals(total_minutes, dead_minutes));
    }

    fn weekly_projects(&self, projects: &[(&String, &(u32, Vec<String>))]) -> String {
        let mut msg = String::new();
        if !projects.is_empty() {
            msg.push_str("## Projects\n");
            for (project_name, (total_minutes, notes)) in projects {
                msg.push_str(&format!("### {}\n", project_name));
                msg.push_str(&format!(
                    "**Time:** {} hours\n",
                    Time::format_duration_decimal(*total_minutes),
                ));

                if !notes.is_empty() {
                    msg.push_str("**Notes:**\n");
                    for note in notes {
                        msg.push_str(&format!("- {}\n", note));
                    }
                }
                msg.push('\n');
            }
        }
        msg
    }
    fn display_weekly_projects(&self, projects: &[(&String, &(u32, Vec<String>))]) {
        println!("{}", self.weekly_projects(projects));
    }

    fn daily_breakdowns_header(&self) -> String {
        "## Daily Breakdowns\n".to_owned()
    }
    fn display_daily_breakdowns_header(&self) {
        println!("{}", self.daily_breakdowns_header());
    }

    fn day_header(&self, day_with_date: &str) -> String {
        format!("### {}\n", day_with_date)
    }
    fn display_day_header(&self, day_with_date: &str) {
        println!("{}", self.day_header(day_with_date));
    }

    fn display_no_file_found(&self, indent: &str) {
        println!("{}*No time tracking file found*", indent);
    }

    fn display_no_data_found(&self, indent: &str) {
        println!("{}*No time tracking data found*", indent);
    }
}
