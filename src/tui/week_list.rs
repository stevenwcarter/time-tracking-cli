//! The weekly per-project rollup: "how many hours did client-bd get this
//! week", answered without leaving the TUI.
//!
//! The pane borrows the [`WeeklySummary`] `App` already loads with every
//! other payload rather than copying it into items of its own — a second
//! copy could only drift from the one the bar chart and `Y`'s yank read —
//! so the widget is rebuilt every frame and the selection lives in
//! [`WeekListState`], which `App` holds across frames.

use ratatui::prelude::*;
use ratatui::widgets::*;
use time_tracking_parser::Time;
use unicode_width::UnicodeWidthStr;

use crate::data_svc::{WeeklyProject, WeeklySummary};

use super::band;
use super::event::AppEvent;
use super::mode::Handled;
use super::theme::Theme;

/// Rows the totals block above the list occupies: working time and dead
/// time, one line each.
const HEADER_ROWS: u16 = 2;

/// The marker drawn in front of the selected row.
const HIGHLIGHT_SYMBOL: &str = ">>";

/// Columns [`HIGHLIGHT_SYMBOL`] costs every row, reserved with
/// [`HighlightSpacing::Always`] so the rows don't shift sideways as the
/// selection moves. Subtracted from the pane width before a row's hours are
/// right-aligned against it, so the hours column lines up with the pane's
/// right edge rather than overshooting it.
const HIGHLIGHT_COLS: u16 = HIGHLIGHT_SYMBOL.len() as u16;

/// Shown in place of the week's dead time when it has none, mirroring the
/// CLI's `--week` output so the two never read differently for the same
/// week.
const NO_DEAD_TIME: &str = "None";

/// Rows the list keeps for itself whatever the warnings block asks for: its
/// own title bar plus one project row. Paired with [`HEADER_ROWS`] as the
/// floor passed to [`band::fit_band`] — see that module for why a band that
/// grows with the file has to be told what it may not take.
const MIN_LIST_ROWS: u16 = 2;

/// Columns the warnings block hangs its entries under the count by.
const WARNING_INDENT: &str = "  ";

/// The week's projects, biggest first, with the week's totals above them.
///
/// Borrows rather than owns; see the module docs.
pub struct WeekListWidget<'a> {
    summary: &'a WeeklySummary,
    theme: &'a Theme,
}

impl<'a> WeekListWidget<'a> {
    /// A pane over `summary`, drawn in `theme`.
    pub fn new(summary: &'a WeeklySummary, theme: &'a Theme) -> Self {
        Self { summary, theme }
    }

    /// The week's working and dead time, the two numbers a timesheet line
    /// is built from.
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let dead_minutes = self.summary.dead_time_minutes;
        let dead = if dead_minutes == 0 {
            NO_DEAD_TIME.to_owned()
        } else {
            format!(
                "{} ({} hours)",
                Time::format_duration_minutes(dead_minutes),
                Time::format_duration_decimal(dead_minutes),
            )
        };
        // Dead time is only worth drawing attention to when there is some;
        // a clean week's "None" is not a warning.
        let dead_style = if dead_minutes == 0 {
            self.theme.status
        } else {
            self.theme.warning
        };

        Paragraph::new(vec![
            // Both spellings, the same way the dead-time line and the CLI's
            // `--week` totals give both: `h:mm` is what a timesheet form
            // asks for and the decimal is what an invoice does.
            Line::from(format!(
                "Working Time: {} ({} hours)",
                Time::format_duration_minutes(self.summary.total_minutes),
                Time::format_duration_decimal(self.summary.total_minutes),
            )),
            Line::styled(format!("Dead Time: {dead}"), dead_style),
        ])
        .bold()
        .centered()
        .render(area, buf);
    }

    /// One row per project, in the order [`DataService::get_weekly_summary`]
    /// already sorted them: minutes descending, then name — which is the
    /// order the billing question is asked in.
    ///
    /// [`DataService::get_weekly_summary`]: crate::DataService::get_weekly_summary
    fn render_list(&self, area: Rect, buf: &mut Buffer, state: &mut WeekListState) {
        let block = Block::new()
            .title(Line::raw("Weekly Projects").centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(self.theme.list_header);

        let row_width = area.width.saturating_sub(HIGHLIGHT_COLS);
        let items: Vec<ListItem<'static>> = self
            .summary
            .projects
            .iter()
            .map(|project| ListItem::new(project_row(project, row_width)))
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(self.theme.selection)
            .highlight_symbol(HIGHLIGHT_SYMBOL)
            .highlight_spacing(HighlightSpacing::Always)
            .repeat_highlight_symbol(true)
            .scroll_padding(1);

        StatefulWidget::render(list, area, buf, &mut state.list);
    }

    /// The week's parser warnings, under a count, or nothing when the week
    /// parsed cleanly.
    ///
    /// This pane is the surface a week's hours are most likely to be pasted
    /// into a timesheet from, and it was the only one showing them with
    /// nothing said about the files they were parsed out of:
    /// [`ProjectListWidget`] renders a warnings block for a *day* and the
    /// CLI's `--week` prints `WEEKLY WARNINGS`, but from the TUI a week's
    /// day-level warnings were otherwise reachable only by visiting all
    /// seven days one at a time.
    ///
    /// Capped at [`band::MAX_WARNING_ROWS`]; the title carries the full
    /// count, so a week too badly parsed to list is still unmistakably
    /// flagged without the block eating the hours above it.
    ///
    /// [`ProjectListWidget`]: super::project_list::ProjectListWidget
    fn warning_lines(&self) -> Vec<Line<'static>> {
        band::warning_lines(&self.summary.warnings, self.theme.error, WARNING_INDENT)
    }
}

impl StatefulWidget for WeekListWidget<'_> {
    type State = WeekListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // The selection outlives any one week — `w` can be pressed again
        // after navigating to a shorter one — so it is brought back into
        // range here as well as on the way into `WeekListState::apply`.
        state.clamp(self.summary.projects.len());

        // Built once and threaded into both the layout and the render, the
        // way `ProjectListWidget` threads its working-time rows, so the two
        // can never disagree about how many rows the block took.
        let warnings = self.warning_lines();
        // The same rule the day pane applies to its header band, on the
        // week's other axis: two `Length`s in one layout have equal claim,
        // so a one-row warnings block was winning the last row off a
        // two-row totals header and the pane's whole reason to exist —
        // the hours — went with it. The block yields instead.
        let warning_rows = band::fit_band(
            u16::try_from(warnings.len()).unwrap_or(u16::MAX),
            area.height,
            HEADER_ROWS + MIN_LIST_ROWS,
        );
        let [header_area, list_area, warning_area] = Layout::vertical([
            Constraint::Length(HEADER_ROWS),
            Constraint::Fill(1),
            Constraint::Length(warning_rows),
        ])
        .areas(area);

        self.render_header(header_area, buf);
        self.render_list(list_area, buf, state);
        if warning_rows > 0 {
            Paragraph::new(warnings).render(warning_area, buf);
        }
    }
}

/// Where the weekly rollup's selection sits.
///
/// Held by [`App`](super::app::App) rather than by [`WeekListWidget`], which
/// borrows the rollup and is therefore rebuilt every frame; see the module
/// docs.
#[derive(Debug, Default)]
pub struct WeekListState {
    list: ListState,
}

impl WeekListState {
    /// Apply an event, if it is one this pane owns.
    ///
    /// `projects` is passed in rather than held for the same reason the
    /// widget borrows it: there is one rollup, and it lives on `App`. The
    /// caller passes an **empty** slice while the rollup on hand describes
    /// another week, which is what stops `Enter` yanking the previous
    /// week's hours into a timesheet — the hazard
    /// [`App::yank_week`](super::app::App) already guards `Y` against.
    ///
    /// Anything this pane does not own answers [`Handled::Ignored`] so it
    /// falls through to the global bindings, the same way
    /// [`ProjectListWidget::apply`] does for the day view.
    ///
    /// [`ProjectListWidget::apply`]: super::project_list::ProjectListWidget::apply
    pub fn apply(&mut self, event: &AppEvent, projects: &[WeeklyProject]) -> Handled {
        let len = projects.len();
        self.clamp(len);
        match event {
            AppEvent::NextWeekProject => self.next_item(len),
            AppEvent::PreviousWeekProject => self.previous_item(len),
            AppEvent::FirstWeekProject => self.go_to_first(len),
            AppEvent::LastWeekProject => self.go_to_last(len),
            AppEvent::CopyWeekProject => {
                return copy_intent(self.selected().and_then(|i| projects.get(i)));
            }
            _ => return Handled::Ignored,
        }
        Handled::Consumed
    }

    /// The selected row, or `None` when the week has no projects to select.
    pub fn selected(&self) -> Option<usize> {
        self.list.selected()
    }

    /// Bring the selection back into range for a list of `len` rows.
    ///
    /// A week with no projects selects nothing; a selection left pointing
    /// past the end of a shorter week falls back to the first row rather
    /// than highlighting a row the list no longer draws.
    fn clamp(&mut self, len: usize) {
        let Some(last) = len.checked_sub(1) else {
            self.list.select(None);
            return;
        };
        self.list
            .select(Some(self.selected().unwrap_or(0).min(last)));
    }

    fn next_item(&mut self, len: usize) {
        let Some(last) = len.checked_sub(1) else {
            return;
        };
        let i = match self.selected() {
            Some(i) if i < last => i + 1,
            _ => 0,
        };
        self.list.select(Some(i));
    }

    fn previous_item(&mut self, len: usize) {
        let Some(last) = len.checked_sub(1) else {
            return;
        };
        let i = match self.selected() {
            Some(i) if i > 0 => i - 1,
            Some(_) => last,
            None => 0,
        };
        self.list.select(Some(i));
    }

    fn go_to_first(&mut self, len: usize) {
        if len > 0 {
            self.list.select(Some(0));
        }
    }

    fn go_to_last(&mut self, len: usize) {
        if let Some(last) = len.checked_sub(1) {
            self.list.select(Some(last));
        }
    }
}

/// What `Enter` should put on the clipboard for `project`, as an intent
/// rather than as the copy itself.
///
/// The pane knows *what* to yank; the one connection to the system
/// clipboard, and the status line that reports a machine which has none,
/// belong to [`App`](super::app::App).
///
/// The hours go on the clipboard alongside the notes, unlike the day view's
/// `Enter`: a weekly timesheet line is the hours, and having to press `Y`
/// for the whole week to get them is what this pane exists to avoid.
/// Answers [`Handled::Consumed`] when nothing is selected — the key belonged
/// to this pane either way, and the pane is already saying why.
fn copy_intent(project: Option<&WeeklyProject>) -> Handled {
    let Some(project) = project else {
        return Handled::Consumed;
    };

    let hours = Time::format_duration_decimal(project.total_minutes);
    let mut payload = format!(
        "{} - {} ({hours} hrs)",
        project.name,
        Time::format_duration_minutes(project.total_minutes),
    );
    for note in &project.notes {
        payload.push_str("\n- ");
        payload.push_str(note);
    }

    Handled::Emit(AppEvent::CopyToClipboard(
        payload,
        format!("Copied {hours} hours for {}", project.name),
    ))
}

/// One project's row: the name on the left, the hours right-aligned against
/// `width`, so a week's projects read as two columns however long the
/// project codes happen to be.
///
/// Measured in display columns rather than characters, so a project code
/// with a wide glyph in it doesn't drag the hours column out of line. A
/// pane too narrow to hold both keeps a single space between them and lets
/// the list clip, which is what every other row in the TUI does at that
/// width.
fn project_row(project: &WeeklyProject, width: u16) -> String {
    let hours = format!(
        "{} ({} hrs)",
        Time::format_duration_minutes(project.total_minutes),
        Time::format_duration_decimal(project.total_minutes),
    );
    let content = 1 + project.name.width() + hours.width();
    let gap = usize::from(width).saturating_sub(content).max(1);
    format!(" {}{}{hours}", project.name, " ".repeat(gap))
}

/// A [`WeeklyProject`] for a fixture week.
#[cfg(test)]
pub(crate) fn week_project(
    name: &str,
    total_minutes: u32,
    notes: impl IntoIterator<Item = &'static str>,
) -> WeeklyProject {
    WeeklyProject {
        name: name.to_owned(),
        total_minutes,
        notes: notes.into_iter().map(str::to_owned).collect(),
    }
}

/// A week with three projects of unmistakably different sizes — client-bd
/// 18h, internal 9.5h, admin 6h — totalling 33.5 hours, plus three quarters
/// of an hour of dead time and one parser warning. The sizes are what make
/// the "biggest first" ordering falsifiable rather than coincidental.
///
/// Module-level rather than private to the tests below, because
/// [`crate::tui::ui`]'s no-panic sweep seeds it too: `day_app` alone leaves
/// `weekly_summary` at `None`, which sends the sweep down the empty-state
/// arm and leaves this widget unrendered at every degenerate size.
#[cfg(test)]
pub(crate) fn fixture_week_summary() -> WeeklySummary {
    WeeklySummary {
        total_minutes: 2010,
        dead_time_minutes: 45,
        projects: vec![
            week_project(
                "client-bd",
                1080,
                [
                    "Mon 2025-06-09: discovery call",
                    "Wed 2025-06-11: proposal draft",
                ],
            ),
            week_project("internal", 570, ["Tue 2025-06-10: code review"]),
            week_project("admin", 360, ["Mon 2025-06-09: inbox triage"]),
        ],
        warnings: vec!["Mon 2025-06-09: Error parsing time range '9-'".to_owned()],
        per_day: std::collections::HashMap::new(),
        days: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::tui::app::{App, LOADING_MESSAGE};
    use crate::tui::context::TuiContext;
    use crate::tui::mode::Mode;
    use crate::tui::testing::{fixture_date, fixture_day, render_to_string};

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn enter() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    }

    /// The week pane, opened on the fixture date with the fixture rollup
    /// already landed.
    fn week_app() -> App {
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_weekly_summary(fixture_week_summary());
        app.mode = Mode::Week;
        app
    }

    #[tokio::test]
    async fn week_mode_lists_projects_with_hours_biggest_first() {
        let mut app = week_app();
        let screen = render_to_string(&mut app, 100, 30);
        let bd = screen.find("client-bd").expect("client-bd must be listed");
        let admin = screen.find("admin").expect("admin must be listed");
        assert!(bd < admin, "biggest project first:\n{screen}");
        assert!(screen.contains("18"), "hours must be shown:\n{screen}");
    }

    #[tokio::test]
    async fn week_mode_shows_the_week_total_and_dead_time() {
        let mut app = week_app();
        let screen = render_to_string(&mut app, 100, 30);
        assert!(
            screen.contains("33.5") || screen.contains("33"),
            "week total:\n{screen}"
        );
        assert!(
            screen.to_lowercase().contains("dead"),
            "week dead time:\n{screen}"
        );
    }

    /// A clean week still says so, rather than leaving the reader to infer
    /// zero dead time from a missing line.
    #[tokio::test]
    async fn a_week_with_no_dead_time_says_none_rather_than_dropping_the_line() {
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_weekly_summary(WeeklySummary {
                dead_time_minutes: 0,
                ..fixture_week_summary()
            });
        app.mode = Mode::Week;
        let screen = render_to_string(&mut app, 100, 30);
        assert!(
            screen.contains(&format!("Dead Time: {NO_DEAD_TIME}")),
            "got:\n{screen}"
        );
    }

    /// The block title names the week on screen, so a rollup read off this
    /// pane can be filed against the right timesheet week.
    #[tokio::test]
    async fn the_pane_is_titled_with_the_week_it_shows() {
        let mut app = week_app();
        let screen = render_to_string(&mut app, 100, 30);
        // fixture_date() is a Wednesday, and `TuiContext::for_test` starts
        // weeks on Saturday.
        assert!(screen.contains("2025-06-07"), "week start:\n{screen}");
        assert!(screen.contains("2025-06-13"), "week end:\n{screen}");
    }

    #[tokio::test]
    async fn enter_in_week_mode_yanks_that_projects_week_notes() {
        let mut app = week_app();
        app.handle_key_events(enter()).unwrap();
        let (payload, _) = app
            .take_pending_copy()
            .expect("Enter must emit a copy intent");
        assert!(
            payload.contains("client-bd"),
            "the selected (first) project is yanked:\n{payload}"
        );
        assert!(
            payload.contains("discovery call"),
            "with its week's notes:\n{payload}"
        );
        assert!(
            payload.contains("18.00"),
            "and its hours, which the day view's Enter cannot give:\n{payload}"
        );
    }

    /// `Enter` follows the selection rather than always yanking the top row.
    #[tokio::test]
    async fn enter_yanks_whichever_project_is_selected() {
        let mut app = week_app();
        app.handle_key_events(key('j')).unwrap();
        app.handle_key_events(enter()).unwrap();

        let (payload, message) = app
            .take_pending_copy()
            .expect("Enter must emit a copy intent");
        assert!(payload.contains("internal"), "got:\n{payload}");
        assert!(!payload.contains("client-bd"), "got:\n{payload}");
        assert!(message.contains("9.50"), "got: {message}");
    }

    #[tokio::test]
    async fn week_mode_with_no_data_renders_an_empty_state_not_a_panic() {
        let mut app = App::new(TuiContext::for_test());
        app.weekly_summary = None;
        app.mode = Mode::Week;
        let screen = render_to_string(&mut app, 100, 30);
        assert!(!screen.trim().is_empty());
    }

    /// A week that loaded and genuinely has nothing in it is not the same
    /// as a week still loading, and must not borrow the other's wording.
    #[tokio::test]
    async fn a_loaded_but_empty_week_says_so_rather_than_claiming_to_be_loading() {
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_weekly_summary(WeeklySummary::default());
        app.mode = Mode::Week;

        let screen = render_to_string(&mut app, 100, 30);
        let pane = screen
            .lines()
            .filter(|line| !line.contains(LOADING_MESSAGE))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            pane.to_lowercase().contains("no tracked time"),
            "got:\n{screen}"
        );
    }

    /// The hazard this pane shares with `Y`: `go_to_date` moves
    /// `active_date` and `week_dates` synchronously but only *queues* the
    /// reload, so `weekly_summary` still holds the **previous** week for as
    /// long as that reload sits unapplied. Drawing it here would put last
    /// week's hours under this week's header — the exact shape of the bug
    /// `Y` was found copying into a timesheet.
    #[tokio::test]
    async fn a_stale_week_shows_a_loading_state_rather_than_last_weeks_numbers() {
        let mut app = week_app();
        let before = render_to_string(&mut app, 100, 30);
        assert!(
            before.contains("client-bd"),
            "guards the assertion below from passing vacuously:\n{before}"
        );

        // Moves into the following week and queues the reload, whose sync
        // arm is a no-op — so no load is ever spawned and the rollup on
        // hand has no way to catch up. This is the frame before the payload
        // lands.
        app.handle_key_events(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::SHIFT))
            .unwrap();
        app.drain_pending_events();
        assert!(app.week_is_stale(), "the reload must not have landed yet");

        let after = render_to_string(&mut app, 100, 30);
        assert!(
            !after.contains("client-bd"),
            "last week's projects must not be drawn under this week's header:\n{after}"
        );
        assert!(!after.contains("33.5"), "nor last week's total:\n{after}");
        assert!(after.contains(LOADING_MESSAGE), "got:\n{after}");
    }

    /// The same window on the clipboard axis: `Enter` must not yank the
    /// previous week's hours into a timesheet either.
    #[tokio::test]
    async fn enter_on_a_stale_week_copies_nothing() {
        let mut app = week_app();
        app.handle_key_events(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::SHIFT))
            .unwrap();
        app.drain_pending_events();
        assert!(app.week_is_stale());

        app.handle_key_events(enter()).unwrap();
        assert!(
            app.take_pending_copy().is_none(),
            "a stale week must not be copied"
        );
    }

    #[tokio::test]
    async fn w_toggles_between_day_and_week_mode() {
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(fixture_day());
        assert_eq!(app.mode, Mode::Day);

        app.handle_key_events(key('w')).unwrap();
        app.drain_pending_events();
        assert_eq!(app.mode, Mode::Week);

        app.handle_key_events(key('w')).unwrap();
        app.drain_pending_events();
        assert_eq!(app.mode, Mode::Day);
    }

    /// The rule the zoom key and `v` already pin, for `w`:
    /// `ToggleWeekMode` has to sit on the `true` side of
    /// `changes_key_routing`, or a key typed straight after it resolves
    /// against the mode just left — here, moving the day's hidden project
    /// list instead of the rollup the user is looking at.
    #[tokio::test]
    async fn a_key_typed_straight_after_w_resolves_in_the_new_mode() {
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(fixture_day())
            .with_weekly_summary(fixture_week_summary());

        app.handle_key_events(key('w')).unwrap();
        app.handle_key_events(key('j')).unwrap();
        app.drain_pending_events();

        assert_eq!(app.mode, Mode::Week);
        assert_eq!(
            app.week_list.selected(),
            Some(1),
            "j must move the rollup's selection"
        );
        assert_eq!(
            app.project_list_widget
                .as_ref()
                .and_then(|widget| widget.selected_item()),
            Some(0),
            "and must not move the day's hidden list"
        );
    }

    #[tokio::test]
    async fn j_k_g_and_capital_g_move_the_weekly_selection() {
        let mut app = week_app();
        // The selection materialises on the first frame, the same way it
        // does when the pane is opened for real — and it starts on the
        // biggest project, which is what `Enter` is most often after.
        let _ = render_to_string(&mut app, 100, 30);
        assert_eq!(app.week_list.selected(), Some(0));

        app.handle_key_events(key('j')).unwrap();
        assert_eq!(app.week_list.selected(), Some(1));

        app.handle_key_events(key('G')).unwrap();
        assert_eq!(app.week_list.selected(), Some(2));

        app.handle_key_events(key('k')).unwrap();
        assert_eq!(app.week_list.selected(), Some(1));

        app.handle_key_events(key('g')).unwrap();
        assert_eq!(app.week_list.selected(), Some(0));
    }

    /// The day view's list must not answer for the rollup's keys, and the
    /// rollup's must not answer for the day's: they share the physical keys
    /// under disjoint mode masks, which only works if each pane sees only
    /// its own events.
    #[tokio::test]
    async fn the_day_lists_selection_is_untouched_by_the_rollups_keys() {
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(fixture_day())
            .with_weekly_summary(fixture_week_summary());
        app.mode = Mode::Week;

        app.handle_key_events(key('G')).unwrap();
        app.drain_pending_events();

        assert_eq!(app.week_list.selected(), Some(2));
        assert_eq!(
            app.project_list_widget
                .as_ref()
                .and_then(|widget| widget.selected_item()),
            Some(0)
        );
    }

    /// The selection outlives the week it was made in, so a shorter week
    /// must pull it back into range rather than highlight a row that is no
    /// longer drawn.
    #[test]
    fn a_selection_past_the_end_of_a_shorter_week_falls_back_to_the_first_row() {
        let mut state = WeekListState::default();
        let projects = fixture_week_summary().projects;

        state.apply(&AppEvent::LastWeekProject, &projects);
        assert_eq!(state.selected(), Some(2));

        state.apply(&AppEvent::NextWeekProject, &projects[..1]);
        assert_eq!(state.selected(), Some(0));
    }

    #[test]
    fn an_empty_week_selects_nothing_and_copies_nothing() {
        let mut state = WeekListState::default();
        assert_eq!(
            state.apply(&AppEvent::NextWeekProject, &[]),
            Handled::Consumed
        );
        assert_eq!(state.selected(), None);
        assert_eq!(
            state.apply(&AppEvent::CopyWeekProject, &[]),
            Handled::Consumed
        );
    }

    /// Anything the pane does not own has to fall through to the global
    /// bindings, or `q` would stop quitting from the rollup.
    #[test]
    fn an_event_the_pane_does_not_own_falls_through() {
        let mut state = WeekListState::default();
        assert_eq!(
            state.apply(&AppEvent::Quit, &fixture_week_summary().projects),
            Handled::Ignored
        );
    }

    /// Both ends wrap, the way the day view's list does.
    #[test]
    fn the_selection_wraps_at_both_ends() {
        let mut state = WeekListState::default();
        let projects = fixture_week_summary().projects;

        state.apply(&AppEvent::PreviousWeekProject, &projects);
        assert_eq!(
            state.selected(),
            Some(2),
            "up from the top wraps to the end"
        );

        state.apply(&AppEvent::NextWeekProject, &projects);
        assert_eq!(
            state.selected(),
            Some(0),
            "and down from the end wraps back"
        );
    }

    /// The defect this closes: the pane is the surface a week's hours get
    /// pasted into a timesheet from, and it was showing them with nothing
    /// said about a day that failed to parse. `project_list.rs` has shown a
    /// day's warnings all along; from the TUI a *week's* were reachable
    /// only by visiting all seven days one at a time.
    #[tokio::test]
    async fn the_weeks_parser_warnings_are_shown_under_the_hours() {
        let mut app = week_app();
        let screen = render_to_string(&mut app, 100, 30);
        assert!(screen.contains("Warnings (1)"), "got:\n{screen}");
        assert!(
            screen.contains("Error parsing time range"),
            "the warning itself, not just a count:\n{screen}"
        );
        assert!(
            screen.find("client-bd") < screen.find("Warnings (1)"),
            "the hours come first; the warnings qualify them:\n{screen}"
        );
    }

    /// A clean week pays no rows at all for the block, so the list keeps
    /// every row it had before the block existed.
    #[tokio::test]
    async fn a_clean_week_pays_no_rows_for_a_warnings_block() {
        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_weekly_summary(WeeklySummary {
                warnings: Vec::new(),
                ..fixture_week_summary()
            });
        app.mode = Mode::Week;

        let screen = render_to_string(&mut app, 100, 30);
        assert!(!screen.contains("Warnings"), "got:\n{screen}");
    }

    /// The week axis of the day pane's blank-list defect, and the same fix.
    /// `Length(HEADER_ROWS)` and `Length(warning_rows)` have equal claim on
    /// the pane, so on a short terminal the one-row warnings block took the
    /// last row and the two numbers this pane exists to show went with it —
    /// at 80x4 the only thing left on screen was `Warnings (1)`. The block
    /// yields to the totals now, however short the terminal gets.
    #[tokio::test]
    async fn the_weeks_totals_outrank_its_warnings_block_on_a_short_terminal() {
        for height in [4, 6, 8, 10] {
            let mut app = App::new(TuiContext::for_test())
                .with_active_date(fixture_date())
                .with_weekly_summary(fixture_week_summary());
            app.mode = Mode::Week;
            let screen = render_to_string(&mut app, 80, height);

            assert!(
                screen.contains("Working Time"),
                "80x{height} lost the week's hours to its warnings block:\n{screen}"
            );
        }
    }

    /// A badly parsed week must not push the hours off the pane. The count
    /// in the title is what keeps the flag honest when the lines don't fit.
    #[tokio::test]
    async fn a_flood_of_warnings_is_capped_but_still_counted() {
        let warnings: Vec<String> = (0..12).map(|i| format!("Mon: bad entry {i}")).collect();
        let summary = WeeklySummary {
            warnings,
            ..fixture_week_summary()
        };
        let rows = WeekListWidget::new(&summary, &Theme::none()).warning_lines();
        assert_eq!(rows.len(), band::MAX_WARNING_ROWS, "the cap must hold");

        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_weekly_summary(summary);
        app.mode = Mode::Week;
        let screen = render_to_string(&mut app, 100, 30);
        assert!(screen.contains("Warnings (12)"), "got:\n{screen}");
        assert!(screen.contains("and 9 more"), "got:\n{screen}");
        assert!(
            screen.contains("client-bd"),
            "the hours the pane exists for must survive the flood:\n{screen}"
        );
    }

    /// Exactly at the budget, every warning is listed rather than the last
    /// one being traded for a "… and 1 more" that costs the same row.
    #[test]
    fn a_week_that_fits_the_budget_lists_every_warning() {
        let warnings: Vec<String> = (0..band::MAX_WARNING_ROWS - 1)
            .map(|i| format!("Mon: bad entry {i}"))
            .collect();
        let summary = WeeklySummary {
            warnings,
            ..fixture_week_summary()
        };
        let rows = WeekListWidget::new(&summary, &Theme::none()).warning_lines();
        assert_eq!(rows.len(), band::MAX_WARNING_ROWS);
        assert!(
            !format!("{rows:?}").contains("more"),
            "nothing was left off, so nothing should say so"
        );
    }

    /// The hours column is aligned against the pane's right edge, which is
    /// the whole reason the row is measured rather than padded to a fixed
    /// column.
    #[test]
    fn a_row_right_aligns_its_hours_against_the_pane_width() {
        let project = week_project("client-bd", 1080, []);
        let row = project_row(&project, 40);
        assert_eq!(row.width(), 40, "got: {row:?}");
        assert!(row.starts_with(" client-bd"), "got: {row:?}");
        assert!(row.ends_with("18:00 (18.00 hrs)"), "got: {row:?}");
    }

    /// A pane too narrow for both columns keeps them apart and lets the
    /// list clip, rather than panicking on a negative gap.
    #[test]
    fn a_row_narrower_than_its_content_still_renders() {
        let project = week_project("a-very-long-project-code", 1080, []);
        let row = project_row(&project, 4);
        assert!(row.contains("a-very-long-project-code"), "got: {row:?}");
        assert!(row.contains("18:00"), "got: {row:?}");
    }
}
