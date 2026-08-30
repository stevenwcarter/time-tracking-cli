use ratatui::prelude::*;
use ratatui::widgets::calendar::*;
use ratatui::widgets::*;
use time::{Date, Duration};

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

    /// The date drawn at (`x`, `y`) when this calendar is rendered in
    /// `area`, or `None` for a cell that is not a day.
    ///
    /// # Why this duplicates ratatui
    ///
    /// [`Monthly`] exposes no hit-test, so this replicates its layout: the
    /// block (a right border and one column/row of padding), then one
    /// month-header row, one weekday-header row, then week rows in which
    /// weekday `i` occupies the two columns at `1 + 3 * i`.
    ///
    /// **Weeks start on Sunday**, because `Monthly` starts them on Sunday —
    /// it offsets from `number_days_from_sunday` internally and takes no
    /// first-day parameter. This deliberately ignores the app's
    /// `week_start_day`; using it here would be wrong on every non-Sunday
    /// configuration.
    ///
    /// `date_at_agrees_with_what_monthly_actually_drew` is what keeps this
    /// honest against a ratatui upgrade.
    pub fn date_at(&self, area: Rect, x: u16, y: u16) -> Option<Date> {
        // The block this widget renders with: `Borders::RIGHT` and
        // `Padding { left: 1, top: 1 }`. Kept in step with `render` below.
        let inner = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.checked_sub(2)?,
            height: area.height.checked_sub(1)?,
        };

        // One month-header row and one weekday-header row, both shown.
        let grid_y = inner.y + 2;
        if y < grid_y || y >= inner.y + inner.height {
            return None;
        }
        if x < inner.x || x >= inner.x + inner.width {
            return None;
        }

        // " Su Mo Tu ..." — a one-column gutter, then two columns per day.
        // Each day is a fixed two-character run (`Monthly` formats it with
        // `{:2?}`): `column % 3 == 0` is the gutter and `== 2` is the run's
        // second character. Only `== 1`, the run's first column, is a hit —
        // `date_at_agrees_with_what_monthly_actually_drew` reads two
        // characters forward from whatever position this returns, and
        // that read only lines up with the day's digits from the run's
        // start; from the second column it would read half a two-digit
        // day plus the gutter after it.
        let column = x - inner.x;
        if column % 3 != 1 {
            return None;
        }
        let weekday_index = column / 3;
        if weekday_index >= 7 {
            return None;
        }
        let week_index = y - grid_y;

        // `Monthly` starts the grid at the Sunday on or before the 1st, and
        // stops drawing once a row's Sunday reaches the following month —
        // a five-week month draws no sixth row, even though this widget's
        // `area` usually has the spare rows to fit one.
        let first_of_month = self.active_date.replace_day(1).ok()?;
        let offset = i64::from(first_of_month.weekday().number_days_from_sunday());
        let grid_start = first_of_month.checked_sub(Duration::days(offset))?;
        let row_start = grid_start.checked_add(Duration::days(i64::from(week_index) * 7))?;
        if row_start.month() == first_of_month.month().next() {
            return None;
        }

        // `show_surrounding` draws the neighbouring months' days greyed
        // out. They are real cells and clicking one navigates there, which
        // is what makes paging months by click work.
        row_start.checked_add(Duration::days(i64::from(weekday_index)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use time::macros::date;

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 24,
        height: 12,
    };

    fn render_calendar(active: Date) -> Buffer {
        let theme = Theme::none();
        let populated: Vec<Date> = Vec::new();
        let mut buf = Buffer::empty(AREA);
        // `Calendar` implements `Widget` for `&mut Calendar<'_>`, not for
        // `Calendar<'_>` itself, so `render` needs a named `mut` binding
        // rather than a temporary to autoref through.
        let mut calendar = Calendar::new(active, &populated, &theme);
        calendar.render(AREA, &mut buf);
        buf
    }

    /// The two-character day number drawn at (`x`, `y`), or `None` if that
    /// cell is not the start of one.
    fn day_number_at(buf: &Buffer, x: u16, y: u16) -> Option<u32> {
        let text: String = (x..x + 2).map(|cx| buf[(cx, y)].symbol()).collect();
        text.trim().parse().ok()
    }

    /// The load-bearing test for this whole feature.
    ///
    /// `Monthly` exposes no hit-test, so `date_at` replicates its geometry.
    /// This walks every cell of a real rendered calendar and asserts that
    /// wherever `date_at` claims a date, the digits actually on screen are
    /// that date's day number. If ratatui ever changes `Monthly`'s layout
    /// this fails loudly, instead of the app silently jumping to a day the
    /// user did not click.
    #[test]
    fn date_at_agrees_with_what_monthly_actually_drew() {
        let active = date!(2025 - 06 - 11);
        let buf = render_calendar(active);
        let theme = Theme::none();
        let populated: Vec<Date> = Vec::new();
        let calendar = Calendar::new(active, &populated, &theme);

        let mut matched = 0;
        for y in AREA.y..AREA.y + AREA.height {
            for x in AREA.x..AREA.x + AREA.width {
                let Some(hit) = calendar.date_at(AREA, x, y) else {
                    continue;
                };
                let drawn = day_number_at(&buf, x, y).unwrap_or_else(|| {
                    panic!("date_at claimed {hit} at ({x},{y}) but no day number is drawn there")
                });
                assert_eq!(
                    u32::from(hit.day()),
                    drawn,
                    "date_at said {hit} at ({x},{y}) but the screen shows {drawn}"
                );
                matched += 1;
            }
        }
        assert!(
            matched >= 28,
            "expected to hit at least a month of days, only matched {matched}"
        );
    }

    /// `Monthly` always starts its weeks on Sunday, whatever the app's
    /// `week_start_day` is set to. A hit-test that used the configured
    /// start would be wrong on every non-Sunday configuration — the single
    /// most likely bug in this feature.
    #[test]
    fn the_first_column_is_sunday() {
        let active = date!(2025 - 06 - 11);
        let theme = Theme::none();
        let populated: Vec<Date> = Vec::new();
        let calendar = Calendar::new(active, &populated, &theme);

        let first_hit = (AREA.y..AREA.y + AREA.height)
            .flat_map(|y| (AREA.x..AREA.x + AREA.width).map(move |x| (x, y)))
            .find_map(|(x, y)| calendar.date_at(AREA, x, y))
            .expect("some cell resolves to a date");

        assert_eq!(
            first_hit.weekday(),
            time::Weekday::Sunday,
            "the topmost-leftmost day cell must be a Sunday"
        );
    }

    #[test]
    fn clicks_outside_the_day_grid_resolve_to_nothing() {
        let active = date!(2025 - 06 - 11);
        let theme = Theme::none();
        let populated: Vec<Date> = Vec::new();
        let calendar = Calendar::new(active, &populated, &theme);

        // The month-header row.
        assert_eq!(calendar.date_at(AREA, 5, AREA.y), None);
        // Far outside the area.
        assert_eq!(calendar.date_at(AREA, 200, 200), None);
    }
}
