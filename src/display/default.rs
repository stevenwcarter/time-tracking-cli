use time_tracking_parser::Time;

use super::DisplayFormatter;

/// Default emoji-based display formatter
pub struct DefaultDisplayFormatter;

impl DisplayFormatter for DefaultDisplayFormatter {
    fn display_day_summary(&self, content: &str, indent: &str) {
        let data = time_tracking_parser::parse_time_tracking_data(content);

        // Display overview
        println!("{}📅 TIME OVERVIEW\n", indent);
        println!("{}Start Time: {}", indent, data.formatted_start_time());
        println!("{}End Time:   {}\n", indent, data.formatted_end_time());

        // Display total working time
        println!("{}⏱️  WORKING TIME", indent);
        println!(
            "{}Total: {} ({} hours)\n",
            indent,
            data.formatted_total_minutes(),
            data.formatted_total_decimal()
        );

        // Display dead time
        println!("{}⏸️  DEAD TIME", indent);
        if data.dead_time_minutes == 0 {
            println!("{}✅ No dead time (gaps) found", indent);
        } else {
            let status_icon = if data.dead_time_minutes < 90 {
                "⚠️"
            } else {
                "❌"
            };
            println!(
                "{}{} {} ({} hours)\n",
                indent,
                status_icon,
                data.formatted_dead_time_minutes(),
                data.formatted_dead_decimal()
            );
        }
        println!();

        // Display warnings
        if !data.warnings.is_empty() {
            println!("{}⚠️  WARNINGS", indent);
            for warning in &data.warnings {
                println!("{}  ⚠ {}", indent, warning);
            }
            println!();
        }

        // Display projects
        if !data.projects.is_empty() {
            println!("{}📋 PROJECTS", indent);
            for project in &data.projects {
                println!(
                    "{}  📌 {} - {} ({} hrs)",
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
            println!(
                "{}  No projects found. Make sure to enter time tracking data in the format:",
                indent
            );
            println!("{}  11:45-12:15 project_code", indent);
            println!("{}  - Comment explaining what you did", indent);
        }
        println!();
    }

    fn display_weekly_header(&self, week_start: &str, week_end: &str) {
        println!("\n{}", "=".repeat(80));
        println!("WEEKLY TIME TRACKING SUMMARY");
        println!("Week of {} to {}", week_start, week_end);
        println!("{}", "=".repeat(80));
    }

    fn display_weekly_totals(&self, total_minutes: u32, dead_minutes: u32) {
        println!("\n📊 WEEKLY TOTALS");
        println!("{}", "-".repeat(40));

        println!(
            "⏱️  Total Working Time: {} ({} hrs)",
            Time::format_duration_minutes(total_minutes),
            Time::format_duration_decimal(total_minutes)
        );

        if dead_minutes > 0 {
            println!(
                "⏸️  Total Dead Time: {} ({} hrs)",
                Time::format_duration_minutes(dead_minutes),
                Time::format_duration_decimal(dead_minutes)
            );
        } else {
            println!("⏸️  Total Dead Time: ✅ None");
        }
    }

    fn display_weekly_projects(&self, projects: &[(&String, &(u32, Vec<String>))]) {
        if !projects.is_empty() {
            println!("\n📋 WEEKLY PROJECTS SUMMARY");

            for (project_name, (total_minutes, notes)) in projects {
                println!(
                    "  📌 {} - {} ({} hrs)",
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
    }

    fn display_daily_breakdowns_header(&self) {
        println!("\n{}", "=".repeat(80));
        println!("DAILY BREAKDOWNS");
        println!("{}", "=".repeat(80));
    }

    fn display_day_header(&self, day_with_date: &str) {
        println!("\n📅 {}", day_with_date);
        println!("{}", "=".repeat(60));
    }

    fn display_no_file_found(&self, indent: &str) {
        println!("{}📄 No time tracking file found", indent);
    }

    fn display_no_data_found(&self, indent: &str) {
        println!("{}💤 No time tracking data found", indent);
    }
}
