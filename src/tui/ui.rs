use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::DATE_FORMAT;

use super::widgets::{Calendar, WeeklyBarChart};

use super::app::App;
use super::widgets::HelpPopup;

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.zoom_bar {
            self.weekly_bar_chart().render(area, buf);
            return;
        }
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(12), Constraint::Min(9)].as_ref())
            .split(area);
        let header_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Fill(1)])
            .split(chunks[0]);
        let calendar_area = header_area[0];
        let bar_chart_area = header_area[1];

        Calendar::new(self.active_date, &self.populated_dates, &self.ctx.theme)
            .render(calendar_area, buf);

        // Create and render the weekly bar chart
        self.weekly_bar_chart().render(bar_chart_area, buf);

        if let Some(group_rect) = bounding_rect(&header_area) {
            Block::default()
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .title("tt-tui")
                .render(group_rect, buf);
        }

        let block = Block::bordered()
            .title(self.active_date.format(DATE_FORMAT).unwrap())
            .border_type(BorderType::Rounded);

        if let Some(widget) = &mut self.project_list_widget {
            widget.render(chunks[1], buf);
        } else {
            let tt_par = Paragraph::new("No data found for date")
                .block(block)
                .style(self.ctx.theme.warning)
                .alignment(Alignment::Left);
            tt_par.render(chunks[1], buf);
        }

        if self.show_help {
            HelpPopup::default().render(area, buf);
        }
    }
}

impl App {
    /// Build the weekly bar chart for the active date, pre-populated with any
    /// week data that has already been loaded.
    fn weekly_bar_chart(&self) -> WeeklyBarChart<'_> {
        let mut bar_chart =
            WeeklyBarChart::new(self.active_date, &self.week_dates, &self.ctx.theme);
        if !self.weekly_data.is_empty() {
            bar_chart.set_weekly_data(&self.weekly_data);
        }
        bar_chart
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
