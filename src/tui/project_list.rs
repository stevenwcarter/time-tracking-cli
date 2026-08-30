#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;

use ratatui::prelude::*;
use ratatui::widgets::*;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use time_tracking_parser::TimeTrackingData;

use super::band;
use super::event::AppEvent;
use super::mode::Handled;
use super::theme::Theme;

/// Columns reserved for the project name in the header line before the hours
/// column starts.
const NAME_COLS: usize = 25;

/// Width of the `"   - "` bullet marker in front of a project's first note
/// line, in display columns. Continuation lines produced by [`wrap_note`]
/// are indented by the same amount so wrapped text stays aligned under the
/// first line rather than under the marker.
const BULLET_INDENT: usize = 5;

/// Dead time at or above this many minutes is a hard failure rather than a
/// recoverable warning. Mirrors the split `format_day_summary_impl` in
/// `src/display/mod.rs` uses, so the TUI and the CLI never disagree about
/// the same file.
const DEAD_TIME_ERROR_THRESHOLD_MINUTES: u32 = 90;

/// Visual gap between the "Working Time" and "Dead Time" halves of the
/// header's busiest row, used both to build the shared-row line and to
/// measure whether it fits — one literal, so the two can never drift apart.
const WORKING_DEAD_SEPARATOR: &str = "    ";

/// The row the list's own `Borders::TOP` title bar costs, outside the
/// viewport its items are laid out in.
///
/// A *budget*, read by the sizing helpers below. The drawn geometry comes
/// from `ProjectListWidget::list_block`'s own `inner`, so the hit-test and
/// the render cannot disagree about it even if the block's decoration
/// changes.
const LIST_TITLE_ROWS: u16 = 1;

/// Columns every row gives up to the highlight marker and the padding
/// around it, subtracted from the list width before a body is wrapped.
///
/// One constant rather than a literal per site because
/// [`ProjectItem::body_rows`] memoises per width: a hit-test measuring at
/// a width the render never used silently rebuilds the layout and then
/// walks the row heights of *that* one.
const BODY_WIDTH_MARGIN: u16 = 4;

/// A typical list item's height: a project's header line and two notes.
const TYPICAL_ITEM_ROWS: u16 = 3;

/// The header's height on a clean day: the start/end block plus a single
/// working-time row, with no dead time and no warnings to pay for.
const CLEAN_HEADER_ROWS: u16 = 4;

/// Rows the list keeps for itself however tall the header above it wants to
/// be: its title bar plus one whole item.
///
/// The floor handed to [`band::fit_band`] — see that module for why the
/// header is the part that yields. Paired with [`clamp_item_rows`]: the
/// floor guarantees the list *has* a viewport, and the clamp guarantees the
/// first item fits inside it, which is what ratatui 0.29 needs before it
/// will draw any item at all.
const LIST_FLOOR_ROWS: u16 = LIST_TITLE_ROWS + TYPICAL_ITEM_ROWS;

/// How many projects on screen at once count as a usable list.
///
/// This codebase's one notion of "usable", so both things that need it are
/// derived from it rather than from a second number:
/// [`MIN_ROWS_FOR_TWO_PROJECTS`] below, and the count
/// `App::band_is_affordable` measures the calendar/chart band against.
///
/// Two rather than all of them because the list *scrolls*. Rendering fewer
/// projects at once is not data loss — `j`/`k` reach the rest. Rendering
/// none was, and that is pinned separately.
pub(super) const USABLE_PROJECTS: usize = 2;

/// The inner height at which this pane can show a usable list on a clean
/// day.
///
/// Exported for [`ui`](super::ui)'s `COMPACT_ROWS`, the size-only floor
/// below which the calendar/chart band is never drawn. An estimate, because
/// its caller applies it to days that have no project list to measure;
/// [`ProjectListWidget::rows_to_show`] is the measured version, and is what
/// a day with a list is actually judged by.
pub(super) const MIN_ROWS_FOR_TWO_PROJECTS: u16 =
    CLEAN_HEADER_ROWS + LIST_TITLE_ROWS + USABLE_PROJECTS as u16 * TYPICAL_ITEM_ROWS;

/// Stands in for the notes a clamped item could not show. Indented to line
/// up under the bullets it replaces.
const OVERFLOW_MARKER_INDENT: &str = "   ";

/// The day view's main pane: the day's header stats above a scrollable list
/// of its projects and their notes.
///
/// Built once per loaded day by [`ProjectListWidget::new`] and held by the
/// app from there on, which is what lets each row's per-width wrapping stay
/// memoized across scrolling and re-renders at an unchanged terminal size.
#[derive(Debug)]
pub struct ProjectListWidget {
    start_time: String,
    end_time: String,
    total_decimal: String,
    dead_time_minutes: u32,
    dead_time: String,
    dead_decimal: String,
    warnings: Vec<String>,
    project_list: ProjectList,
    theme: Theme,
}

#[derive(Default, Debug)]
struct ProjectList {
    items: Vec<ProjectItem>,
    state: ListState,
}

#[derive(Debug)]
struct ProjectItem {
    name: String,
    total_hours: f32,
    tasks: Vec<String>,
    /// The wrapped body, memoized per width so scrolling and re-renders at
    /// an unchanged terminal size don't re-wrap every note on every frame.
    rendered: RefCell<Option<(u16, Text<'static>)>>,
    /// Counts real cache-miss rebuilds. Test-only: it is what turns "is this
    /// actually memoized?" into a falsifiable assertion instead of something
    /// merely assumed from two calls returning equal output.
    #[cfg(test)]
    rebuild_count: Cell<usize>,
}

impl ProjectItem {
    fn new(name: String, total_hours: f32, tasks: Vec<String>) -> Self {
        Self {
            name,
            total_hours,
            tasks,
            rendered: RefCell::new(None),
            #[cfg(test)]
            rebuild_count: Cell::new(0),
        }
    }

    /// The rendered body for a list area `width` columns wide, rebuilding
    /// and caching it only when `width` differs from the last call.
    fn body(&self, width: u16) -> Text<'static> {
        if let Some((cached_width, text)) = self.rendered.borrow().as_ref()
            && *cached_width == width
        {
            return text.clone();
        }

        let text = self.render_body(width);
        #[cfg(test)]
        self.rebuild_count.set(self.rebuild_count.get() + 1);
        *self.rendered.borrow_mut() = Some((width, text.clone()));
        text
    }

    /// Rows [`Self::body`] occupies at `width`, read off the memo rather
    /// than through the clone `body` hands back — this is called once per
    /// item per frame purely to measure, and measuring should not allocate.
    fn body_rows(&self, width: u16) -> usize {
        if let Some((cached_width, text)) = self.rendered.borrow().as_ref()
            && *cached_width == width
        {
            return text.lines.len();
        }
        self.body(width).lines.len()
    }

    #[cfg(test)]
    fn rebuild_count(&self) -> usize {
        self.rebuild_count.get()
    }

    /// Builds the header line plus wrapped bullet lines from scratch for the
    /// given `width`. Never truncates a note away: a word wider than `width`
    /// is hard-broken rather than dropped.
    fn render_body(&self, width: u16) -> Text<'static> {
        let name = pad_display_width(&self.name, NAME_COLS);
        let hour_word = if self.total_hours == 1. {
            "hour"
        } else {
            "hours"
        };
        let mut lines = vec![Line::from(format!(
            " {name}{} {hour_word}",
            self.total_hours
        ))];

        for task in &self.tasks {
            let wrapped = wrap_note(task, width, BULLET_INDENT);
            let mut wrapped = wrapped.into_iter();
            if let Some(first) = wrapped.next() {
                lines.push(Line::from(format!("   - {first}")));
            }
            lines.extend(wrapped.map(Line::from));
        }

        Text::from(lines)
    }
}

impl ProjectListWidget {
    pub fn new(data: &TimeTrackingData, theme: &Theme) -> Self {
        let items: Vec<ProjectItem> = data
            .projects
            .iter()
            .map(|project| {
                ProjectItem::new(
                    project.name.clone(),
                    project.total_minutes as f32 / 60.,
                    project.notes.clone(),
                )
            })
            .collect();

        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(0));
        }

        Self {
            start_time: data.formatted_start_time(),
            end_time: data.formatted_end_time(),
            total_decimal: data.formatted_total_decimal(),
            dead_time_minutes: data.dead_time_minutes,
            dead_time: data.formatted_dead_time_minutes(),
            dead_decimal: data.formatted_dead_decimal(),
            warnings: data.warnings.clone(),
            project_list: ProjectList { items, state },
            theme: theme.clone(),
        }
    }

    /// Apply an event, if it is one this list owns.
    ///
    /// Which key produced `event` is decided by
    /// [`keymap`](super::keymap) — the list never sees a [`KeyCode`], which
    /// is what keeps the keymap in one place.
    ///
    /// [`KeyCode`]: crossterm::event::KeyCode
    pub fn apply(&mut self, event: &AppEvent) -> Handled {
        match event {
            AppEvent::NextProject => self.next_item(),
            AppEvent::PreviousProject => self.previous_item(),
            AppEvent::FirstProject => self.go_to_first(),
            AppEvent::LastProject => self.go_to_last(),
            AppEvent::CopyNotes => return self.copy_intent(),
            _ => return Handled::Ignored,
        }
        Handled::Consumed
    }

    /// What `Enter` should put on the clipboard, as an intent rather than as
    /// the copy itself.
    ///
    /// The widget knows *what* to yank and what to say about it; the one
    /// connection to the system clipboard — and the status line that reports
    /// a machine which has none — belongs to
    /// [`App`](super::app::App). Doing the I/O here is what made the headline
    /// action of the TUI fail silently into a log file the alternate screen
    /// hides.
    ///
    /// Answers [`Handled::Consumed`] when there is nothing selected: the key
    /// belonged to this list either way.
    fn copy_intent(&self) -> Handled {
        let Some(project) = self
            .project_list
            .state
            .selected()
            .and_then(|selected| self.project_list.items.get(selected))
        else {
            return Handled::Consumed;
        };

        let count = project.tasks.len();
        let noun = if count == 1 { "note" } else { "notes" };
        Handled::Emit(AppEvent::CopyToClipboard(
            // Empty for a project with no notes, which `App` reports rather
            // than wiping whatever the user already had on the clipboard.
            project
                .tasks
                .iter()
                .map(|task| format!("- {task}"))
                .collect::<Vec<_>>()
                .join("\n"),
            format!("Copied {count} {noun} for {}", project.name),
        ))
    }

    fn next_item(&mut self) {
        let Some(last) = self.project_list.items.len().checked_sub(1) else {
            return;
        };
        let i = match self.project_list.state.selected() {
            Some(i) if i < last => i + 1,
            _ => 0,
        };
        self.project_list.state.select(Some(i));
    }

    fn previous_item(&mut self) {
        let Some(last) = self.project_list.items.len().checked_sub(1) else {
            return;
        };
        let i = match self.project_list.state.selected() {
            Some(i) if i > 0 => i - 1,
            Some(_) => last,
            None => 0,
        };
        self.project_list.state.select(Some(i));
    }

    fn go_to_first(&mut self) {
        if !self.project_list.items.is_empty() {
            self.project_list.state.select(Some(0));
        }
    }

    fn go_to_last(&mut self) {
        if !self.project_list.items.is_empty() {
            self.project_list
                .state
                .select(Some(self.project_list.items.len() - 1));
        }
    }

    pub fn selected_item(&self) -> Option<usize> {
        self.project_list.state.selected()
    }

    pub fn has_items(&self) -> bool {
        !self.project_list.items.is_empty()
    }

    /// The block [`Self::render_list`] wraps the list in: its title bar,
    /// drawn as a top border.
    ///
    /// Shared with [`Self::item_rows`] so the row it costs is ratatui's
    /// answer for the block actually drawn, rather than a constant the
    /// renderer and the hit-test keep in step by hand.
    fn list_block(&self) -> Block<'static> {
        Block::new()
            .title(Line::raw("Project Summaries").centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(self.theme.list_header)
    }

    /// The rows the projects themselves are drawn into when this pane is
    /// rendered in `area`: `area` minus its header band and minus the
    /// list's title bar.
    ///
    /// `area` is the whole pane — the rect `App` records when it draws this
    /// widget — because that is what a click arrives with.
    fn item_rows(&self, area: Rect) -> Rect {
        let working_time_lines = self.working_time_lines(area.width);
        let warning_lines = band::warning_lines(&self.warnings, self.theme.error, "");
        let [_, main_area] = split_pane(area, &working_time_lines, &warning_lines);
        self.list_block().inner(main_area)
    }

    /// The project index drawn at row `y` when this list is rendered in
    /// `area`, or `None` for the header band, the title row, and rows past
    /// the last item.
    ///
    /// Both the header split and the title row come from [`Self::item_rows`]
    /// — the same call `render` makes — rather than being added up again
    /// here. That is the whole point: while this worked the offset out for
    /// itself it resolved rows against the *pane*, and every click on a day
    /// with a header selected a project several rows above the cursor.
    ///
    /// Rows are **variable-height**: [`Self::render_list`] clamps each
    /// item's [`ProjectItem::body_rows`] to the viewport via
    /// [`clamp_item_rows`], so an index cannot be divided out of `y` —
    /// this walks the same per-item heights instead, starting at
    /// [`ListState::offset`]. Rather than replaying `clamp_item_rows`'s
    /// kept-lines-plus-marker arithmetic, it uses the one fact that
    /// matters here: the clamp always leaves an item exactly
    /// `body_rows(width).min(viewport_rows.max(1))` rows tall.
    ///
    /// Current only because hit-testing runs after a draw, so the offset
    /// a click lands on is the one already on screen.
    pub fn index_at(&self, area: Rect, y: u16) -> Option<usize> {
        let rows = self.item_rows(area);
        if y < rows.y || y >= rows.y.checked_add(rows.height)? {
            return None;
        }

        let body_width = rows.width.saturating_sub(BODY_WIDTH_MARGIN);
        let viewport_rows = usize::from(rows.height.max(1));
        let target = usize::from(y - rows.y);

        let mut row = 0_usize;
        for index in self.project_list.state.offset()..self.project_list.items.len() {
            let item = self.project_list.items.get(index)?;
            let height = item.body_rows(body_width).min(viewport_rows);
            let next = row + height;
            if target < next {
                return Some(index);
            }
            row = next;
        }
        None
    }

    /// Select `index`, ignoring one past the end.
    pub fn select_index(&mut self, index: usize) {
        if index < self.project_list.items.len() {
            self.project_list.state.select(Some(index));
        }
    }
}

impl Widget for &mut ProjectListWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // No footer row: the status line is drawn by `App`, across the whole
        // width and in every mode, so the help hint survives a day with no
        // project list at all.
        //
        // The working/dead-time row is measured once, at the real render
        // width, and threaded into both the height calculation and the
        // render itself so the two can never disagree about how many rows
        // it took.
        let working_time_lines = self.working_time_lines(area.width);
        let warning_lines = band::warning_lines(&self.warnings, self.theme.error, "");
        let [header_area, main_area] = split_pane(area, &working_time_lines, &warning_lines);

        self.render_header(header_area, working_time_lines, warning_lines, buf);
        self.render_list(main_area, buf);
    }
}

/// The pane's header band and the list below it, given the header content
/// already measured at this width.
///
/// The one place the split is computed. [`ProjectListWidget::render`] and
/// [`ProjectListWidget::item_rows`] — and so [`ProjectListWidget::index_at`]
/// — both come through here, because the one thing a hit-test must not do
/// is derive the offset separately: two copies agree only until either side
/// moves, and the symptom is a click selecting a project several rows from
/// the one under the cursor rather than anything that reads as a layout bug.
fn split_pane(
    area: Rect,
    working_time_lines: &[Line<'static>],
    warning_lines: &[Line<'static>],
) -> [Rect; 2] {
    // The header is a `Length` against the list's `Fill(1)`, so it wins
    // every tie and used to win the list's last row outright — see
    // `band` for the whole failure. It yields to the list's floor here.
    let header_rows = band::fit_band(
        header_height(working_time_lines, warning_lines),
        area.height,
        LIST_FLOOR_ROWS,
    );
    Layout::vertical([Constraint::Length(header_rows), Constraint::Fill(1)]).areas(area)
}

/// Rows the header needs given its already-measured `working_time_lines`
/// ([`working_time_lines`](ProjectListWidget::working_time_lines)) and
/// `warning_lines` ([`band::warning_lines`]): three for the start/end block,
/// which is always present, plus however many rows the working/dead-time row
/// took, plus a blank separator and the warnings block when the day has any.
///
/// A clean day on a wide-enough terminal — no dead time, no warnings — never
/// pays for a block it doesn't show, so the list below keeps every row it
/// had before either feature existed.
fn header_height(working_time_lines: &[Line<'static>], warning_lines: &[Line<'static>]) -> u16 {
    const FIXED_ROWS: u16 = 3;
    let rows = |lines: &[Line<'static>]| u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let warning_rows = if warning_lines.is_empty() {
        0
    } else {
        1 + rows(warning_lines)
    };
    FIXED_ROWS + rows(working_time_lines) + warning_rows
}

/// `body` cut down to at most `max_rows` rows, the notes that were dropped
/// replaced by a line counting them.
///
/// This is the amplifier in the blank-pane failure, defused at its source:
/// ratatui 0.29's `List` walks its items until one does not fit and then
/// stops, so when the *first* item is taller than the viewport it draws
/// **zero** items rather than a clipped one — a whole day's work vanishing
/// because a project picked up a fourth note. Handing the list nothing
/// taller than its viewport turns that cliff into a gradual degradation.
fn clamp_item_rows(mut body: Text<'static>, max_rows: u16) -> Text<'static> {
    let max_rows = usize::from(max_rows.max(1));
    if body.lines.len() <= max_rows {
        return body;
    }

    // At one row only the project's own header line fits, and it is the row
    // worth keeping: it carries the name and the hours.
    let kept = max_rows.saturating_sub(1).max(1);
    let hidden = body.lines.len() - kept;
    body.lines.truncate(kept);
    if max_rows > 1 {
        body.lines.push(Line::from(format!(
            "{OVERFLOW_MARKER_INDENT}… +{hidden} more"
        )));
    }
    body
}

impl ProjectListWidget {
    /// The inner rows this pane would need at `width` to show its header and
    /// the first `projects` of its projects at once — or all of them, if it
    /// has fewer than that.
    ///
    /// Read by [`ui`](super::ui) to decide whether the calendar/chart band
    /// above the pane can be afforded — see `App::band_is_affordable`. It has
    /// to be measured rather than estimated because all three inputs move:
    /// the header grows with dead time and warnings, and an item grows with
    /// its notes and with how they wrap at this width. The list draws from
    /// the top, so the first `projects` items are the ones that decide it.
    pub(super) fn rows_to_show(&self, width: u16, projects: usize) -> u16 {
        let working_time_lines = self.working_time_lines(width);
        let warning_lines = band::warning_lines(&self.warnings, self.theme.error, "");
        let body_width = width.saturating_sub(BODY_WIDTH_MARGIN);
        let items = self
            .project_list
            .items
            .iter()
            .take(projects)
            .map(|item| u16::try_from(item.body_rows(body_width)).unwrap_or(u16::MAX))
            .fold(0u16, u16::saturating_add);

        header_height(&working_time_lines, &warning_lines)
            .saturating_add(LIST_TITLE_ROWS)
            .saturating_add(items)
    }

    fn render_header(
        &self,
        area: Rect,
        working_time_lines: Vec<Line<'static>>,
        warning_lines: Vec<Line<'static>>,
        buf: &mut Buffer,
    ) {
        let mut lines = vec![
            Line::from(format!("Start Time: {}", self.start_time)),
            Line::from(format!("  End Time: {}", self.end_time)),
            Line::from(""),
        ];
        lines.extend(working_time_lines);

        if !warning_lines.is_empty() {
            lines.push(Line::from(""));
            lines.extend(warning_lines);
        }

        Paragraph::new(lines).bold().centered().render(area, buf);
    }

    /// The "Working Time" line, plus a "Dead Time" line whenever the day
    /// has any. The two share a single row when `width` has room for both
    /// — styled `theme.warning` below [`DEAD_TIME_ERROR_THRESHOLD_MINUTES`]
    /// and `theme.error` at or above it, matching `format_day_summary_impl`
    /// in `src/display/mod.rs` so the TUI and the CLI never disagree about
    /// the same file — and fall back to two rows when they don't: this
    /// `Paragraph` has no wrapping, so without the fallback the shared row
    /// silently truncates mid-word for the majority of non-round-hour
    /// working days at the 60-column floor. A day with no dead time
    /// renders nothing extra, so it costs no header width or height.
    fn working_time_lines(&self, width: u16) -> Vec<Line<'static>> {
        let working = format!("Working Time: {} hours", self.total_decimal);
        if self.dead_time_minutes == 0 {
            return vec![Line::from(working)];
        }

        let style = if self.dead_time_minutes < DEAD_TIME_ERROR_THRESHOLD_MINUTES {
            self.theme.warning
        } else {
            self.theme.error
        };
        let dead = format!(
            "Dead Time: {} ({} hours)",
            self.dead_time, self.dead_decimal
        );

        let combined_width = working.width() + WORKING_DEAD_SEPARATOR.len() + dead.width();
        if combined_width <= usize::from(width) {
            return vec![Line::from(vec![
                Span::raw(format!("{working}{WORKING_DEAD_SEPARATOR}")),
                Span::styled(dead, style),
            ])];
        }

        vec![Line::from(working), Line::styled(dead, style)]
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer) {
        let block = self.list_block();
        // The rows the items get: the block's title bar is a top border, so
        // it sits inside `area` and outside this. `Self::index_at` resolves
        // a click against the very same rect.
        let rows = block.inner(area);

        let body_width = rows.width.saturating_sub(BODY_WIDTH_MARGIN);
        let viewport_rows = rows.height;
        let items: Vec<ListItem> = self
            .project_list
            .items
            .iter()
            .enumerate()
            .map(|(i, project_item)| {
                ListItem::new(clamp_item_rows(
                    project_item.body(body_width),
                    viewport_rows,
                ))
                .style(alternate_row_style(&self.theme, i))
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(self.theme.selection)
            .highlight_symbol(">>")
            .highlight_spacing(HighlightSpacing::Always)
            .repeat_highlight_symbol(true)
            .scroll_padding(1);

        StatefulWidget::render(list, area, buf, &mut self.project_list.state);
    }
}

/// Background style for row `i`, alternating so long lists stay readable.
fn alternate_row_style(theme: &Theme, i: usize) -> Style {
    if i.is_multiple_of(2) {
        theme.row_bg
    } else {
        theme.alt_row_bg
    }
}

/// Pad `name` with spaces to `cols` display columns, truncating with `…`
/// if it is wider. Uses display width, not char count, so CJK and emoji
/// keep the hours column aligned.
fn pad_display_width(name: &str, cols: usize) -> String {
    let w = name.width();
    if w <= cols {
        format!("{name}{}", " ".repeat(cols - w))
    } else {
        let mut out = String::new();
        let mut used = 0;
        for c in name.chars() {
            let cw = c.width().unwrap_or(0);
            if used + cw > cols.saturating_sub(1) {
                break;
            }
            out.push(c);
            used += cw;
        }
        out.push('…');
        out.push_str(&" ".repeat(cols.saturating_sub(used + 1)));
        out
    }
}

/// The longest prefix of `s` (on a char boundary) whose display width fits
/// within `room` columns. Always takes at least one character when `s` is
/// non-empty, even if that character alone is wider than `room`, so a
/// single oversized glyph can never stall the wrapping loop below.
fn take_within_width(s: &str, room: usize) -> &str {
    let mut used = 0;
    let mut end = 0;
    for ch in s.chars() {
        let cw = ch.width().unwrap_or(0);
        if used > 0 && used + cw > room {
            break;
        }
        used += cw;
        end += ch.len_utf8();
        if used >= room {
            break;
        }
    }
    &s[..end]
}

/// Greedily packs the whitespace-separated words of `note` onto lines of at
/// most `width` display columns, indenting every continuation line with
/// `hanging_indent` spaces so wrapped text stays aligned under the first
/// line. A word that doesn't fit even alone on an empty line is hard-broken
/// across as many lines as it takes — notes are the payload of this tool, so
/// silently dropping part of one at the right edge is not an option.
fn wrap_note(note: &str, width: u16, hanging_indent: usize) -> Vec<String> {
    let width = usize::from(width).max(1);
    // Cap the indent itself so a pathologically narrow width (a terminal a
    // few columns wide) can't make a continuation line's indent alone
    // overshoot `width` before any content is even added.
    let hanging_indent = hanging_indent.min(width.saturating_sub(1));
    let mut has_word = false;

    let indent_for = |lines: &[String]| -> String {
        if lines.is_empty() {
            String::new()
        } else {
            " ".repeat(hanging_indent)
        }
    };
    let mut lines: Vec<String> = Vec::new();
    let mut line = indent_for(&lines);

    for word in note.split_whitespace() {
        let word_width = word.width();
        let sep = usize::from(has_word);

        if line.width() + sep + word_width <= width {
            if has_word {
                line.push(' ');
            }
            line.push_str(word);
            has_word = true;
            continue;
        }

        if has_word {
            lines.push(std::mem::take(&mut line));
            line = indent_for(&lines);
            has_word = false;
        }

        if line.width() + word_width <= width {
            line.push_str(word);
            has_word = true;
            continue;
        }

        // Doesn't fit even alone on a fresh line: hard-break it.
        let mut remaining = word;
        while !remaining.is_empty() {
            let room = width.saturating_sub(line.width()).max(1);
            let take = take_within_width(remaining, room);
            line.push_str(take);
            remaining = &remaining[take.len()..];
            if remaining.is_empty() {
                has_word = true;
            } else {
                lines.push(std::mem::take(&mut line));
                line = indent_for(&lines);
            }
        }
    }

    if has_word || lines.is_empty() {
        lines.push(line);
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::App;
    use crate::tui::context::TuiContext;
    use crate::tui::testing::{
        fixture_day, fixture_day_with_notes, render_to_string, row_containing,
    };
    use crate::tui::ui::MIN_ROWS;
    use time_tracking_parser::ProjectSummary;

    /// A project with `notes`, for tests that need a variable note count —
    /// `crate::tui::testing::project` fixes it at two.
    fn project(
        name: &str,
        total_minutes: u32,
        notes: impl IntoIterator<Item = &'static str>,
    ) -> ProjectSummary {
        ProjectSummary {
            name: name.to_owned(),
            total_minutes,
            notes: notes.into_iter().map(str::to_owned).collect(),
        }
    }

    #[test]
    fn pads_by_display_width_not_char_count() {
        // Each CJK glyph occupies two columns.
        assert_eq!(pad_display_width("日本語", 10).width(), 10);
        assert_eq!(pad_display_width("abc", 10).width(), 10);
    }

    #[test]
    fn ascii_name_longer_than_cols_truncates_to_exact_width() {
        let padded = pad_display_width("abcdefghijklmnopqrstuvwxyz", 10);
        assert_eq!(padded.width(), 10);
        assert!(
            padded.ends_with('…'),
            "should end with an ellipsis: {padded:?}"
        );
    }

    #[test]
    fn cjk_name_longer_than_cols_truncates_to_exact_width() {
        // Each glyph is 2 columns wide, so the last one that fits can land
        // one column short of the ellipsis; the arithmetic must pad that
        // column rather than assume every glyph is 1 column wide.
        let padded = pad_display_width("日本語のプログラミング入門", 10);
        assert_eq!(padded.width(), 10);
        assert!(
            padded.contains('…'),
            "should contain an ellipsis: {padded:?}"
        );
    }

    #[test]
    fn emoji_name_longer_than_cols_truncates_to_exact_width() {
        // Emoji are also 2 columns wide; same boundary concern as CJK.
        let padded = pad_display_width("🎉🎉🎉🎉🎉🎉", 10);
        assert_eq!(padded.width(), 10);
        assert!(
            padded.contains('…'),
            "should contain an ellipsis: {padded:?}"
        );
    }

    #[test]
    fn wraps_long_notes_with_a_hanging_indent() {
        let lines = wrap_note("alpha beta gamma delta epsilon", 16, 5);
        assert!(lines.len() > 1, "a 30-char note must wrap at width 16");
        assert!(lines[0].len() <= 16);
        assert!(
            lines[1].starts_with("     "),
            "continuation lines are indented"
        );
        for l in &lines {
            assert!(l.width() <= 16, "no line may exceed the width");
        }
    }

    #[test]
    fn a_word_longer_than_the_width_is_hard_broken_not_dropped() {
        let lines = wrap_note("supercalifragilisticexpialidocious", 10, 2);
        let joined: String = lines.iter().map(|l| l.trim().to_string()).collect();
        assert!(
            joined.contains("supercali"),
            "content must survive wrapping"
        );
    }

    #[test]
    fn body_is_rebuilt_when_the_width_changes() {
        let item = ProjectItem::new("admin".into(), 1.0, vec!["a fairly long note here".into()]);
        let narrow = item.body(20).height();
        let wide = item.body(80).height();
        assert!(narrow > wide, "a narrower pane needs more lines");
    }

    #[test]
    fn body_is_reused_at_the_same_width() {
        let item = ProjectItem::new("admin".into(), 1.0, vec!["note".into()]);
        let a = item.body(40);
        let b = item.body(40);
        assert_eq!(a, b);
        assert_eq!(item.rebuild_count(), 1, "same width must not rebuild");
    }

    #[test]
    fn hanging_indent_never_exceeds_a_pathologically_narrow_width() {
        // At width 3 an uncapped 5-space hanging indent would overshoot
        // every continuation line before any content is even added.
        let lines = wrap_note("alpha beta gamma", 3, 5);
        for l in &lines {
            assert!(l.width() <= 3, "line {l:?} exceeds width 3");
        }
    }

    /// `Enter` answers with an intent; the clipboard connection and the
    /// status line that reports a machine without one belong to `App`.
    #[test]
    fn enter_emits_a_copy_intent_rather_than_doing_io() {
        let mut widget = ProjectListWidget::new(&fixture_day(), &Theme::none());

        match widget.apply(&AppEvent::CopyNotes) {
            Handled::Emit(AppEvent::CopyToClipboard(payload, message)) => {
                assert_eq!(
                    payload, "- standup\n- inbox triage",
                    "notes are copied as a bullet list"
                );
                assert!(
                    message.contains("admin"),
                    "the toast names the project: {message:?}"
                );
            }
            other => panic!("expected a copy intent, got {other:?}"),
        }
    }

    /// An empty payload rather than a lone `"- "`: `App` reads it as "nothing
    /// to copy" and leaves whatever the user already had on the clipboard.
    #[test]
    fn a_project_with_no_notes_yields_an_empty_payload() {
        let mut data = fixture_day();
        data.projects[0].notes.clear();
        let mut widget = ProjectListWidget::new(&data, &Theme::none());

        match widget.apply(&AppEvent::CopyNotes) {
            Handled::Emit(AppEvent::CopyToClipboard(payload, _)) => {
                assert!(payload.is_empty(), "got {payload:?}");
            }
            other => panic!("expected a copy intent, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn the_header_shows_dead_time() {
        let mut data = fixture_day();
        data.dead_time_minutes = 95;
        let mut app = App::new(TuiContext::for_test()).with_data(data);
        let screen = render_to_string(&mut app, 100, 30);
        assert!(screen.to_lowercase().contains("dead"), "got:\n{screen}");
    }

    #[tokio::test]
    async fn parser_warnings_are_rendered() {
        let mut data = fixture_day();
        data.warnings = vec!["Error parsing time range 'x-y'".into()];
        let mut app = App::new(TuiContext::for_test()).with_data(data);
        let screen = render_to_string(&mut app, 100, 30);
        assert!(
            screen.contains("Error parsing time range"),
            "got:\n{screen}"
        );
    }

    /// The header-band half of the blank-list failure, pinned at the size
    /// the app advertises as its floor. The day's warnings block used to
    /// grow a row per warning, uncapped, against a `Fill(1)` list that
    /// cannot outrank a `Length` — five warnings at 60x15 left the list a
    /// two-row viewport, which ratatui draws as nothing at all.
    ///
    /// All three assertions are load-bearing and pin different seams: the
    /// count pins [`band::warning_lines`]'s cap, `admin` pins
    /// [`clamp_item_rows`], and the *note* pins [`band::fit_band`]'s floor —
    /// without the floor the list is left a one-row viewport, which shows a
    /// project's name and none of its work. Removing any one of the three
    /// fails exactly its own assertion.
    #[tokio::test]
    async fn a_flood_of_warnings_is_capped_and_counted_without_costing_the_list() {
        let mut data = fixture_day();
        data.warnings = (0..5).map(|i| format!("bad entry {i}")).collect();
        let mut app = App::new(TuiContext::for_test()).with_data(data);
        let screen = render_to_string(&mut app, 80, MIN_ROWS);

        assert!(
            screen.contains("Warnings (5)"),
            "a capped block still has to carry the full count:\n{screen}"
        );
        assert!(
            screen.contains("admin"),
            "the warnings block must not cost the day its project list:\n{screen}"
        );
        assert!(
            screen.contains("standup"),
            "the list's floor is a whole project, not a one-line stub:\n{screen}"
        );
    }

    /// The other half: ratatui 0.29 draws *zero* items when the first is
    /// taller than the viewport, so a project with more notes than the pane
    /// has rows used to blank the whole list rather than clip one item.
    #[tokio::test]
    async fn a_project_with_more_notes_than_the_pane_still_renders() {
        let mut app =
            App::new(TuiContext::for_test()).with_data(fixture_day_with_notes("solo", 40));
        let screen = render_to_string(&mut app, 80, 24);

        assert!(
            screen.contains("solo"),
            "an over-tall item must clip, not vanish:\n{screen}"
        );
        assert!(
            screen.contains("note 0"),
            "the notes that do fit must still be shown:\n{screen}"
        );
        assert!(
            screen.contains("… +"),
            "the notes that don't fit must be counted, not silently dropped:\n{screen}"
        );
    }

    #[tokio::test]
    async fn a_clean_day_renders_no_warning_block() {
        let mut app = App::new(TuiContext::for_test()).with_data(fixture_day());
        let screen = render_to_string(&mut app, 100, 30);
        assert!(!screen.to_lowercase().contains("warning"), "got:\n{screen}");
    }

    /// Regression guard for the header's height calculation: a naive
    /// implementation that always reserved rows for a warnings block (even
    /// an empty one) would eat into the list below it. On a clean day the
    /// last project's last note must still fit at the same terminal size
    /// the other header tests use.
    #[tokio::test]
    async fn a_clean_day_loses_no_list_rows_to_an_empty_warning_block() {
        let mut app = App::new(TuiContext::for_test()).with_data(fixture_day());
        let screen = render_to_string(&mut app, 100, 30);
        assert!(
            screen.contains("release notes"),
            "the last project's last note should still fit with no warnings to show:\n{screen}"
        );
    }

    /// A clean day must not claim dead time exists: the label itself is
    /// unpinned without this, unlike its presence (covered above).
    #[tokio::test]
    async fn a_clean_day_shows_no_dead_time_label() {
        let mut app = App::new(TuiContext::for_test()).with_data(fixture_day());
        let screen = render_to_string(&mut app, 100, 30);
        assert!(!screen.contains("Dead Time"), "got:\n{screen}");
    }

    /// Regression: at the 60-column floor Task 24 establishes, the pane
    /// border leaves a 58-column header. With the raw `total_minutes as
    /// f32 / 60.` `Display` (`7.6833334 hours`) and no wrapping on the
    /// header `Paragraph`, an ordinary non-round-hour working day
    /// silently truncated mid-word — no ellipsis, no indicator. This pair
    /// is the exact one that was found truncating.
    #[tokio::test]
    async fn an_awkward_working_and_dead_time_pair_is_not_truncated_at_60_columns() {
        let mut data = fixture_day();
        data.total_minutes = 461;
        data.dead_time_minutes = 95;
        let mut app = App::new(TuiContext::for_test()).with_data(data);
        let screen = render_to_string(&mut app, 60, 30);
        assert!(
            screen.contains("Working Time: 7.68 hours"),
            "got:\n{screen}"
        );
        assert!(
            screen.contains("Dead Time: 1:35 (1.58 hours)"),
            "the dead-time figure must survive whole, not cut off mid-word:\n{screen}"
        );
    }

    /// Worst case over the realistic `(total_minutes, dead_time_minutes)`
    /// space: both halves at their widest (a double-digit hour count on
    /// each side) no longer fit on one 58-column row even with bounded
    /// `{:.2}` formatting, so `working_time_lines` must fall back to two
    /// rows rather than truncate the shared one.
    #[tokio::test]
    async fn the_widest_pair_falls_back_to_two_rows_instead_of_truncating() {
        let mut data = fixture_day();
        data.total_minutes = 600;
        data.dead_time_minutes = 600;
        let mut app = App::new(TuiContext::for_test()).with_data(data);
        let screen = render_to_string(&mut app, 60, 30);
        assert!(
            screen.contains("Working Time: 10.00 hours"),
            "got:\n{screen}"
        );
        assert!(
            screen.contains("Dead Time: 10:00 (10.00 hours)"),
            "got:\n{screen}"
        );
    }

    /// A day whose projects have different note counts: rows are
    /// variable-height, since `render_list` builds each item from
    /// `clamp_item_rows(body(width), viewport_rows)`.
    fn uneven_day() -> TimeTrackingData {
        TimeTrackingData {
            total_minutes: 180,
            dead_time_minutes: 0,
            projects: vec![
                project("one-note", 60, ["a"]),
                project("three-notes", 60, ["a", "b", "c"]),
                project("two-notes", 60, ["a", "b"]),
            ],
            warnings: Vec::new(),
            start_time: None,
            end_time: None,
        }
    }

    /// Render `data`'s pane into `area` and assert that every project
    /// hit-tests back from the row it was actually drawn on.
    ///
    /// Rows are read off the rendered buffer rather than derived from
    /// `area` — a hit-test checked against a hand-built `Rect` is checked
    /// against its own arithmetic and agrees with a wrong answer as
    /// readily as with a right one. What that missed: `render` splits the
    /// pane into a header band and a list, and `index_at` was resolving
    /// rows against the *pane*, so every click landed some projects short.
    fn assert_every_project_hit_tests_back(data: &TimeTrackingData, area: Rect) {
        let theme = Theme::none();
        let mut widget = ProjectListWidget::new(data, &theme);
        let mut buf = Buffer::empty(Rect::new(0, 0, area.width, area.y + area.height));
        (&mut widget).render(area, &mut buf);

        for (index, project) in data.projects.iter().enumerate() {
            let y = row_containing(&buf, &project.name);
            assert_eq!(
                widget.index_at(area, y),
                Some(index),
                "`{}` is drawn on row {y}",
                project.name
            );
        }
    }

    /// The load-bearing test for this pane's hit-test: wherever `index_at`
    /// claims a project is, that is where the pane actually drew it.
    ///
    /// Variable row heights are part of the same assertion: an index that
    /// was divided out of `y` rather than walked would disagree here too.
    #[test]
    fn index_at_agrees_with_the_rows_the_list_drew() {
        assert_every_project_hit_tests_back(&uneven_day(), Rect::new(0, 2, 60, 28));
    }

    /// The header band is not a fixed height — it grows with dead time and
    /// with warnings — so a hit-test that happened to track it on a clean
    /// day would still be wrong on an ordinary one. Same assertion, a
    /// header several rows taller.
    #[test]
    fn index_at_follows_a_header_grown_by_dead_time_and_warnings() {
        let data = TimeTrackingData {
            dead_time_minutes: 45,
            warnings: vec!["Error parsing time range '9-'".to_owned()],
            ..uneven_day()
        };
        assert_every_project_hit_tests_back(&data, Rect::new(0, 2, 60, 28));
    }

    /// The pane rect starts at the header band, not at the first project
    /// row, so every row above the first project resolves to nothing —
    /// the last row of the header included, which is the one the old
    /// arithmetic handed to a project further down the list.
    #[test]
    fn index_at_ignores_clicks_above_the_first_project_row() {
        let data = uneven_day();
        let theme = Theme::none();
        let mut widget = ProjectListWidget::new(&data, &theme);
        let area = Rect::new(0, 5, 60, 28);
        let mut buf = Buffer::empty(Rect::new(0, 0, area.width, area.y + area.height));
        (&mut widget).render(area, &mut buf);

        let first_row = row_containing(&buf, &data.projects[0].name);
        for y in area.y..first_row {
            assert_eq!(
                widget.index_at(area, y),
                None,
                "row {y} is above the first project row ({first_row})"
            );
        }
    }

    #[test]
    fn select_index_moves_the_selection() {
        let theme = Theme::none();
        let mut widget = ProjectListWidget::new(&fixture_day(), &theme);
        widget.select_index(2);
        assert_eq!(widget.selected_item(), Some(2));
    }

    #[test]
    fn select_index_past_the_end_is_ignored() {
        let theme = Theme::none();
        let mut widget = ProjectListWidget::new(&fixture_day(), &theme);
        let before = widget.selected_item();
        widget.select_index(999);
        assert_eq!(widget.selected_item(), before);
    }
}
