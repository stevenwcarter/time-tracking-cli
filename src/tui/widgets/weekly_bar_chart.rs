use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::HashMap;
use time::Date;

use crate::time_utils::WeekdayExt;
use crate::tui::theme::Theme;

pub struct WeeklyBarChart<'a> {
    active_date: Date,
    week_dates: &'a [Date; 7],
    theme: &'a Theme,
    week_data: Option<&'a HashMap<Date, u32>>, // Date -> total minutes
}

impl<'a> WeeklyBarChart<'a> {
    pub fn new(active_date: Date, week_dates: &'a [Date; 7], theme: &'a Theme) -> Self {
        Self {
            active_date,
            week_dates,
            theme,
            week_data: None,
        }
    }

    pub fn set_weekly_data(&mut self, data: &'a HashMap<Date, u32>) {
        self.week_data = Some(data);
    }

    /// Calculate total hours for the week
    fn calculate_total_weekly_hours(&self) -> f64 {
        if let Some(week_data) = self.week_data {
            let total_minutes: u32 = week_data.values().sum();
            total_minutes as f64 / 60.0
        } else {
            0.0
        }
    }

    /// Per-day bar data for the week: the date, its value scaled to tenths
    /// of an hour (so `BarChart`'s integer axis can carry one decimal
    /// place), and the text shown at the foot of the bar.
    ///
    /// Split out of `prepare_bars` because `Bar`'s `value` field is private
    /// in ratatui 0.29, so a test can't read it back off a built `Bar` —
    /// it asserts against this instead.
    fn bar_values(&self, bar_width: u16) -> Vec<(Date, u64, String)> {
        let week_data = self.week_data;

        self.week_dates
            .iter()
            .map(|&date| {
                let minutes = week_data
                    .and_then(|data| data.get(&date))
                    .copied()
                    .unwrap_or(0);
                let hours = minutes as f64 / 60.0;
                let value = u64::from(minutes) * 10 / 60; // Scale by 10 for one decimal place; integer avoids lossy float→u64 cast

                // Format text to fit within bar width dynamically
                let text_value = if bar_width >= 5 {
                    // Wide bars can show full precision
                    format!("{:.1}h", hours) // "10.5h"
                } else if bar_width >= 4 {
                    // Medium bars: show integer for 10+, decimal for <10
                    if hours >= 10.0 {
                        format!("{:.0}h", hours) // "10h"
                    } else {
                        format!("{:.1}h", hours) // "8.5h"
                    }
                } else {
                    // Narrow bars: just show integer hours
                    format!("{:.0}h", hours) // "10h" or "8h"
                };

                (date, value, text_value)
            })
            .collect()
    }

    fn prepare_bars(&self, bar_width: u16) -> Vec<Bar<'_>> {
        if self.week_data.is_none() {
            return vec![]; // Empty data if not loaded
        }

        self.bar_values(bar_width)
            .into_iter()
            .map(|(date, value, text_value)| {
                let day_name = date.weekday().short_name();

                // Create label with day name and day of month
                let label = format!("{}\n{}", day_name, date.day());

                let style = if date == self.active_date {
                    self.theme.active_date
                } else if value > 0 {
                    self.theme.populated_date
                } else {
                    self.theme.inactive_date
                };

                Bar::default()
                    .value(value)
                    .label(Line::from(label).style(style)) // Day abbreviation and day of month below the bar
                    .text_value(text_value) // Hours at bottom of bar
                    .style(style)
                    .value_style(style)
            })
            .collect()
    }
    /// Calculate dynamic bar dimensions based on available area
    fn calculate_bar_dimensions(&self, area: Rect) -> (u16, u16, u64) {
        // Account for borders and padding
        let content_width = area.width.saturating_sub(4); // left border + padding
        let content_height = area.height.saturating_sub(4); // top/bottom borders + padding + title

        // Calculate optimal bar width for 7 days with gaps
        // Formula: (total_width - gaps) / bars = bar_width
        // We want 6 gaps between 7 bars, so: content_width = 7*bar_width + 6*gap
        let min_gap = 1;
        let num_bars = 7;
        let total_gaps = (num_bars - 1) * min_gap;

        let bar_width = if content_width > total_gaps {
            (content_width - total_gaps) / num_bars
        } else {
            1 // Minimum bar width
        };

        // Ensure bar width is at least wide enough for common text ("10h" = 3 chars minimum)
        let bar_width = bar_width.max(3);

        // Calculate gap based on remaining space
        let used_width = num_bars * bar_width;
        let bar_gap = if used_width < content_width {
            (content_width - used_width) / (num_bars - 1).max(1)
        } else {
            min_gap
        };

        // Calculate max value based on available height
        // More height = can show taller bars = higher max value
        let max_value = (content_height as u64 * 10).max(160); // At least 16 hours, scale up with height

        (bar_width, bar_gap, max_value)
    }
}

impl Widget for &mut WeeklyBarChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let title = format!("{} {}", self.active_date.month(), self.active_date.year());

        // Calculate dynamic dimensions
        let (bar_width, bar_gap, max_value) = self.calculate_bar_dimensions(area);

        // Prepare bars with responsive formatting
        let bars = self.prepare_bars(bar_width);

        // Calculate total weekly hours for display
        let total_hours = self.calculate_total_weekly_hours();
        let total_text = format!("{:.1}h total", total_hours);

        // Create the block with title and total hours
        let block = Block::default()
            .padding(Padding {
                left: 1,
                top: 1,
                bottom: 1, // Extra bottom padding for day labels with day of month
                ..Padding::default()
            })
            .border_type(BorderType::Rounded)
            .title(title);

        // Render the block first
        let inner_area = block.inner(area);
        block.render(area, buf);

        // Render total hours in upper right corner
        let total_area = Rect {
            x: area.x + area.width.saturating_sub(total_text.len() as u16 + 2),
            y: area.y + 1,
            width: total_text.len() as u16,
            height: 1,
        };

        Paragraph::new(total_text)
            .style(self.theme.warning)
            .render(total_area, buf);

        // Render the chart in the inner area
        let chart = BarChart::default()
            .data(BarGroup::default().bars(&bars))
            .bar_width(bar_width)
            .bar_gap(bar_gap)
            .max(max_value);

        chart.render(inner_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::Theme;
    use time::macros::date;

    fn week() -> [Date; 7] {
        [
            date!(2026 - 08 - 22),
            date!(2026 - 08 - 23),
            date!(2026 - 08 - 24),
            date!(2026 - 08 - 25),
            date!(2026 - 08 - 26),
            date!(2026 - 08 - 27),
            date!(2026 - 08 - 28),
        ]
    }

    #[test]
    fn bar_values_come_from_the_passed_week_not_the_global_config() {
        let theme = Theme::none();
        let week = week();
        let mut data = HashMap::new();
        data.insert(date!(2026 - 08 - 24), 480u32); // 8h on the third day
        let mut chart = WeeklyBarChart::new(date!(2026 - 08 - 24), &week, &theme);
        chart.set_weekly_data(&data);

        let values = chart.bar_values(6);

        assert_eq!(values.len(), 7);
        assert_eq!(values[2].0, date!(2026 - 08 - 24));
        assert_eq!(values[2].1, 80, "8h in tenths of an hour");
        assert_eq!(
            values[0].1, 0,
            "a day with no data is a zero bar, not absent"
        );
    }
}
