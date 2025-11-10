use ratatui::layout::Flex;
use ratatui::prelude::*;

use crate::tui::widgets::Popup;

#[derive(Default)]
pub struct HelpPopup {}

impl Widget for &mut HelpPopup {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = popup_area(area, 60, 60);
        let popup = Popup::default()
            .content(
                "↓↑ or j/k: select project to copy to clipboard
                    g/G: to go to the top or bottom
                    r: to reload data from disk
                    e: edit the current date's notes in $EDITOR
                    f: toggle zooming into the weekly bar chart
                    Enter: copy the notes for the current project to your clipboard.",
            )
            .style(Style::new().yellow())
            .title("Help")
            .title_style(Style::new().white().bold())
            .border_style(Style::new().red());
        popup.render(area, buf);
    }
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
