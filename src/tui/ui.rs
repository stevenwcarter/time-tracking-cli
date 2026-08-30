use std::path::{Path, PathBuf};

use ratatui::layout::Flex;
use ratatui::prelude::*;
use ratatui::widgets::*;
use time::Date;

use crate::{DATE_FORMAT, time_utils::WeekdayExt};

use super::app::{App, DayPane, EmptyReason, LOADING_MESSAGE, WeekPane};
use super::mode::{Mode, Overlay};
use super::project_list::{MIN_ROWS_FOR_TWO_PROJECTS, USABLE_PROJECTS};
use super::theme::Theme;
use super::week_list::WeekListWidget;
use super::widgets::HelpPopup;
use super::widgets::{Calendar, DatePrompt, RawFileContent, RawFileView, WeeklyBarChart};

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

/// Rows the calendar/chart band claims in the day view when it is drawn at
/// all. Fixed: the calendar is six week rows plus its month and weekday
/// headers, and the chart is sized to sit beside it.
const HEADER_BAND_ROWS: u16 = 12;

/// Rows the day pane's border costs, top and bottom.
const PANE_BORDER_ROWS: u16 = 2;

/// Columns the day pane's border costs, left and right.
const PANE_BORDER_COLS: u16 = 2;

/// Below this many rows the calendar/chart header stops earning its space,
/// whatever the day holds.
///
/// The size-only half of the rule; [`App::band_is_affordable`] carries the
/// half that depends on the data, and is what a day with a project list is
/// actually judged by. This floor decides the case that has no list to
/// measure — an empty day would otherwise hand the band twelve of its
/// fifteen rows and squeeze its one-line notice into what was left.
///
/// Derived rather than chosen, because choosing it is how this went wrong:
/// at 22 the band cost 12 of the terminal's rows and left the project list
/// below it too short to draw a single project, so dragging a window from
/// 21 rows to 22 made the day's work *disappear*.
const COMPACT_ROWS: u16 = 1 // App's status line, outside the day view entirely
    + HEADER_BAND_ROWS
    + PANE_BORDER_ROWS
    + MIN_ROWS_FOR_TWO_PROJECTS;

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
        // A region not drawn this frame must not be hittable next frame.
        self.layout.clear();

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

        // The calendar/chart header is dropped entirely unless it is free:
        // on a short terminal the project list is what the user opened the
        // app to read, and a header it can't afford is worse than no header.
        let header_height = if self.band_is_affordable(bp, area) {
            HEADER_BAND_ROWS
        } else {
            0
        };
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
                self.layout.project_list = Some(inner);
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
            DayPane::Empty(reason) => {
                let text = match reason {
                    EmptyReason::NoFile => NO_FILE_TEXT,
                    EmptyReason::FileWithNoEntries => FILE_WITH_NO_ENTRIES_TEXT,
                    EmptyReason::Unreadable => UNREADABLE_TEXT,
                };
                render_call_to_action(text, self.ctx.theme.warning, block, chunks[1], buf);
            }
        }
    }

    /// Can the calendar/chart band be drawn in `area` without costing the
    /// project list a single row it would otherwise have used?
    ///
    /// The band is a fixed [`HEADER_BAND_ROWS`], so the terminal row that
    /// takes the layout across the threshold buys eleven rows *less* list
    /// than the row below it had. No threshold on `area` alone can judge
    /// that, because whether those rows were carrying anything depends on
    /// the day, not on the terminal: `COMPACT_ROWS = 22` blanked the pane
    /// outright, and a `COMPACT_ROWS` derived for a clean day still dropped
    /// a three-project day to one as soon as a parser warning appeared. So
    /// the cost is measured against the day.
    ///
    /// What it is measured against is [`USABLE_PROJECTS`], not every
    /// project the day has, and the difference is a deliberate trade:
    ///
    /// - The list **scrolls**. Rendering fewer projects at once is not data
    ///   loss — `j`/`k` reach the rest. Rendering *none* was, and that is
    ///   the Critical this rule descends from; it is pinned separately.
    /// - Requiring every project to fit was tried and rejected. It gives
    ///   strict monotonicity — a taller terminal never renders fewer
    ///   projects — but it costs a twelve-project day its calendar and
    ///   chart until roughly 56 rows, which is a worse product than the
    ///   wart it removes.
    ///
    /// So this **accepts a non-monotonic step**: crossing the threshold can
    /// show fewer projects than the row below it did, while gaining the
    /// band. That is a decision, not an oversight. What it will not do is
    /// leave the list below a usable two — or below every project the day
    /// has, when it has fewer than two.
    ///
    /// [`Breakpoint::Compact`] still gates it as a hard floor, for the days
    /// with no list to crowd out; see [`COMPACT_ROWS`].
    ///
    /// [`USABLE_PROJECTS`]: super::project_list::USABLE_PROJECTS
    fn band_is_affordable(&self, bp: Breakpoint, area: Rect) -> bool {
        if bp == Breakpoint::Compact {
            return false;
        }
        let Some(widget) = self
            .project_list_widget
            .as_ref()
            .filter(|_| self.day_pane() == DayPane::Projects)
        else {
            return true;
        };
        let pane_inner_width = area.width.saturating_sub(PANE_BORDER_COLS);
        let needed = HEADER_BAND_ROWS
            .saturating_add(PANE_BORDER_ROWS)
            .saturating_add(widget.rows_to_show(pane_inner_width, USABLE_PROJECTS));
        area.height >= needed
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
            self.layout.bar_chart = Some(area);
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
        self.layout.calendar = Some(calendar_area);
        self.layout.bar_chart = Some(bar_chart_area);

        if let Some(group_rect) = bounding_rect(&header_area) {
            draw_header_border(group_rect, buf);
        }
    }

    /// The weekly bar chart, full screen.
    fn render_zoomed_week(&mut self, area: Rect, buf: &mut Buffer) {
        self.weekly_bar_chart().render(area, buf);
        self.layout.bar_chart = Some(area);
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
                // `week_is_stale` is re-read here rather than inferred from
                // the arm. `WeekPane::Projects` does imply a fresh rollup,
                // but this is the one place the rollup is read for drawing,
                // and a read that reaches it without the guard is exactly
                // how the previous week's hours end up under this week's
                // header. Field-by-field so the borrow checker can see the
                // rollup and the selection are different fields of `self`.
                let stale = self.week_is_stale();
                if let Some(summary) = self.weekly_summary.as_ref().filter(|_| !stale) {
                    StatefulWidget::render(
                        WeekListWidget::new(summary, &self.ctx.theme),
                        inner,
                        buf,
                        &mut self.week_list,
                    );
                    self.layout.week_list = Some(inner);
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
        // The title comes from `active_date` and `raw_content` describes
        // `loaded_date`, so the gate every other pane applies is owed here
        // too — and owed hardest, because this is the pane a user reaches
        // for to establish ground truth about a file. See `App::day_is_stale`.
        let content = match self.raw_content.as_deref() {
            _ if self.day_is_stale() => RawFileContent::Unknown,
            Some(text) => RawFileContent::Text(text),
            None => RawFileContent::Missing,
        };
        RawFileView::new(&path, content, self.raw_scroll, &self.ctx.theme).render(area, buf);
        self.layout.raw_file = Some(area);
    }

    /// Draw the modal layer, if one is open, over whatever the mode drew.
    fn render_overlay(&mut self, area: Rect, buf: &mut Buffer) {
        match &self.overlay {
            Some(Overlay::Help) => {
                let popup = HelpPopup::new(&self.ctx.theme, self.mode);
                self.layout.overlay = Some(popup.popup_rect(area));
                popup.render(area, buf);
            }
            Some(Overlay::DatePrompt(input)) => {
                self.layout.overlay = Some(DatePrompt::popup_rect(area));
                DatePrompt::new(&self.ctx.theme, input).render(area, buf);
            }
            None => {}
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
    ///
    /// `pub(super)` rather than private: this is the single assembly point
    /// for the chart, and [`App::handle_click`](super::app::App::handle_click)'s
    /// bar-chart hit-test has to build the identical instance `render` does —
    /// otherwise the two could silently drift apart (a variable-width bar, a
    /// legend column) with nothing to catch it, since `date_at` reads its
    /// geometry off whatever instance it is called on. Narrowing this back to
    /// private reopens that hole; both call sites must keep going through it.
    pub(super) fn weekly_bar_chart(&self) -> WeeklyBarChart<'_> {
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
    fn render_status(&mut self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.footer_text())
            .style(self.ctx.theme.status)
            .wrap(Wrap { trim: true })
            .centered()
            .render(area, buf);
        self.layout.help_hint = Some(area);
    }
}

/// What the day pane says when there is no file at all for the date on
/// screen — the first screen a new user sees, and the one every weekend and
/// future date lands on too. Names the two keys that move forward: `e` to
/// start writing this date's file, `t` to bail back to a date that might
/// already have one.
const NO_FILE_TEXT: &str =
    "No file for this day yet. press e to create and edit it · press t for today";

/// What the day pane says when a file exists but the parser recognised no
/// time entries in it — distinct from [`NO_FILE_TEXT`], and pointed at `v`:
/// the raw text is still there to read even though the project list has
/// nothing to show.
const FILE_WITH_NO_ENTRIES_TEXT: &str = "This file has no time entries the parser recognised. press v to see the raw text · press e to edit";

/// What the day pane says for [`EmptyReason::Unreadable`] — the pane's only
/// route there is a load that failed, so this is the post-failed-load
/// state, and the only one of the three empty texts. Deliberately does not
/// restate the error: `App::apply_sync_event` already put `Load failed:
/// <message>` on the status line below, and this pane cannot honestly say
/// more about the file itself than "unknown".
const UNREADABLE_TEXT: &str = "Could not read this day. See the message below · press r to retry";

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

/// The day pane's empty-state call to action — see [`EmptyReason`]. Centred
/// and word-wrapped rather than [`render_pane_message`]'s left-aligned
/// single line: all three of [`NO_FILE_TEXT`], [`FILE_WITH_NO_ENTRIES_TEXT`]
/// and [`UNREADABLE_TEXT`] run well past [`MIN_COLS`], so they need to wrap
/// to stay readable on the narrowest terminal the day view supports at all.
fn render_call_to_action(text: &str, style: Style, block: Block<'_>, area: Rect, buf: &mut Buffer) {
    Paragraph::new(text)
        .block(block)
        .style(style)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
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
    use std::collections::HashMap;

    use time::Weekday;
    use time::macros::date;

    use time_tracking_parser::TimeTrackingData;

    use super::*;
    use crate::time_utils::get_week_dates;
    use crate::tui::context::TuiContext;
    use crate::tui::testing::{
        fixture_date, fixture_day, fixture_day_with_notes, fixture_day_with_projects,
        render_to_string,
    };
    use crate::tui::week_list::fixture_week_summary;

    fn day_app() -> App {
        App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(fixture_day())
    }

    /// `day_app` with a week's rollup landed on top, so `Mode::Week` draws
    /// the real pane rather than its empty state.
    fn week_app() -> App {
        day_app().with_weekly_summary(fixture_week_summary())
    }

    /// `day_app` with a week's chart data, calendar markers and a long,
    /// already-scrolled raw file landed on top, so the sweep reaches
    /// `WeeklyBarChart`'s real arithmetic — `bar_values`, `style_for`,
    /// `calculate_bar_dimensions`'s two divisions and `BarChart`'s own
    /// layout with actual `Bar`s — instead of the empty box an unseeded
    /// `weekly_data` produces.
    fn charted_app() -> App {
        let week: HashMap<Date, u32> = get_week_dates(&fixture_date(), Weekday::Saturday)
            .into_iter()
            .enumerate()
            .map(|(i, date)| (date, i as u32 * 137 + 1439))
            .collect();
        let mut app = day_app()
            .with_populated_dates(vec![fixture_date()])
            .with_weekly_data(week);
        app.raw_content = Some((0..300).map(|i| format!("line {i}\n")).collect());
        app.raw_scroll = 250;
        app
    }

    /// A completed load that found no file — `EmptyReason::NoFile`,
    /// exercising [`render_call_to_action`]'s centred wrap rather than
    /// [`day_app`]'s project list. `loaded_date` is set explicitly to mark
    /// the load as having actually landed; without it this would be
    /// `EmptyReason::Unreadable` instead — see [`EmptyReason`].
    fn empty_no_file_app() -> App {
        let mut app = App::new(TuiContext::for_test()).with_active_date(fixture_date());
        app.loaded_date = Some(app.active_date);
        app
    }

    /// A file that parsed to no entries — `EmptyReason::FileWithNoEntries`,
    /// the second text [`render_call_to_action`] has to wrap.
    /// `with_raw_content` sets `loaded_date` itself, via `with_data`.
    fn empty_file_with_no_entries_app() -> App {
        App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_raw_content("# just a heading, no entries\n")
    }

    /// A load that failed after navigating away from a populated day —
    /// `EmptyReason::Unreadable`, the third text [`render_call_to_action`]
    /// has to wrap. `active_date` is moved on without a matching
    /// `loaded_date`, exactly what `AppEvent::LoadFailed` leaves behind.
    fn empty_unreadable_app() -> App {
        let mut app = day_app();
        app.active_date = fixture_date().next_day().expect("a next day exists");
        app
    }

    #[test]
    fn breakpoints_are_chosen_by_size() {
        assert_eq!(breakpoint(Rect::new(0, 0, 50, 20)), Breakpoint::TooSmall);
        assert_eq!(breakpoint(Rect::new(0, 0, 100, 12)), Breakpoint::TooSmall);
        assert_eq!(breakpoint(Rect::new(0, 0, 80, 20)), Breakpoint::Compact);
        assert_eq!(breakpoint(Rect::new(0, 0, 80, 30)), Breakpoint::Narrow);
        assert_eq!(breakpoint(Rect::new(0, 0, 120, 30)), Breakpoint::Full);
    }

    /// One column or one row short of the floor, on its own, must still trip
    /// the notice. Existing coverage pins the floor itself (clean) and a
    /// size well below it (too small); neither would notice `breakpoint`'s
    /// `<` comparisons drifting by one, since a size well below the floor
    /// stays `TooSmall` under either operator. This is the adjacent value
    /// that actually distinguishes them.
    #[test]
    fn one_less_than_the_floor_in_either_dimension_is_too_small() {
        assert_eq!(
            breakpoint(Rect::new(0, 0, MIN_COLS - 1, MIN_ROWS)),
            Breakpoint::TooSmall,
            "one column short of the floor must still trip the notice"
        );
        assert_eq!(
            breakpoint(Rect::new(0, 0, MIN_COLS, MIN_ROWS - 1)),
            Breakpoint::TooSmall,
            "one row short of the floor must still trip the notice"
        );
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

    /// Regression: the chart band's `Length(12)` plus an uncapped warnings
    /// header can starve the list below ratatui's "an item taller than the
    /// viewport renders nothing" threshold, blanking the pane entirely.
    ///
    /// Three separate triggers, one root cause, and all three sit at sizes a
    /// user actually has: 80x22 showed *less* than 80x21, and 80x24 — the
    /// most common terminal size there is — was emptied by a single parser
    /// warning or by a fourth note on the day's only project.
    #[tokio::test]
    async fn the_project_list_survives_the_sizes_a_user_actually_has() {
        // (a) one row over `COMPACT_ROWS` must not show *less* than one row
        // under it: the layout has to be monotonic in height.
        let mut app = day_app();
        let screen = render_to_string(&mut app, 80, 22);
        assert!(
            screen.contains("admin"),
            "80x22 lost the whole list:\n{screen}"
        );

        // (b) a single parser warning must not cost the entire list.
        let mut data = fixture_day();
        data.warnings = vec!["Error parsing time range '9-'".to_owned()];
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(data);
        let screen = render_to_string(&mut app, 80, 24);
        assert!(
            screen.contains("admin"),
            "one warning blanked the list:\n{screen}"
        );

        // (c) a project with four notes is an ordinary day, not an overflow.
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(fixture_day_with_notes("solo", 4));
        let screen = render_to_string(&mut app, 80, 24);
        assert!(
            screen.contains("solo"),
            "a 4-note project blanked the list:\n{screen}"
        );
    }

    /// `data` with `n` parser warnings attached.
    fn with_warnings(mut data: TimeTrackingData, n: usize) -> TimeTrackingData {
        data.warnings = (0..n).map(|i| format!("bad entry {i}")).collect();
        data
    }

    /// `data` with every project carrying `notes` notes instead of two, so
    /// its list items are taller than the three-row typical case.
    fn with_more_notes(mut data: TimeTrackingData, notes: usize) -> TimeTrackingData {
        for (i, project) in data.projects.iter_mut().enumerate() {
            project.notes = (0..notes).map(|n| format!("note {i}-{n}")).collect();
        }
        data
    }

    /// Once the terminal is tall enough to render a usable list, growing it
    /// never takes that away: from the first height that shows
    /// [`USABLE_PROJECTS`] projects — or all of them, when the day has fewer
    /// — every taller height shows at least as many.
    ///
    /// Deliberately *not* strict monotonicity. That was the first
    /// formulation and it was too strong: it holds only if the band waits
    /// for every project to fit, which costs a twelve-project day its
    /// calendar and chart until roughly 56 rows. The list scrolls, so
    /// rendering fewer projects at once is not data loss; rendering *none*
    /// is, and that is pinned separately by
    /// `the_project_list_survives_the_sizes_a_user_actually_has`. See
    /// `App::band_is_affordable` for the trade this settles on.
    ///
    /// Nor is the floor asserted unconditionally from [`MIN_ROWS`] up, which
    /// was the second formulation. Below about twenty rows a day carrying a
    /// warnings block genuinely cannot fit both it and two three-row
    /// projects — 80x15 leaves the list five rows — and that is arithmetic
    /// the band has no part in: every such height renders no band at all.
    /// Demanding two projects there would only buy them by dropping the
    /// warnings block, which is a correctness signal and the worse trade.
    /// What must never happen is *losing* a usable list by growing the
    /// window, which is the shape all three findings in this class took.
    #[tokio::test]
    async fn growing_the_terminal_never_takes_back_a_usable_list() {
        let cases = [
            ("a clean day", fixture_day_with_projects(4)),
            (
                "one parser warning",
                with_warnings(fixture_day_with_projects(4), 1),
            ),
            (
                "a flood of warnings",
                with_warnings(fixture_day_with_projects(4), 20),
            ),
            (
                "multi-note projects",
                with_more_notes(fixture_day_with_projects(4), 5),
            ),
            // Exercises the `min`: a one-project day owes one, not two.
            ("a single project", fixture_day_with_projects(1)),
        ];

        for (label, data) in cases {
            let names: Vec<String> = data.projects.iter().map(|p| p.name.clone()).collect();
            let floor = names.len().min(USABLE_PROJECTS);
            // Both sides of `NARROW_COLS`, so the band is swept with the
            // calendar beside the chart as well as without it.
            for width in [80, 120] {
                let mut reached = None;
                for height in MIN_ROWS..=44 {
                    let mut app = App::new(TuiContext::for_test())
                        .with_active_date(fixture_date())
                        .with_data(data.clone());
                    let screen = render_to_string(&mut app, width, height);
                    let visible = names.iter().filter(|n| screen.contains(*n)).count();

                    let Some(first) = reached else {
                        if visible >= floor {
                            reached = Some(height);
                        }
                        continue;
                    };
                    assert!(
                        visible >= floor,
                        "{label}: {width}x{height} renders {visible} projects, back below \
                         the usable floor of {floor} it first cleared at {width}x{first}:\n\
                         {screen}"
                    );
                }
                assert!(
                    reached.is_some(),
                    "{label}: {width} columns never rendered {floor} projects at any swept \
                     height, so this proves nothing"
                );
            }
        }
    }

    /// The same floor stated against the band specifically, so a regression
    /// says *which* seam broke: wherever the band is drawn, the pane below
    /// it still shows its whole header and a usable list. The sweep above
    /// would catch this too, but it would not name the band as the cause.
    #[tokio::test]
    async fn wherever_the_band_is_drawn_the_list_is_still_usable() {
        let mut band_seen = false;
        for height in COMPACT_ROWS..=44 {
            let mut app = day_app();
            let screen = render_to_string(&mut app, 80, height);
            if !screen.contains("tt-tui") {
                continue;
            }
            band_seen = true;

            assert!(
                screen.contains("Working Time"),
                "80x{height} clipped the header the band was preferred over:\n{screen}"
            );
            let visible = ["admin", "client-bd", "internal"]
                .iter()
                .filter(|name| screen.contains(**name))
                .count();
            assert!(
                visible >= USABLE_PROJECTS,
                "80x{height} drew the band over a list of only {visible}:\n{screen}"
            );
        }
        assert!(
            band_seen,
            "the band has to be drawn somewhere in the swept range, or this proves nothing"
        );
    }

    /// Cheap insurance: ratatui panics on some zero-width layout arithmetic,
    /// and this task's constraints are exactly the kind of place that can
    /// produce it. Swept across every `Mode` and every overlay state, not
    /// just `Day`: `Day` is the one mode `breakpoint` protects below the
    /// floor, so it is the *least* likely of the four to hit a degenerate
    /// rectangle — restricting the sweep to it would cover the safest case
    /// and miss the others entirely (as it did in fix round 1, where the
    /// zoomed chart's total-hours overlay had a real mislocation bug at a
    /// narrow width no test here ever rendered).
    ///
    /// Swept over two *fixtures* as well, and that is load-bearing rather
    /// than thorough: `day_app` leaves `weekly_summary` at `None`, so
    /// `Mode::Week` falls to `WeekPane::Empty` and renders a one-line
    /// notice — which made this sweep's coverage of the weekly rollup
    /// vacuous. `week_app` lands a rollup, so `WeekListWidget`'s own
    /// layout — the header split, the `List` with `scroll_padding`, and
    /// `project_row`'s width arithmetic — is what gets rendered at 1x1.
    ///
    /// `Overlay::DatePrompt` carries a non-empty buffer here rather than
    /// `String::new()`: an empty prompt would still exercise the popup's
    /// border and title, but not the cursor span drawn one cell past the
    /// typed text, which is exactly the arithmetic most likely to run past
    /// a narrow buffer's edge.
    ///
    /// `empty_no_file_app`, `empty_file_with_no_entries_app` and
    /// `empty_unreadable_app` cover the task's own new arithmetic:
    /// `render_call_to_action`'s centred wrap, all three of its texts,
    /// which `day_app` and `week_app` never reach at all.
    ///
    /// `charted_app` is the third fixture added for the same reason as
    /// `week_app`, on the axis that had gone vacuous next: none of the
    /// others calls `with_weekly_data`, so `prepare_bars` early-returned an
    /// empty vector and `Mode::ZoomedWeek` swept an empty box at every
    /// size — leaving the widget with the most arithmetic on the branch
    /// unswept. It seeds a scrolled raw file too, so `render_raw_file`'s
    /// `visible_lines`/`clamp_raw_scroll` path is exercised somewhere other
    /// than its trivial state.
    #[tokio::test]
    async fn no_render_panics_at_any_plausible_size() {
        for build in [
            day_app as fn() -> App,
            week_app as fn() -> App,
            charted_app as fn() -> App,
            empty_no_file_app as fn() -> App,
            empty_file_with_no_entries_app as fn() -> App,
            empty_unreadable_app as fn() -> App,
        ] {
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
                    for overlay in [
                        None,
                        Some(Overlay::Help),
                        Some(Overlay::DatePrompt("2026-08-14".to_owned())),
                    ] {
                        let mut app = build();
                        app.mode = mode;
                        app.overlay = overlay;
                        let _ = render_to_string(&mut app, w, h);
                    }
                }
            }
        }
    }

    /// The same guard for the chart axis, and the half of this fix that
    /// stops the hole reopening: this branch has now caught the sweep going
    /// vacuous twice, once for the week rollup and once for the bar chart,
    /// and both times the fixture was there while nothing checked it still
    /// reached the widget. Bars at a size the sweep actually renders is
    /// what makes `charted_app` more than decoration.
    #[tokio::test]
    async fn the_sweeps_chart_fixture_really_renders_bars() {
        let mut app = charted_app();
        app.mode = Mode::ZoomedWeek;
        let screen = render_to_string(&mut app, 80, 24);

        assert!(
            screen.contains('\u{2588}'),
            "the sweep must not draw an empty chart:\n{screen}"
        );
        assert!(
            !screen.contains("0.0h total"),
            "nor total an empty week:\n{screen}"
        );
    }

    /// Guards the sweep above from silently going vacuous again: at a size
    /// it actually sweeps, `week_app` has to reach `WeekListWidget` rather
    /// than the empty-state notice `day_app` gets.
    #[tokio::test]
    async fn the_sweeps_week_fixture_really_renders_the_rollup() {
        let mut app = week_app();
        app.mode = Mode::Week;
        let screen = render_to_string(&mut app, 80, 24);
        assert!(screen.contains("client-bd"), "got:\n{screen}");
        assert!(
            !screen.contains(EMPTY_WEEK_TEXT),
            "the sweep must not be seeded into the empty state:\n{screen}"
        );
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
        // The content has to describe the date in the title before the pane
        // will draw it; see `the_raw_pane_does_not_show_the_previous_dates_file`.
        app.loaded_date = Some(app.active_date);
        let screen = render_to_string(&mut app, 80, 20);
        assert!(screen.contains("8-10 admin"), "got:\n{screen}");
    }

    /// The raw pane is the diagnostic escape hatch — its whole contract is
    /// "this file, exactly as it sits on disk" — so it is the worst place to
    /// draw one date's bytes under another date's path. The title came from
    /// `active_date` and the body from `raw_content`, which describes
    /// `loaded_date`; `l` in `Mode::RawFile` put them out of step, and a
    /// failed load left them that way. Nor may it claim the file is missing:
    /// it does not know yet.
    #[tokio::test]
    async fn the_raw_pane_does_not_show_the_previous_dates_file() {
        let mut app = day_app();
        app.raw_content = Some("9:00-10:00 admin\n".to_owned());
        app.mode = Mode::RawFile;
        app.loaded_date = Some(app.active_date);
        app.active_date = app.active_date.next_day().expect("a next day exists");
        app.loading = true;

        let screen = render_to_string(&mut app, 80, 24);
        assert!(
            !screen.contains("admin"),
            "the previous date's file must not be drawn under the new date's path:\n{screen}"
        );
        assert!(
            !screen.contains("No file"),
            "nor may an unloaded date be reported as a file-less one:\n{screen}"
        );
    }

    /// A missing file is not the same as a load still in flight, and the
    /// raw view has to say so rather than drawing an empty box.
    #[tokio::test]
    async fn a_missing_file_says_so_rather_than_rendering_blank() {
        let mut app = App::new(TuiContext::for_test());
        app.raw_content = None;
        app.mode = Mode::RawFile;
        // A load that landed and found nothing, not one that never ran —
        // only the first of those may be reported as a missing file.
        app.loaded_date = Some(app.active_date);
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

    /// The first screen a new user sees, and the one every weekend and
    /// future date lands on too: no file, no data, and — before this task —
    /// no way forward. `e` is the key that fixes that.
    ///
    /// `loaded_date` is set explicitly: a bare `with_active_date` leaves it
    /// unset, which `App::day_pane` reads as `EmptyReason::Unreadable`
    /// (nothing has actually loaded), not `NoFile`. See [`EmptyReason`].
    #[tokio::test]
    async fn no_file_offers_to_create_one() {
        let mut app = App::new(TuiContext::for_test()).with_active_date(date!(2026 - 08 - 30));
        app.loaded_date = Some(app.active_date);
        assert_eq!(app.day_pane(), DayPane::Empty(EmptyReason::NoFile));
        let screen = render_to_string(&mut app, 100, 30);
        assert!(screen.contains("Sun 2026-08-30"), "got:\n{screen}");
        // Not just `contains("press e")`: both `NO_FILE_TEXT` and
        // `FILE_WITH_NO_ENTRIES_TEXT` mention `e`, so that alone would pass
        // even with the two constants' bodies swapped.
        assert!(
            screen.contains("press e to create and edit it"),
            "got:\n{screen}"
        );
        assert!(
            !screen.contains("no time entries"),
            "must be the no-file text, not the file-with-no-entries text:\n{screen}"
        );
    }

    /// A file that exists but fences or parses to zero entries is not the
    /// same dead end as no file at all: the raw text is still there, via
    /// `v`. `with_raw_content` sets `loaded_date` itself, via `with_data`.
    #[tokio::test]
    async fn a_file_that_parses_to_nothing_says_so_and_points_at_v() {
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(date!(2026 - 08 - 30))
            .with_raw_content("# just a heading, no entries\n");
        assert_eq!(
            app.day_pane(),
            DayPane::Empty(EmptyReason::FileWithNoEntries)
        );
        let screen = render_to_string(&mut app, 100, 30);
        assert!(screen.contains("no time entries"), "got:\n{screen}");
        assert!(screen.contains("press v"), "got:\n{screen}");
    }

    // `the_help_hint_survives_a_day_with_no_project_list` in `app.rs` already
    // pins this exact regression (the hint surviving an empty day) with the
    // same literal `HELP_HINT` string; a second copy here would vary only
    // the fixture date and terminal size, neither of which the code path
    // depends on, so it was deleted rather than kept as a near-duplicate.
}
