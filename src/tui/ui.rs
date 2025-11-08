#![allow(dead_code)]
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{
        Block, BorderType, Paragraph, Widget,
        calendar::{CalendarEventStore, Monthly},
    },
};
use time::format_description;

use crate::tui::popup::Popup;

use super::app::App;

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(10), Constraint::Min(5)].as_ref())
            .split(area);
        let block = Block::bordered()
            .title("tt-tui")
            .title_alignment(Alignment::Center)
            // TODO: Add week bar chart
            // .padding(Padding::new(
            //     (chunks[0].width / 2) - 12,
            //     (chunks[0].width / 2) - 12,
            //     0,
            //     0,
            // ))
            .border_type(BorderType::Rounded);

        let mut es = CalendarEventStore::default();
        es.add(self.active_date, Style::new().red().bold());
        Monthly::new(self.active_date, es)
            .block(block)
            .show_month_header(Style::new().bold())
            .show_weekdays_header(Style::new().italic())
            .render(chunks[0], buf);

        let datetime_format = format_description::parse("[year]-[month]-[day]").unwrap();
        let block = Block::bordered()
            .title(self.active_date.format(&datetime_format).unwrap())
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded);

        if let Some(widget) = &mut self.project_list_widget {
            widget.render(chunks[1], buf);
        } else {
            let tt_par = Paragraph::new("No data found for date")
                .block(block)
                .fg(Color::Yellow)
                .bg(Color::Black)
                .alignment(Alignment::Left);
            tt_par.render(chunks[1], buf);
        }

        if self.show_help {
            let area = popup_area(area, 60, 60);
            let popup = Popup::default()
                .content(
                    "↓↑ or j/k: select project to copy to clipboard
                    g/G: to go to the top or bottom
                    r: to reload data from disk
                    e: edit the current date's notes in $EDITOR
                    Enter: copy the notes for the current project to your clipboard.",
                )
                .style(Style::new().yellow())
                .title("Help")
                .title_style(Style::new().white().bold())
                .border_style(Style::new().red());
            popup.render(area, buf);
        }
    }
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    // Cut the given rectangle into three vertical pieces
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    // Then cut the middle vertical piece into three width-wise pieces
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1] // Return the middle chunk
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
