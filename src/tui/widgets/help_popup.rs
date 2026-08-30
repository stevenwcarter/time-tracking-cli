use ratatui::layout::Flex;
use ratatui::prelude::*;

use crate::tui::keymap::help_rows;
use crate::tui::mode::Mode;
use crate::tui::theme::Theme;
use crate::tui::widgets::Popup;

/// Separator between the key column and the description column.
const COLUMN_GAP: &str = "  ";
/// Rows and columns the popup's border costs.
const BORDER: usize = 2;
/// Columns of breathing room inside the border.
const SIDE_PADDING: usize = 2;

/// The help overlay, listing the keys that do something in the current mode.
///
/// Every line comes from the one binding table in
/// [`keymap`](crate::tui::keymap), so the popup cannot drift from what the
/// keys actually do.
pub struct HelpPopup<'a> {
    theme: &'a Theme,
    mode: Mode,
}

impl<'a> HelpPopup<'a> {
    /// A help popup for `mode`, drawn in `theme`.
    pub fn new(theme: &'a Theme, mode: Mode) -> Self {
        Self { theme, mode }
    }

    /// The lines to list: the keys `mode` binds, in a column, grouped as
    /// [`help_rows`] groups them.
    fn content(&self) -> String {
        let rows = help_rows(self.mode);
        let keys_width = rows
            .iter()
            .map(|(keys, _)| keys.chars().count())
            .max()
            .unwrap_or(0);
        rows.iter()
            .map(|(keys, description)| {
                // The blank pairs `help_rows` puts between groups.
                if keys.is_empty() {
                    String::new()
                } else {
                    format!("{keys:<keys_width$}{COLUMN_GAP}{description}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Where [`HelpPopup::render`] would draw its box in `area`.
    ///
    /// Recomputes the content rather than caching it: the popup is sized to
    /// fit its rows, so the rect cannot be known without them, and this runs
    /// once per frame on a table of about thirty entries.
    pub fn popup_rect(&self, area: Rect) -> Rect {
        popup_area(area, &self.content())
    }
}

impl Widget for HelpPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let content = self.content();
        let popup = popup_area(area, &content);
        Popup::default()
            .content(content)
            .style(self.theme.warning)
            .title("Help")
            .title_style(self.theme.status.add_modifier(Modifier::BOLD))
            .border_style(self.theme.error)
            .render(popup, buf);
    }
}

/// Columns/rows of margin kept clear around the popup, so it never touches
/// the screen edge even when the content is wide or tall enough to want to.
const SCREEN_MARGIN: u16 = 4;

/// Centre a popup just big enough for `content`, capped at the screen.
///
/// Sized rather than a fixed percentage because the list grows every time a
/// binding is added; a fixed box silently clipped the last few rows. Capped
/// with `Constraint::Length`, not a flat 60% square, so the popup stays
/// readable at both extremes: a tiny terminal gets a popup no bigger than it
/// needs, and a huge one doesn't stretch the box to some arbitrary fraction
/// of the screen.
fn popup_area(area: Rect, content: &str) -> Rect {
    let widest = content
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let width =
        clamp_to_u16(widest + BORDER + SIDE_PADDING).min(area.width.saturating_sub(SCREEN_MARGIN));
    let height = clamp_to_u16(content.lines().count() + BORDER)
        .min(area.height.saturating_sub(SCREEN_MARGIN));

    let vertical = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

fn clamp_to_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::keymap::BINDINGS;

    fn rendered(mode: Mode) -> String {
        let theme = Theme::dark();
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 40));
        HelpPopup::new(&theme, mode).render(buf.area, &mut buf);
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The popup is the keymap's documentation, so every row [`help_rows`]
    /// produces for the mode has to actually be rendered — this is the
    /// check that used to be a hand-written string nobody updated. Checks
    /// against `help_rows`'s own output; see
    /// `every_bindings_table_row_for_the_mode_is_rendered_on_the_popup`
    /// below for the equivalent check against `BINDINGS` itself.
    #[test]
    fn help_rows_output_is_fully_rendered_on_the_popup() {
        for mode in [Mode::Day, Mode::Week, Mode::ZoomedWeek, Mode::RawFile] {
            let screen = rendered(mode);
            for (keys, description) in help_rows(mode) {
                if keys.is_empty() {
                    continue;
                }
                assert!(
                    screen.contains(description),
                    "{mode:?} popup is missing {keys:?}:\n{screen}"
                );
            }
        }
    }

    /// The day view's list keys belong to the day view only.
    #[test]
    fn the_popup_is_narrowed_to_the_mode() {
        assert!(rendered(Mode::Day).contains("select the next project"));
        assert!(!rendered(Mode::ZoomedWeek).contains("select the next project"));
    }

    /// Sourced from [`BINDINGS`] directly rather than `help_rows`, so this
    /// checks the popup against the actual source of truth instead of
    /// re-checking `help_rows`'s own output against itself — a binding added
    /// to the table without a row here fails this test with no edit needed.
    /// The counterpart above checks `help_rows`'s output makes it to the
    /// screen; this one checks `help_rows` didn't drop anything in the
    /// first place.
    #[test]
    fn every_bindings_table_row_for_the_mode_is_rendered_on_the_popup() {
        for mode in [Mode::Day, Mode::Week, Mode::ZoomedWeek, Mode::RawFile] {
            let screen = rendered(mode);
            for binding in BINDINGS.iter().filter(|b| b.modes.contains(mode)) {
                assert!(
                    screen.contains(binding.description),
                    "{mode:?} popup is missing {:?}:\n{screen}",
                    binding.keys
                );
            }
        }
    }

    /// Regression: date navigation used to appear in neither the popup nor
    /// the README, so a user reading either one learned no way to look at
    /// yesterday.
    #[test]
    fn help_lists_the_date_motions() {
        let screen = rendered(Mode::Day);
        for expected in [
            "go to the previous day",
            "go forward a week",
            "go forward a month",
            "jump to a date",
        ] {
            assert!(
                screen.contains(expected),
                "help omits {expected}:\n{screen}"
            );
        }
    }
}
