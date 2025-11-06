use super::DisplayFormatter;
use time_tracking_parser::Time;

/// Markdown-formatted display formatter
pub struct MarkdownDisplayFormatter;

impl DisplayFormatter for MarkdownDisplayFormatter {
    fn display_day_summary(
        &self,
        content: &str,
        indent: &str,
        prefix: Option<&str>,
        suffix: Option<&str>,
    ) {
        let data = time_tracking_parser::parse_time_tracking_data(content, prefix, suffix);

        println!(
            "{}**Total Time:** {} hours",
            indent,
            data.formatted_total_decimal()
        );

        if data.dead_time_minutes > 0 {
            println!(
                "{}**Dead Time:** {} hours",
                indent,
                data.formatted_dead_decimal()
            );
        }

        // Display warnings
        if !data.warnings.is_empty() {
            println!("{}**Warnings:**", indent);
            for warning in &data.warnings {
                println!("{}  - ⚠️ {}", indent, warning);
            }
        }

        // Display projects
        if !data.projects.is_empty() {
            println!("{}**Projects:**", indent);
            for project in &data.projects {
                println!(
                    "{}  - **{}** - {} hours",
                    indent,
                    project.name,
                    Time::format_duration_decimal(project.total_minutes)
                );
                for note in &project.notes {
                    println!("{}    - {}", indent, note);
                }
            }
        }
    }

    fn display_weekly_header(&self, week_start: &str, week_end: &str) {
        println!("# Weekly Summary");
        println!("**Period:** {} to {}", week_start, week_end);
        println!();
    }

    fn display_weekly_totals(&self, total_minutes: u32, dead_minutes: u32) {
        println!("## Summary");
        println!(
            "- **Total Time:** {} hours",
            Time::format_duration_decimal(total_minutes)
        );
        if dead_minutes > 0 {
            println!(
                "- **Dead Time:** {} hours",
                Time::format_duration_decimal(dead_minutes)
            );
        }
        println!();
    }

    fn display_weekly_projects(&self, projects: &[(&String, &(u32, Vec<String>))]) {
        if !projects.is_empty() {
            println!("## Projects");
            for (project_name, (total_minutes, notes)) in projects {
                println!("### {}", project_name);
                println!(
                    "**Time:** {} hours",
                    Time::format_duration_decimal(*total_minutes)
                );

                if !notes.is_empty() {
                    println!("**Notes:**");
                    for note in notes {
                        println!("- {}", note);
                    }
                }
                println!();
            }
        }
    }

    fn display_daily_breakdowns_header(&self) {
        println!("## Daily Breakdowns");
        println!();
    }

    fn display_day_header(&self, day_with_date: &str) {
        println!("### {}", day_with_date);
        println!();
    }

    fn display_no_file_found(&self, indent: &str) {
        println!("{}*No time tracking file found*", indent);
    }

    fn display_no_data_found(&self, indent: &str) {
        println!("{}*No time tracking data found*", indent);
    }
}
