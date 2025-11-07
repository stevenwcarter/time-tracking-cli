use time_tracking_parser::Time;

use super::DisplayFormatter;

/// Default emoji-based display formatter
#[derive(Debug, Clone)]
pub struct DefaultDisplayFormatter;

impl DisplayFormatter for DefaultDisplayFormatter {
    fn day_summary(&self, content: &str, indent: &str) -> String {
        let data = time_tracking_parser::parse_time_tracking_data(content);

        let mut msg = String::new();

        // Display overview
        msg.push_str(&format!("{}📅 TIME OVERVIEW\n", indent));
        msg.push_str(&format!(
            "{}Start Time: {}\n",
            indent,
            data.formatted_start_time()
        ));
        msg.push_str(&format!(
            "{}End Time:   {}\n\n",
            indent,
            data.formatted_end_time()
        ));

        // Display total working time
        msg.push_str(&format!("{}⏱️  WORKING TIME\n", indent));
        msg.push_str(&format!(
            "{}Total: {} ({} hours)\n\n",
            indent,
            data.formatted_total_minutes(),
            data.formatted_total_decimal(),
        ));

        // Display dead time
        msg.push_str(&format!("{}⏸️  DEAD TIME\n", indent));
        if data.dead_time_minutes == 0 {
            msg.push_str(&format!("{}✅ No dead time (gaps) found\n\n", indent));
        } else {
            let status_icon = if data.dead_time_minutes < 90 {
                "⚠️"
            } else {
                "❌"
            };
            msg.push_str(&format!(
                "{}{} {} ({} hours)\n\n",
                indent,
                status_icon,
                data.formatted_dead_time_minutes(),
                data.formatted_dead_decimal(),
            ));
        }
        msg.push('\n');

        // Display warnings
        if !data.warnings.is_empty() {
            msg.push_str(&format!("{}⚠️  WARNINGS\n", indent));
            for warning in &data.warnings {
                msg.push_str(&format!("{}  ⚠ {}\n", indent, warning));
            }
            msg.push('\n');
        }

        // Display projects
        if !data.projects.is_empty() {
            msg.push_str(&format!("{}📋 PROJECTS\n", indent));
            for project in &data.projects {
                msg.push_str(&format!(
                    "{}  📌 {} - {} ({} hrs)\n",
                    indent,
                    project.name,
                    Time::format_duration_minutes(project.total_minutes),
                    Time::format_duration_decimal(project.total_minutes),
                ));

                if !project.notes.is_empty() {
                    for note in &project.notes {
                        msg.push_str(&format!("{}     • {}\n", indent, note));
                    }
                }
            }
        } else {
            msg.push_str(&format!("{}📋 PROJECTS\n", indent));
            msg.push_str(&format!(
                "{}  No projects found. Make sure to enter time tracking data in the format:\n",
                indent,
            ));
            msg.push_str(&format!("{}  11:45-12:15 project_code\n", indent));
            msg.push_str(&format!("{}  - Comment explaining what you did\n", indent));
        }
        msg.push('\n');

        msg
    }
    fn display_day_summary(&self, content: &str, indent: &str) {
        println!("{}", self.day_summary(content, indent));
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
        msg.push_str("\n📊 WEEKLY TOTALS\n");
        msg.push_str(&format!("{}\n", "-".repeat(40)));

        msg.push_str(&format!(
            "⏱️  Total Working Time: {} ({} hrs)\n",
            Time::format_duration_minutes(total_minutes),
            Time::format_duration_decimal(total_minutes),
        ));

        if dead_minutes > 0 {
            msg.push_str(&format!(
                "⏸️  Total Dead Time: {} ({} hrs)\n",
                Time::format_duration_minutes(dead_minutes),
                Time::format_duration_decimal(dead_minutes),
            ));
        } else {
            msg.push_str("⏸️  Total Dead Time: ✅ None\n");
        }

        msg
    }
    fn display_weekly_totals(&self, total_minutes: u32, dead_minutes: u32) {
        println!("{}", self.weekly_totals(total_minutes, dead_minutes));
    }

    fn weekly_projects(&self, projects: &[(&String, &(u32, Vec<String>))]) -> String {
        let mut msg = String::new();
        if !projects.is_empty() {
            msg.push_str("\n📋 WEEKLY PROJECTS SUMMARY\n");

            for (project_name, (total_minutes, notes)) in projects {
                msg.push_str(&format!(
                    "  📌 {} - {} ({} hrs)\n",
                    project_name,
                    Time::format_duration_minutes(*total_minutes),
                    Time::format_duration_decimal(*total_minutes),
                ));

                if !notes.is_empty() {
                    for note in notes {
                        msg.push_str(&format!("     • {}\n", note));
                    }
                }
            }
        }
        msg
    }
    fn display_weekly_projects(&self, projects: &[(&String, &(u32, Vec<String>))]) {
        println!("{}", self.weekly_projects(projects));
    }

    fn daily_breakdowns_header(&self) -> String {
        let mut msg = String::new();
        msg.push_str(&format!("\n{}\n", "=".repeat(80)));
        msg.push_str("DAILY BREAKDOWNS\n");
        msg.push_str(&format!("{}\n", "=".repeat(80)));
        msg
    }
    fn display_daily_breakdowns_header(&self) {
        println!("{}", self.daily_breakdowns_header());
    }

    fn day_header(&self, day_with_date: &str) -> String {
        let mut msg = String::new();
        msg.push_str(&format!("\n📅 {}\n", day_with_date));
        msg.push_str(&format!("{}\n", "=".repeat(60)));
        msg
    }
    fn display_day_header(&self, day_with_date: &str) {
        println!("{}", self.day_header(day_with_date));
    }

    fn display_no_file_found(&self, indent: &str) {
        println!("{}📄 No time tracking file found", indent);
    }

    fn display_no_data_found(&self, indent: &str) {
        println!("{}💤 No time tracking data found", indent);
    }
}
