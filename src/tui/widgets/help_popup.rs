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

/// Centre a popup just big enough for `content`, capped at the screen.
///
/// Sized rather than a fixed percentage because the list grows every time a
/// binding is added; a fixed box silently clipped the last few rows.
fn popup_area(area: Rect, content: &str) -> Rect {
    let widest = content
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let width = clamp_to_u16(widest + BORDER + SIDE_PADDING).min(area.width);
    let height = clamp_to_u16(content.lines().count() + BORDER).min(area.height);

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

    /// The popup is the keymap's documentation, so every binding the mode
    /// carries has to be on it — this is the check that used to be a
    /// hand-written string nobody updated.
    #[test]
    fn every_binding_the_mode_carries_is_on_the_popup() {
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
}
