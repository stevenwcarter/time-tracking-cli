//! [`Overlay::DatePrompt`](crate::tui::mode::Overlay::DatePrompt)'s parser
//! and its rendering.
//!
//! The overlay's own state — the typed buffer — and its editing keys live on
//! [`App`](crate::tui::app::App); this module only turns that buffer into a
//! [`Date`] and draws it, the same split [`super::help_popup`] keeps from the
//! keymap it reads.

use ratatui::layout::Flex;
use ratatui::prelude::*;
use time::{Date, OffsetDateTime};

use crate::tui::theme::Theme;
use crate::tui::widgets::Popup;

/// Parse `input` as a date, exactly the way `ttcli --date`/`ttcli 'last
/// friday'` do.
///
/// Delegates entirely to [`interim::parse_date_string`] rather than growing a
/// second date grammar — `config.rs`'s own call for the CLI's `--date` flag
/// is the sibling this mirrors — so anything the command line accepts, the
/// TUI's prompt accepts too.
pub fn parse_prompt(input: &str, now: OffsetDateTime) -> Result<Date, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Enter a date".into());
    }
    interim::parse_date_string(trimmed, now, interim::Dialect::Us)
        .map(|dt| dt.date())
        .map_err(|_| format!("Could not parse date: {trimmed}"))
}

/// Columns/rows of margin kept clear around the box, matching
/// [`super::help_popup`]'s.
const SCREEN_MARGIN: u16 = 4;
/// The box's interior width in columns — enough for a full phrase like "the
/// friday before last" with room left to keep typing.
const INPUT_COLS: u16 = 32;
/// Rows/columns the border costs.
const BORDER: u16 = 2;

/// The jump-to-date prompt: a one-line bordered box holding what has been
/// typed so far, with a reverse-video cursor block after it.
pub struct DatePrompt<'a> {
    theme: &'a Theme,
    input: &'a str,
}

impl<'a> DatePrompt<'a> {
    /// A prompt showing `input`, drawn in `theme`.
    pub fn new(theme: &'a Theme, input: &'a str) -> Self {
        Self { theme, input }
    }
}

impl Widget for DatePrompt<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = popup_area(area);
        // A single trailing space in reverse video stands in for a cursor:
        // ratatui has no real terminal cursor to place inside a widget, and
        // this is the same trick a shell's line editor renders as when it
        // draws its own prompt rather than relying on the terminal's.
        let line = Line::from(vec![
            Span::raw(self.input),
            Span::styled(" ", Style::new().add_modifier(Modifier::REVERSED)),
        ]);
        Popup::default()
            .content(line)
            .style(self.theme.status)
            .title("Jump to date")
            .title_style(self.theme.status.add_modifier(Modifier::BOLD))
            .border_style(self.theme.warning)
            .render(popup, buf);
    }
}

/// Centre a fixed-size, one-line box, capped at the screen the same way
/// [`super::help_popup`]'s popup is.
fn popup_area(area: Rect) -> Rect {
    let width = (INPUT_COLS + BORDER).min(area.width.saturating_sub(SCREEN_MARGIN));
    let height = (1 + BORDER).min(area.height.saturating_sub(SCREEN_MARGIN));
    let vertical = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{date, datetime};

    #[test]
    fn parses_an_iso_date() {
        let now = datetime!(2026 - 08 - 28 12:00 UTC);
        assert_eq!(
            parse_prompt("2026-08-14", now).unwrap(),
            date!(2026 - 08 - 14)
        );
    }

    #[test]
    fn parses_a_natural_language_date() {
        // A Monday, deliberately not a Friday: `interim` 0.2.1 only applies
        // its next/last correction by comparing *times* when the named
        // weekday is the same as `now`'s, and the implied midnight target is
        // never later than `now`'s own time — so "last friday" asked on a
        // Friday resolves to today rather than a week back. That is a
        // property of the shared parser this function deliberately does not
        // work around (see its doc comment), not something to pin here.
        let now = datetime!(2026 - 08 - 31 12:00 UTC);
        assert_eq!(
            parse_prompt("last friday", now).unwrap(),
            date!(2026 - 08 - 28)
        );
    }

    #[test]
    fn rejects_gibberish_with_a_message() {
        let now = datetime!(2026 - 08 - 28 12:00 UTC);
        assert!(parse_prompt("not a date at all", now).is_err());
    }

    /// Whitespace alone must not reach `interim`: an empty prompt is a
    /// different failure than a *parseable-looking* one, and deserves its
    /// own message rather than whatever `interim` says about `""`.
    #[test]
    fn blank_input_is_rejected_without_reaching_the_parser() {
        let now = datetime!(2026 - 08 - 28 12:00 UTC);
        assert_eq!(parse_prompt("   ", now), Err("Enter a date".to_owned()));
    }

    #[test]
    fn the_typed_text_and_a_trailing_cursor_are_both_on_screen() {
        let theme = Theme::dark();
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let input = "2026-08";
        DatePrompt::new(&theme, input).render(area, &mut buf);

        let popup = popup_area(area);
        let text_row = popup.y + 1;
        let text_col = popup.x + 1;

        let rendered: String = (0..input.chars().count() as u16)
            .map(|i| buf[(text_col + i, text_row)].symbol().to_owned())
            .collect();
        assert_eq!(rendered, input);

        let cursor_col = text_col + input.chars().count() as u16;
        assert!(
            buf[(cursor_col, text_row)]
                .modifier
                .contains(Modifier::REVERSED),
            "the cell after the typed text should be the reverse-video cursor"
        );
    }

    /// Cheap insurance alongside `ui::no_render_panics_at_any_plausible_size`:
    /// this widget's own fixed-size math must not panic at the extremes that
    /// sweep drives it through.
    #[test]
    fn renders_without_panicking_at_extreme_sizes() {
        let theme = Theme::dark();
        for (w, h) in [(0, 0), (1, 1), (3, 3), (200, 60)] {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            DatePrompt::new(&theme, "last friday").render(area, &mut buf);
        }
    }
}
