use ratatui::prelude::*;
use ratatui::widgets::*;
use time::Date;

use crate::{DATE_FORMAT, time_utils::WeekdayExt};

use super::app::{App, DayPane, LOADING_MESSAGE};
use super::mode::{Mode, Overlay};
use super::theme::Theme;
use super::widgets::HelpPopup;
use super::widgets::{Calendar, WeeklyBarChart};

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // The status line is `App`'s, not the project list's, and it is drawn
        // in every mode: the help hint has to survive a day with no project
        // list at all, which is the one screen a new user is most likely to
        // meet first.
        let [main_area, status_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

        match self.mode {
            Mode::Day => self.render_day(main_area, buf),
            Mode::ZoomedWeek => self.render_zoomed_week(main_area, buf),
            // Tasks 20 and 16 replace these with the real views.
            Mode::Week => render_placeholder("Week view", &self.ctx.theme, main_area, buf),
            Mode::RawFile => render_placeholder("Raw file view", &self.ctx.theme, main_area, buf),
        }
        self.render_status(status_area, buf);
        // Drawn last, and in every mode: the help popup used to be skipped
        // entirely while the bar chart was zoomed.
        self.render_overlay(area, buf);
    }
}

impl App {
    /// The day view: calendar and weekly bar chart above the project list.
    fn render_day(&mut self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(12), Constraint::Min(9)].as_ref())
            .split(area);
        let header_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Fill(1)])
            .split(chunks[0]);
        let calendar_area = header_area[0];
        let bar_chart_area = header_area[1];

        Calendar::new(self.active_date, &self.populated_dates, &self.ctx.theme)
            .render(calendar_area, buf);

        // Create and render the weekly bar chart
        self.weekly_bar_chart().render(bar_chart_area, buf);

        if let Some(group_rect) = bounding_rect(&header_area) {
            Block::default()
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .title("tt-tui")
                .render(group_rect, buf);
        }

        let block = Block::bordered()
            .title(format_pane_title(self.active_date))
            .border_type(BorderType::Rounded);

        // Computed before the pane is borrowed mutably, and *not* simply
        // "is there a widget?": a project list left over from the previous
        // date must not be drawn under this date's title. See `DayPane`.
        match self.day_pane() {
            DayPane::Projects => {
                let inner = block.inner(chunks[1]);
                block.render(chunks[1], buf);
                if let Some(widget) = &mut self.project_list_widget {
                    widget.render(inner, buf);
                }
            }
            DayPane::Loading => {
                render_pane_message(
                    LOADING_MESSAGE,
                    self.ctx.theme.status,
                    block,
                    chunks[1],
                    buf,
                );
            }
            DayPane::Empty => {
                render_pane_message(EMPTY_TEXT, self.ctx.theme.warning, block, chunks[1], buf);
            }
        }
    }

    /// The weekly bar chart, full screen.
    fn render_zoomed_week(&mut self, area: Rect, buf: &mut Buffer) {
        self.weekly_bar_chart().render(area, buf);
    }

    /// Draw the modal layer, if one is open, over whatever the mode drew.
    fn render_overlay(&mut self, area: Rect, buf: &mut Buffer) {
        match &self.overlay {
            Some(Overlay::Help) => HelpPopup::new(&self.ctx.theme, self.mode).render(area, buf),
            // Task 17 renders the prompt here.
            Some(Overlay::DatePrompt(_)) | None => {}
        }
    }

    /// Build the weekly bar chart for the active date, pre-populated with the
    /// week data already loaded — but only when it is *this* week's.
    ///
    /// Crossing a week boundary moves `week_dates` at once and `weekly_data`
    /// only when the load lands, and the chart totals every value in the map:
    /// feeding it the old week's data would print the previous week's total
    /// above seven empty bars. Withholding it draws no bars at all, which is
    /// what an unloaded week should look like.
    fn weekly_bar_chart(&self) -> WeeklyBarChart<'_> {
        let mut bar_chart =
            WeeklyBarChart::new(self.active_date, &self.week_dates, &self.ctx.theme);
        bar_chart.set_daily_target_hours(self.ctx.daily_target_hours);
        if !self.weekly_data.is_empty() && !self.week_is_stale() {
            bar_chart.set_weekly_data(&self.weekly_data);
        }
        bar_chart
    }

    /// The status line: whatever `App::footer_text` says the footer should
    /// carry right now.
    fn render_status(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.footer_text())
            .style(self.ctx.theme.status)
            .wrap(Wrap { trim: true })
            .centered()
            .render(area, buf);
    }
}

/// What the day pane says when there is nothing to show for the date on
/// screen.
const EMPTY_TEXT: &str = "No data found for date";

/// A one-line message where the project list would be.
fn render_pane_message(text: &str, style: Style, block: Block<'_>, area: Rect, buf: &mut Buffer) {
    Paragraph::new(text)
        .block(block)
        .style(style)
        .alignment(Alignment::Left)
        .render(area, buf);
}

/// Stand-in for a mode whose view has not been built yet.
fn render_placeholder(name: &str, theme: &Theme, area: Rect, buf: &mut Buffer) {
    Paragraph::new(format!("{name} is not implemented yet"))
        .block(Block::bordered().border_type(BorderType::Rounded))
        .style(theme.warning)
        .alignment(Alignment::Center)
        .render(area, buf);
}

/// The project pane's title: the active date's short weekday plus its ISO
/// form, e.g. `"Thu 2026-08-27"`. This is the only textual confirmation of
/// which day is on screen once a `h`/`l` press has moved off the calendar's
/// highlighted cell, so it is attached to the pane whether or not that day
/// has any data.
///
/// Falls back to the bare ISO date if formatting ever fails, rather than
/// unwrapping and panicking the render loop over a display string.
fn format_pane_title(date: Date) -> String {
    let iso = date
        .format(DATE_FORMAT)
        .unwrap_or_else(|_| date.to_string());
    format!("{} {iso}", date.weekday().short_name())
}

fn bounding_rect(chunks: &[Rect]) -> Option<Rect> {
    if chunks.is_empty() {
        return None;
    }

    let x = chunks.iter().map(|r| r.x).min()?;
    let y = chunks.iter().map(|r| r.y).min()?;
    let right = chunks.iter().map(|r| r.x + r.width).max()?;
    let bottom = chunks.iter().map(|r| r.y + r.height).max()?;

    Some(Rect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use super::*;
    use crate::tui::context::TuiContext;
    use crate::tui::testing::{fixture_date, fixture_day, render_to_string};

    fn day_app() -> App {
        App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(fixture_day())
    }

    /// Regression: `render` early-returned on the zoom branch before the help
    /// check, so `?` did nothing visible while the chart was zoomed.
    #[test]
    fn help_renders_over_the_zoomed_chart() {
        let mut app = day_app();
        app.mode = Mode::ZoomedWeek;
        app.overlay = Some(Overlay::Help);

        let screen = render_to_string(&mut app, 100, 30);
        assert!(screen.contains("Help"), "got:\n{screen}");
    }

    #[test]
    fn help_renders_over_the_day_view() {
        let mut app = day_app();
        app.overlay = Some(Overlay::Help);

        let screen = render_to_string(&mut app, 100, 30);
        assert!(screen.contains("Help"), "got:\n{screen}");
    }

    #[test]
    fn the_zoomed_chart_takes_the_whole_screen() {
        let mut app = day_app();
        let day = render_to_string(&mut app, 100, 30);
        app.mode = Mode::ZoomedWeek;
        let zoomed = render_to_string(&mut app, 100, 30);

        assert!(day.contains("Project Summaries"), "got:\n{day}");
        assert!(
            !zoomed.contains("Project Summaries"),
            "the zoomed chart is the only thing on screen:\n{zoomed}"
        );
    }

    #[test]
    fn the_unbuilt_modes_say_so_rather_than_rendering_nothing() {
        for (mode, expected) in [(Mode::Week, "Week view"), (Mode::RawFile, "Raw file view")] {
            let mut app = day_app();
            app.mode = mode;
            let screen = render_to_string(&mut app, 100, 30);
            assert!(screen.contains(expected), "{mode:?} rendered:\n{screen}");
        }
    }

    /// This is the test that catches the bug: the block carrying the active
    /// date's title was built and then dropped without ever being attached
    /// when the day had data, so only `the_empty_pane_also_shows_the_active_date`
    /// passed before the fix.
    #[test]
    fn the_day_pane_shows_the_active_date_with_its_weekday() {
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(date!(2026 - 08 - 27))
            .with_data(fixture_day());
        let screen = render_to_string(&mut app, 100, 30);
        assert!(screen.contains("Thu 2026-08-27"), "got:\n{screen}");
    }

    #[test]
    fn the_empty_pane_also_shows_the_active_date() {
        let mut app = App::new(TuiContext::for_test()).with_active_date(date!(2026 - 08 - 27));
        let screen = render_to_string(&mut app, 100, 30);
        assert!(screen.contains("Thu 2026-08-27"), "got:\n{screen}");
    }
}
