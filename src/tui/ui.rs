use std::path::{Path, PathBuf};

use ratatui::layout::Flex;
use ratatui::prelude::*;
use ratatui::widgets::*;
use time::Date;

use crate::{DATE_FORMAT, time_utils::WeekdayExt};

use super::app::{App, DayPane, LOADING_MESSAGE, WeekPane};
use super::mode::{Mode, Overlay};
use super::theme::Theme;
use super::week_list::WeekListWidget;
use super::widgets::HelpPopup;
use super::widgets::{Calendar, RawFileView, WeeklyBarChart};

/// The narrowest terminal width the day view can lay out at all.
///
/// Load-bearing outside this file: other TUI layout math is written to fit
/// within this exact number (the day header's width budget in particular).
/// Changing it is a cross-cutting change, not a local tweak.
pub(crate) const MIN_COLS: u16 = 60;

/// The shortest terminal height the day view can lay out at all.
///
/// Load-bearing, same as [`MIN_COLS`].
pub(crate) const MIN_ROWS: u16 = 15;

/// Below this many rows the calendar/chart header stops earning its space.
const COMPACT_ROWS: u16 = 22;

/// Below this many columns the calendar no longer fits next to the chart.
const NARROW_COLS: u16 = 100;

/// Columns the calendar's block claims in the header row.
const CALENDAR_COLS: u16 = 24;

/// The chart stops growing past this width on a very wide terminal; the
/// header is centred instead of stretching edge to edge.
const MAX_CHART_COLS: u16 = 140;

/// How much room the terminal gives the day view, coarsened into the bands
/// [`App::render_day`] branches its layout on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Breakpoint {
    /// Narrower than [`MIN_COLS`] or shorter than [`MIN_ROWS`]: too small to
    /// draw the day view at all.
    TooSmall,
    /// Wide enough, but too short for the calendar/chart header: it
    /// collapses so the project list gets every row.
    Compact,
    /// Tall enough, but too narrow to fit the calendar beside the chart:
    /// the calendar is dropped and the chart takes the header's full width.
    Narrow,
    /// Room for the calendar, the chart and the project list all at once.
    Full,
}

/// Classifies `area` into the band [`App::render_day`] should lay out for.
fn breakpoint(area: Rect) -> Breakpoint {
    if area.width < MIN_COLS || area.height < MIN_ROWS {
        Breakpoint::TooSmall
    } else if area.height < COMPACT_ROWS {
        Breakpoint::Compact
    } else if area.width < NARROW_COLS {
        Breakpoint::Narrow
    } else {
        Breakpoint::Full
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // The status line is `App`'s, not the project list's, and it is drawn
        // in every mode: the help hint has to survive a day with no project
        // list at all, which is the one screen a new user is most likely to
        // meet first.
        let [main_area, status_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

        match self.mode {
            // Classified from the whole terminal, not `main_area`: the
            // notice a `TooSmall` terminal shows names a size that, once
            // resized to, actually clears the gate.
            Mode::Day => self.render_day(breakpoint(area), main_area, buf),
            Mode::ZoomedWeek => self.render_zoomed_week(main_area, buf),
            Mode::Week => self.render_week(main_area, buf),
            Mode::RawFile => self.render_raw_file(main_area, buf),
        }
        self.render_status(status_area, buf);
        // Drawn last, and in every mode: the help popup used to be skipped
        // entirely while the bar chart was zoomed.
        self.render_overlay(area, buf);
    }
}

impl App {
    /// The day view: calendar and weekly bar chart above the project list,
    /// reshaped by `bp` to fit whatever room the terminal has.
    fn render_day(&mut self, bp: Breakpoint, area: Rect, buf: &mut Buffer) {
        if bp == Breakpoint::TooSmall {
            render_too_small_notice(&self.ctx.theme, area, buf);
            return;
        }

        // `Compact` drops the calendar/chart header entirely: on a short
        // terminal the project list is what the user opened the app to
        // read, and a header it can't afford is worse than no header.
        let header_height = if bp == Breakpoint::Compact { 0 } else { 12 };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(header_height), Constraint::Min(9)].as_ref())
            .split(area);

        if header_height > 0 {
            self.render_day_header(bp, chunks[0], buf);
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

    /// The calendar/chart band above the project list.
    ///
    /// `Narrow` drops the calendar so the chart alone gets the header's full
    /// width; otherwise the two sit side by side, the pair capped at
    /// `CALENDAR_COLS + MAX_CHART_COLS` and centred so the chart doesn't
    /// stretch into a smear on a very wide terminal. Only ever called with
    /// `Narrow` or `Full` — `render_day` handles `TooSmall` and `Compact`
    /// itself before this runs.
    ///
    /// Draws the surrounding `tt-tui` block last: it paints over row 0 and
    /// the rightmost column of whatever area it's given (Task 23 hit this
    /// with a `title_top`), so it has to come after the widgets it wraps.
    fn render_day_header(&mut self, bp: Breakpoint, area: Rect, buf: &mut Buffer) {
        if bp == Breakpoint::Narrow {
            self.weekly_bar_chart().render(area, buf);
            draw_header_border(area, buf);
            return;
        }

        let content_area = center_capped(area, CALENDAR_COLS + MAX_CHART_COLS);
        let header_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(CALENDAR_COLS), Constraint::Fill(1)])
            .split(content_area);
        let calendar_area = header_area[0];
        let bar_chart_area = header_area[1];

        Calendar::new(self.active_date, &self.populated_dates, &self.ctx.theme)
            .render(calendar_area, buf);
        self.weekly_bar_chart().render(bar_chart_area, buf);

        if let Some(group_rect) = bounding_rect(&header_area) {
            draw_header_border(group_rect, buf);
        }
    }

    /// The weekly bar chart, full screen.
    fn render_zoomed_week(&mut self, area: Rect, buf: &mut Buffer) {
        self.weekly_bar_chart().render(area, buf);
    }

    /// The week's per-project rollup: the billing question the bar chart
    /// only teases.
    ///
    /// Laid out exactly like [`App::render_day`]'s project pane — one
    /// bordered block, and either the content or a one-line reason there
    /// isn't any — so the week the header names is always the week the
    /// numbers under it describe. [`WeekPane`] is what decides which.
    fn render_week(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(format_week_title(&self.week_dates))
            .border_type(BorderType::Rounded);

        // Read before the rollup is borrowed, the same ordering
        // `render_day` uses for `DayPane`.
        match self.week_pane() {
            WeekPane::Projects => {
                let inner = block.inner(area);
                block.render(area, buf);
                // Field-by-field, so the borrow checker can see that the
                // rollup being drawn and the selection being moved are two
                // different fields of `self`.
                if let Some(summary) = &self.weekly_summary {
                    StatefulWidget::render(
                        WeekListWidget::new(summary, &self.ctx.theme),
                        inner,
                        buf,
                        &mut self.week_list,
                    );
                }
            }
            WeekPane::Loading => {
                render_pane_message(LOADING_MESSAGE, self.ctx.theme.status, block, area, buf);
            }
            WeekPane::Empty => {
                render_pane_message(EMPTY_WEEK_TEXT, self.ctx.theme.warning, block, area, buf);
            }
        }
    }

    /// The active date's file exactly as it sits on disk.
    ///
    /// Records `raw_visible_lines` from `area` before drawing, computed the
    /// same way [`RawFileView::render`] lays the pane out — see
    /// [`RawFileView::visible_lines`] — then re-clamps `raw_scroll` against
    /// it via `App::clamp_raw_scroll`: date navigation and the mtime watcher
    /// can both replace `raw_content` while this mode is on screen, neither
    /// goes through `App::scroll_raw_file`, and an unclamped offset past a
    /// shorter file's end renders as a blank pane rather than a shorter one.
    fn render_raw_file(&mut self, area: Rect, buf: &mut Buffer) {
        self.raw_visible_lines = RawFileView::visible_lines(area);
        self.clamp_raw_scroll();
        let path = raw_file_path(&self.ctx.data_dir, self.active_date);
        RawFileView::new(
            &path,
            self.raw_content.as_deref(),
            self.raw_scroll,
            &self.ctx.theme,
        )
        .render(area, buf);
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

/// What the weekly rollup says for a week that loaded and turned out to
/// have nothing in it. Deliberately not the day pane's wording: "no data
/// found for date" under a week's header reads as a failed load.
const EMPTY_WEEK_TEXT: &str = "No tracked time this week";

/// A one-line message where the project list would be.
fn render_pane_message(text: &str, style: Style, block: Block<'_>, area: Rect, buf: &mut Buffer) {
    Paragraph::new(text)
        .block(block)
        .style(style)
        .alignment(Alignment::Left)
        .render(area, buf);
}

/// Drawn instead of the day view when the terminal is smaller than
/// `MIN_COLS`x`MIN_ROWS`: below that floor there isn't room to lay out the
/// calendar, chart and project list without corrupting all three, so this
/// says so instead of drawing a broken screen.
fn render_too_small_notice(theme: &Theme, area: Rect, buf: &mut Buffer) {
    const LINES: u16 = 2;

    let message = format!("Terminal too small.\nResize to at least {MIN_COLS}x{MIN_ROWS}.");
    let [notice_area] = Layout::vertical([Constraint::Length(LINES.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    Paragraph::new(message)
        .style(theme.warning)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .render(notice_area, buf);
}

/// The `tt-tui` block wrapped around the calendar/chart header. Must be
/// drawn after the widgets it surrounds — see `App::render_day_header`.
fn draw_header_border(area: Rect, buf: &mut Buffer) {
    Block::default()
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .title("tt-tui")
        .render(area, buf);
}

/// Centres a `max_width`-wide (or narrower, if `area` itself is) band
/// horizontally within `area`, leaving its height untouched.
fn center_capped(area: Rect, max_width: u16) -> Rect {
    let width = area.width.min(max_width);
    let [centered] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    centered
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
    format!("{} {}", date.weekday().short_name(), iso(date))
}

/// The weekly rollup pane's title: the week it covers, e.g.
/// `"Week of 2026-08-22 to 2026-08-28"`.
///
/// The rollup is a billing artefact, so the week it belongs to has to be on
/// screen next to the hours — `week_dates` is already ordered from the
/// configured start-of-week, so its ends are the range.
fn format_week_title(week_dates: &[Date; 7]) -> String {
    let [first, .., last] = *week_dates;
    format!("Week of {} to {}", iso(first), iso(last))
}

/// `date` in ISO form, falling back to [`Date`]'s own rendering if
/// formatting ever fails, rather than unwrapping and panicking the render
/// loop over a display string.
fn iso(date: Date) -> String {
    date.format(DATE_FORMAT)
        .unwrap_or_else(|_| date.to_string())
}

/// The path `date`'s file would have under `data_dir`, for
/// [`RawFileView`]'s title.
///
/// Mirrors `DataService::get_file_path` rather than calling it: that method
/// is `async` for symmetry with the disk-touching paths on `DataService`,
/// but resolves nothing beyond what is already sitting in `data_dir` for a
/// TUI service — always built with a fixed directory — so recomputing here
/// keeps the render path synchronous.
fn raw_file_path(data_dir: &Path, date: Date) -> PathBuf {
    data_dir.join(format!("{}.md", iso(date)))
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
    use crate::tui::testing::{
        fixture_date, fixture_day, fixture_day_with_projects, render_to_string,
    };

    fn day_app() -> App {
        App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(fixture_day())
    }

    #[test]
    fn breakpoints_are_chosen_by_size() {
        assert_eq!(breakpoint(Rect::new(0, 0, 50, 20)), Breakpoint::TooSmall);
        assert_eq!(breakpoint(Rect::new(0, 0, 100, 12)), Breakpoint::TooSmall);
        assert_eq!(breakpoint(Rect::new(0, 0, 80, 20)), Breakpoint::Compact);
        assert_eq!(breakpoint(Rect::new(0, 0, 80, 30)), Breakpoint::Narrow);
        assert_eq!(breakpoint(Rect::new(0, 0, 120, 30)), Breakpoint::Full);
    }

    /// Below the floor, the notice has to name the floor: a user staring at
    /// a blank pane with no numbers on it has no idea how far to resize.
    #[tokio::test]
    async fn a_tiny_terminal_gets_a_notice_naming_the_required_size() {
        let mut app = day_app();
        let screen = render_to_string(&mut app, 50, 10);
        assert!(
            screen.contains("60"),
            "the notice names the required width:\n{screen}"
        );
        assert!(
            screen.contains("15"),
            "the notice names the required height:\n{screen}"
        );
    }

    #[tokio::test]
    async fn a_narrow_terminal_drops_the_calendar_for_the_chart() {
        let mut app = day_app();
        let narrow = render_to_string(&mut app, 80, 30);
        let wide = render_to_string(&mut app, 140, 30);
        // The calendar renders a " Su Mo Tu We Th Fr Sa" weekday header row;
        // it should be gone when narrow. "Su" alone also matches "Project
        // Summaries" in the pane title below, so match the pair of days.
        assert!(
            wide.contains("Su Mo"),
            "the wide layout keeps the calendar:\n{wide}"
        );
        assert!(
            !narrow.contains("Su Mo"),
            "the narrow layout drops it:\n{narrow}"
        );
    }

    #[tokio::test]
    async fn a_short_terminal_keeps_the_project_list_usable() {
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(fixture_day_with_projects(6));
        let screen = render_to_string(&mut app, 100, 20);
        let listed = ["project-00", "project-01", "project-02"]
            .iter()
            .filter(|p| screen.contains(**p))
            .count();
        assert!(
            listed >= 3,
            "the collapsed chart band must give the list room:\n{screen}"
        );
    }

    /// Cheap insurance: ratatui panics on some zero-width layout arithmetic,
    /// and this task's constraints are exactly the kind of place that can
    /// produce it. Swept across every `Mode` and both overlay states, not
    /// just `Day`: `Day` is the one mode `breakpoint` protects below the
    /// floor, so it is the *least* likely of the four to hit a degenerate
    /// rectangle — restricting the sweep to it would cover the safest case
    /// and miss the others entirely (as it did in fix round 1, where the
    /// zoomed chart's total-hours overlay had a real mislocation bug at a
    /// narrow width no test here ever rendered).
    #[tokio::test]
    async fn no_render_panics_at_any_plausible_size() {
        for (w, h) in [
            (1, 1),
            (10, 3),
            (40, 10),
            (60, 15),
            (80, 24),
            (200, 60),
            (400, 100),
        ] {
            for mode in [Mode::Day, Mode::Week, Mode::ZoomedWeek, Mode::RawFile] {
                for overlay in [None, Some(Overlay::Help)] {
                    let mut app = day_app();
                    app.mode = mode;
                    app.overlay = overlay;
                    let _ = render_to_string(&mut app, w, h);
                }
            }
        }
    }

    /// Pins the property `breakpoint` exists to guarantee: at exactly the
    /// floor it names, the day view must actually clear the gate. A notice
    /// reading "resize to at least 60x15" while sitting at 60x15 would be
    /// self-contradicting — this was verified by hand during Task 24 and
    /// then discarded; a future change to `MIN_ROWS`, `MIN_COLS`, or the
    /// status-line split could silently reintroduce it without this test.
    #[tokio::test]
    async fn the_exact_floor_size_does_not_trigger_the_too_small_notice() {
        let mut app = day_app();
        let screen = render_to_string(&mut app, MIN_COLS, MIN_ROWS);
        assert!(
            !screen.contains("Terminal too small"),
            "60x15 is the advertised minimum, not below it:\n{screen}"
        );
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

    /// The rollup takes the whole screen, the way the zoomed chart does:
    /// the week is the question being asked, and the day's projects
    /// alongside it would only invite reading one week's hours off the
    /// other pane.
    #[test]
    fn the_week_rollup_replaces_the_day_view_rather_than_sharing_it() {
        let mut app = day_app();
        app.mode = Mode::Week;
        let screen = render_to_string(&mut app, 100, 30);
        assert!(
            !screen.contains("Project Summaries"),
            "the day's list must be gone:\n{screen}"
        );
        assert!(screen.contains("Week of "), "got:\n{screen}");
    }

    /// The point of the task: a day file that fences or parses to zero
    /// entries is no longer a dead end — the raw text is right there.
    #[tokio::test]
    async fn v_enters_raw_mode_and_shows_the_file_text() {
        let mut app = App::new(TuiContext::for_test());
        app.raw_content = Some("```timetracking\n8-10 admin\n```".into());
        app.mode = Mode::RawFile;
        let screen = render_to_string(&mut app, 80, 20);
        assert!(screen.contains("8-10 admin"), "got:\n{screen}");
    }

    /// A missing file is not the same as a load still in flight, and the
    /// raw view has to say so rather than drawing an empty box.
    #[tokio::test]
    async fn a_missing_file_says_so_rather_than_rendering_blank() {
        let mut app = App::new(TuiContext::for_test());
        app.raw_content = None;
        app.mode = Mode::RawFile;
        let screen = render_to_string(&mut app, 80, 20);
        assert!(screen.contains("No file"), "got:\n{screen}");
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
