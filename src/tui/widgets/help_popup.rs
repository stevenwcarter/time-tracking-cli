use ratatui::layout::Flex;
use ratatui::prelude::*;

use crate::tui::mode::Mode;
use crate::tui::theme::Theme;
use crate::tui::widgets::Popup;

/// The help overlay, listing the keys that do something in the current mode.
///
/// Task 5 replaces the hand-written text below with rows generated from the
/// single binding table, which is what stops the keymap and its documentation
/// drifting apart again.
pub struct HelpPopup<'a> {
    theme: &'a Theme,
    mode: Mode,
}

impl<'a> HelpPopup<'a> {
    /// A help popup for `mode`, drawn in `theme`.
    pub fn new(theme: &'a Theme, mode: Mode) -> Self {
        Self { theme, mode }
    }

    /// The lines to list, narrowed to the keys `mode` actually binds.
    fn content(&self) -> String {
        let mut lines = Vec::new();
        if self.mode == Mode::Day {
            lines.extend([
                "↓↑ or j/k: select a project",
                "g/G: go to the top or the bottom",
                "Enter: copy the selected project's notes to your clipboard",
            ]);
        }
        lines.extend([
            "h/l or ←/→: go to the previous or next day",
            "t: go to today",
            "r: reload data from disk",
            "e: edit the current date's notes in $EDITOR",
            "f: toggle zooming into the weekly bar chart",
            "?, Esc or q: close this popup",
        ]);
        lines.join("\n")
    }
}

impl Widget for HelpPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = popup_area(area, 60, 60);
        Popup::default()
            .content(self.content())
            .style(self.theme.warning)
            .title("Help")
            .title_style(self.theme.status.add_modifier(Modifier::BOLD))
            .border_style(self.theme.error)
            .render(area, buf);
    }
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
