use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::DATE_FORMAT;

use super::widgets::Calendar;

use super::app::App;
use super::widgets::HelpPopup;

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(10), Constraint::Min(5)].as_ref())
            .split(area);
        let header_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Fill(1)])
            .split(chunks[0]);
        let calendar_area = header_area[0];

        Calendar::new(self).render(calendar_area, buf);

        if let Some(group_rect) = bounding_rect(&header_area) {
            Block::default()
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .title("tt-tui")
                .render(group_rect, buf);
        }

        let block = Block::bordered()
            .title(self.active_date.format(DATE_FORMAT).unwrap())
            // .title_alignment(Alignment::Center)
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
            HelpPopup::default().render(area, buf);
        }
    }
}

fn bounding_rect(chunks: &[Rect]) -> Option<Rect> {
    if chunks.is_empty() {
        return None;
    }

    let x = chunks.iter().map(|r| r.x).min()?;
    let y = chunks.iter().map(|r| r.y).min()?;
    let right = chunks.iter().map(|r| r.x + r.width).max()?;
    let bottom = chunks.iter().map(|r| r.y + r.height).max()?;

    Some(Rect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}
