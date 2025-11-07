#![allow(dead_code)]
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{
        Block, BorderType, Paragraph, Widget,
        calendar::{CalendarEventStore, Monthly},
    },
};
use time::format_description;

use crate::tui::project_list::ProjectListWidget;

use super::app::App;

impl Widget for &App {
    /// Renders the user interface widgets.
    ///
    // This is where you add new widgets.
    // See the following resources:
    // - https://docs.rs/ratatui/latest/ratatui/widgets/index.html
    // - https://github.com/ratatui/ratatui/tree/master/examples
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(10),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ]
                .as_ref(),
            )
            .split(area);
        let block = Block::bordered()
            .title("tt-tui")
            .title_alignment(Alignment::Center)
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
        // let time_tracking_data = format!(
        //     "\nActive date: {}\n{}",
        //     self.active_date.format(&datetime_format).unwrap(),
        //     self.day_summary
        //         .as_deref()
        //         .unwrap_or("No data available for this date.")
        // );

        if let Some(data) = self.data.clone() {
            let mut widget = ProjectListWidget::new(&data);
            widget.render(chunks[1], buf);
        } else {
            let tt_par = Paragraph::new("No data found for date")
                .block(block)
                .fg(Color::Yellow)
                .bg(Color::Black)
                .alignment(Alignment::Left);
            tt_par.render(chunks[1], buf);
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
