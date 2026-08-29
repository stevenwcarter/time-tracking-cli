#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;

use ratatui::prelude::*;
use ratatui::widgets::*;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use time_tracking_parser::TimeTrackingData;

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
        let [header_area, main_area] = Layout::vertical([
            Constraint::Length(self.header_height(&working_time_lines)),
            Constraint::Fill(1),
        ])
        .areas(area);

        self.render_header(header_area, working_time_lines, buf);
        self.render_list(main_area, buf);
    }
}

impl ProjectListWidget {
    /// Rows the header needs given the already-measured
    /// `working_time_lines` ([`working_time_lines`](Self::working_time_lines)):
    /// three for the start/end block, which is always present, plus however
    /// many rows the working/dead-time row took, plus a blank separator and
    /// one row per warning when there are any. A clean day on a
    /// wide-enough terminal — no dead time, no warnings — never pays for a
    /// block it doesn't show, so the list below keeps every row it had
    /// before this feature existed.
    fn header_height(&self, working_time_lines: &[Line<'static>]) -> u16 {
        const FIXED_ROWS: u16 = 3;
        let working_rows = u16::try_from(working_time_lines.len()).unwrap_or(u16::MAX);
        let warning_rows = if self.warnings.is_empty() {
            0
        } else {
            1 + u16::try_from(self.warnings.len()).unwrap_or(u16::MAX)
        };
        FIXED_ROWS + working_rows + warning_rows
    }

    fn render_header(&self, area: Rect, working_time_lines: Vec<Line<'static>>, buf: &mut Buffer) {
        let mut lines = vec![
            Line::from(format!("Start Time: {}", self.start_time)),
            Line::from(format!("  End Time: {}", self.end_time)),
            Line::from(""),
        ];
        lines.extend(working_time_lines);

        if !self.warnings.is_empty() {
            lines.push(Line::from(""));
            lines.extend(
                self.warnings
                    .iter()
                    .map(|warning| Line::styled(warning.clone(), self.theme.error)),
            );
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
        let block = Block::new()
            .title(Line::raw("Project Summaries").centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(self.theme.list_header);

        let body_width = area.width.saturating_sub(4);
        let items: Vec<ListItem> = self
            .project_list
            .items
            .iter()
            .enumerate()
            .map(|(i, project_item)| {
                ListItem::new(project_item.body(body_width))
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
    use crate::tui::testing::{fixture_day, render_to_string};

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
}
