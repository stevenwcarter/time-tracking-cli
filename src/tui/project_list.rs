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

#[derive(Debug)]
pub struct ProjectListWidget {
    start_time: String,
    end_time: String,
    total_minutes: u32,
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
            total_minutes: data.total_minutes,
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
            // Task 12 turns this into `Handled::Emit(CopyToClipboard(..))`.
            AppEvent::CopyNotes => self.copy_selected_notes_to_clipboard(),
            _ => return Handled::Ignored,
        }
        Handled::Consumed
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

    fn copy_selected_notes_to_clipboard(&self) {
        if let Some(selected) = self.project_list.state.selected()
            && let Some(project) = self.project_list.items.get(selected)
        {
            let notes_text = format!("- {}", project.tasks.join("\n- "));

            use copypasta::ClipboardProvider;
            match copypasta::ClipboardContext::new() {
                Ok(mut ctx) => {
                    if let Err(e) = ctx.set_contents(notes_text) {
                        tracing::warn!("Failed to copy notes to clipboard: {e}");
                    }
                }
                Err(e) => tracing::warn!("Failed to access clipboard: {e}"),
            }
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
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(self.header_height()),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        self.render_header(header_area, buf);
        self.render_footer(footer_area, buf);
        self.render_list(main_area, buf);
    }
}

impl ProjectListWidget {
    /// Rows the header needs: four for the start/end/working-time block,
    /// which is always present, plus a blank separator and one row per
    /// warning when there are any. A clean day — no warnings — never pays
    /// for a block it doesn't show, so the list below keeps every row it
    /// had before this feature existed.
    fn header_height(&self) -> u16 {
        const BASE_ROWS: u16 = 4;
        if self.warnings.is_empty() {
            return BASE_ROWS;
        }
        let extra = 1 + self.warnings.len();
        BASE_ROWS + u16::try_from(extra).unwrap_or(u16::MAX)
    }

    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = vec![
            Line::from(format!("Start Time: {}", self.start_time)),
            Line::from(format!("  End Time: {}", self.end_time)),
            Line::from(""),
            self.working_time_line(),
        ];

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

    /// The "Working Time" line, with a "Dead Time" span appended whenever
    /// the day has any — styled `theme.warning` below
    /// [`DEAD_TIME_ERROR_THRESHOLD_MINUTES`] and `theme.error` at or above
    /// it, matching `format_day_summary_impl` in `src/display/mod.rs` so
    /// the TUI and the CLI never disagree about the same file. A day with
    /// no dead time renders nothing extra, so it costs no header width.
    fn working_time_line(&self) -> Line<'static> {
        let working = format!("Working Time: {} hours", self.total_minutes as f32 / 60.);
        if self.dead_time_minutes == 0 {
            return Line::from(working);
        }

        let style = if self.dead_time_minutes < DEAD_TIME_ERROR_THRESHOLD_MINUTES {
            self.theme.warning
        } else {
            self.theme.error
        };
        Line::from(vec![
            Span::raw(format!("{working}    ")),
            Span::styled(
                format!(
                    "Dead Time: {} ({} hours)",
                    self.dead_time, self.dead_decimal
                ),
                style,
            ),
        ])
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("? for help")
            .wrap(Wrap { trim: true })
            .centered()
            .render(area, buf);
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
}
