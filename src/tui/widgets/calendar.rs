use ratatui::prelude::*;
use ratatui::widgets::calendar::*;
use ratatui::widgets::*;
use time::Date;

use crate::tui::theme::Theme;

pub struct Calendar<'a> {
    active_date: Date,
    populated_dates: &'a [Date],
    theme: &'a Theme,
}

impl<'a> Calendar<'a> {
    pub fn new(active_date: Date, populated_dates: &'a [Date], theme: &'a Theme) -> Self {
        Self {
            active_date,
            populated_dates,
            theme,
        }
    }

    fn build_event_store(&self) -> CalendarEventStore {
        let mut es = CalendarEventStore::default();
        self.populated_dates
            .iter()
            .for_each(|d| es.add(*d, self.theme.populated_date));
        es.add(self.active_date, self.theme.active_date);

        es
    }
}

impl Widget for &mut Calendar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let calendar_block = Block::default()
            .borders(Borders::RIGHT)
            .padding(Padding {
                left: 1,
                top: 1,
                ..Padding::default()
            })
            .border_type(BorderType::Rounded);

        let es = self.build_event_store();

        Monthly::new(self.active_date, es)
            .block(calendar_block)
            .show_surrounding(self.theme.inactive_date)
            .show_month_header(Style::new().add_modifier(Modifier::BOLD))
            .show_weekdays_header(Style::new().add_modifier(Modifier::ITALIC))
            .render(area, buf);
    }
}
