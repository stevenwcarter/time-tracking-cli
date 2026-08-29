//! Test-only helpers shared by the TUI unit tests.

use ratatui::{Terminal, backend::TestBackend};
use time::{Date, macros::date};
use time_tracking_parser::{ProjectSummary, Time, TimeTrackingData};

use super::app::App;

/// The date every TUI render test opens on.
///
/// `App::new` otherwise defaults to today, which would make the calendar and
/// the weekly bar chart follow the wall clock. This one is mid-month and
/// mid-week (a Wednesday in a 30-day month), so no month-boundary or
/// week-boundary edge case is ever in play; its whole Saturday-start week
/// falls inside June 2025.
pub fn fixture_date() -> Date {
    date!(2025 - 06 - 11)
}

/// Render an `App` into an off-screen buffer and flatten it to a string,
/// one line per terminal row, trailing spaces trimmed.
pub fn render_to_string(app: &mut App, w: u16, h: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("test backend");
    terminal
        .draw(|frame| frame.render_widget(app, frame.area()))
        .expect("draw");
    let buf = terminal.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// An eight-hour day with three projects, two notes each, no warnings and no
/// dead time.
///
/// Later tasks assert against rendered output built from this fixture, so its
/// shape is deliberately stable: change it and those assertions all move.
///
/// Rendering it in full needs about **40 terminal rows**: the header row is a
/// fixed `Length(12)`, and three projects at three lines each plus the list
/// header and footer overflow an 80x24 buffer, silently clipping `internal`.
pub fn fixture_day() -> TimeTrackingData {
    TimeTrackingData {
        total_minutes: 480,
        dead_time_minutes: 0,
        projects: vec![
            project("admin", 90, ["standup", "inbox triage"]),
            project("client-bd", 240, ["discovery call", "proposal draft"]),
            project("internal", 150, ["code review", "release notes"]),
        ],
        warnings: Vec::new(),
        start_time: Some(time_at(9, 0)),
        end_time: Some(time_at(5, 0)),
    }
}

/// A day with `n` uniform one-hour projects, for exercising list scrolling.
pub fn fixture_day_with_projects(n: usize) -> TimeTrackingData {
    let projects = (0..n)
        .map(|i| {
            project(
                &format!("project-{i:02}"),
                60,
                [format!("note {i}a"), format!("note {i}b")],
            )
        })
        .collect();

    TimeTrackingData {
        total_minutes: 60 * n as u32,
        dead_time_minutes: 0,
        projects,
        warnings: Vec::new(),
        start_time: Some(time_at(9, 0)),
        end_time: Some(time_at(5, 0)),
    }
}

fn project(name: &str, total_minutes: u32, notes: [impl Into<String>; 2]) -> ProjectSummary {
    ProjectSummary {
        name: name.to_owned(),
        total_minutes,
        notes: notes.into_iter().map(Into::into).collect(),
    }
}

fn time_at(hour: u8, minute: u8) -> Time {
    Time::new(hour, minute).expect("fixture time is in range")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use time::Weekday;

    use super::*;
    use crate::time_utils::get_week_dates;
    use crate::tui::context::TuiContext;

    #[tokio::test]
    async fn renders_the_three_fixture_projects() {
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(fixture_day());
        let rendered = render_to_string(&mut app, 80, 40);

        assert!(
            rendered.contains("June 2025"),
            "calendar should be pinned to the fixture date:\n{rendered}"
        );
        for name in ["admin", "client-bd", "internal"] {
            assert!(rendered.contains(name), "{name} missing from:\n{rendered}");
        }
        assert!(
            rendered.contains("standup"),
            "notes missing from:\n{rendered}"
        );
    }

    #[tokio::test]
    async fn renders_a_generated_project_list() {
        let data = fixture_day_with_projects(12);
        assert_eq!(data.projects.len(), 12);
        assert_eq!(data.total_minutes, 720);

        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(data);
        let rendered = render_to_string(&mut app, 80, 24);
        assert!(
            rendered.contains("project-00"),
            "first project missing from:\n{rendered}"
        );
    }

    #[tokio::test]
    async fn seeded_week_data_reaches_the_bar_chart() {
        let week: HashMap<Date, u32> = get_week_dates(&fixture_date(), Weekday::Saturday)
            .into_iter()
            .map(|d| (d, u32::from(d == fixture_date()) * 480))
            .collect();

        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(fixture_day())
            .with_populated_dates(vec![fixture_date()])
            .with_weekly_data(week);
        let rendered = render_to_string(&mut app, 80, 40);

        assert!(
            rendered.contains("8.0h total"),
            "weekly minutes should reach the bar chart:\n{rendered}"
        );
    }

    #[tokio::test]
    async fn raw_content_is_parsed_into_the_project_list() {
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_raw_content(
                "9:00-10:30 admin\n- standup\n10:30-12:00 client-bd\n- discovery call\n",
            );

        let data = app.data.as_ref().expect("raw content should parse");
        assert_eq!(data.total_minutes, 180);
        assert_eq!(data.projects.len(), 2);

        let rendered = render_to_string(&mut app, 80, 24);
        assert!(
            rendered.contains("client-bd"),
            "parsed project missing from:\n{rendered}"
        );
    }
}
