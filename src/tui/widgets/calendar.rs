use ratatui::prelude::*;
use ratatui::style::palette::tailwind::*;
use ratatui::widgets::calendar::*;
use ratatui::widgets::*;

use crate::tui::app::App;
use super::colors::WidgetColors;

pub struct Calendar<'a>(&'a App);

impl<'a> Calendar<'a> {
    pub fn new(app: &'a App) -> Self {
        Self(app)
    }
    fn build_event_store(&self) -> CalendarEventStore {
        let mut es = CalendarEventStore::default();
        self.0
            .populated_dates
            .iter()
            .for_each(|d| es.add(*d, WidgetColors::populated_style()));
        es.add(self.0.active_date, WidgetColors::active_style());

        es
    }
}

impl<'a> Widget for &mut Calendar<'a> {
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

        Monthly::new(self.0.active_date, es)
            .block(calendar_block)
            .show_surrounding(Style::new().fg(SLATE.c400).add_modifier(Modifier::ITALIC))
            .show_month_header(Style::new().bold())
            .show_weekdays_header(Style::new().italic())
            .render(area, buf);
    }
}
