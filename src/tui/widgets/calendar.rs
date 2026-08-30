use ratatui::prelude::*;
use ratatui::widgets::calendar::*;
use ratatui::widgets::*;
use time::{Date, Duration};

use crate::tui::theme::Theme;

/// The month calendar drawn beside the weekly chart in the day view's
/// header band.
///
/// Highlights the active date and shades every populated date around it, so
/// a month's worth of coverage reads at a glance.
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

        // " Su Mo Tu ..." — a one-column gutter, then a two-column run per
        // day. Only the gutter (`column.is_multiple_of(3)`) belongs to no day; **both**
        // columns of the run are the same day and must both be clickable.
        //
        // This deliberately does not accept only the run's first column.
        // `Monthly` right-aligns each day into its two columns
        // (`format!("{:2?}", day)`), so a single-digit day draws its only
        // visible character in the *second* column — accepting just the first
        // left every day from the 1st to the 9th unclickable on the digit
        // itself, while the blank beside it worked. That was a real shipped
        // bug, reported as "only some of the calendar accepts clicks"; see
        // `a_single_digit_day_is_clickable_on_its_digit`.
        let column = x - inner.x;
        if column.is_multiple_of(3) {
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

    /// The day number drawn in the two-character run that *contains* column
    /// `x`, or `None` when `x` is a gutter column or the run holds no digits.
    ///
    /// Reads from the run's start rather than from `x`, which is the whole
    /// point: `Monthly` right-aligns each day into two columns
    /// (`format!("{:2?}", day)`), so a single-digit day puts its only visible
    /// character in the run's *second* column. A helper that read two
    /// characters forward from an arbitrary `x` could only parse a run from
    /// its first column — and an earlier version of this file narrowed
    /// `date_at` itself to match that limitation, which is exactly the bug
    /// this pair of functions now exists to prevent.
    fn day_in_run_at(buf: &Buffer, area: Rect, x: u16, y: u16) -> Option<u32> {
        let inner_x = area.x + 1;
        let column = x.checked_sub(inner_x)?;
        if column.is_multiple_of(3) {
            // The gutter between two day cells belongs to neither.
            return None;
        }
        let run_start = inner_x + (column / 3) * 3 + 1;
        // A run clipped by the buffer's right edge is not a drawn day.
        let run_end = run_start.checked_add(2)?;
        if run_end > buf.area.right() {
            return None;
        }
        let text: String = (run_start..run_end)
            .map(|cx| buf[(cx, y)].symbol())
            .collect();
        text.trim().parse().ok()
    }

    /// The load-bearing test for this whole feature.
    ///
    /// `Monthly` exposes no hit-test, so `date_at` replicates its geometry.
    /// This walks every cell of the day grid and asserts agreement **in both
    /// directions**: every cell `date_at` claims shows that day's number, and
    /// every cell showing a day number is claimed by `date_at`.
    ///
    /// The second direction is the one that matters and the one an earlier
    /// version of this test lacked. It skipped rejected cells with a
    /// `continue`, so narrowing `date_at` to claim fewer cells passed
    /// trivially — and that is precisely what happened: `date_at` was
    /// restricted to the first column of each two-character day run, which
    /// left every single-digit day unclickable on its only visible character,
    /// because `Monthly` right-aligns days into their two columns. The suite
    /// stayed green while half the calendar ignored clicks.
    #[test]
    fn date_at_agrees_with_what_monthly_actually_drew() {
        let active = date!(2025 - 06 - 11);
        let buf = render_calendar(active);
        let theme = Theme::none();
        let populated: Vec<Date> = Vec::new();
        let calendar = Calendar::new(active, &populated, &theme);

        // One row of block padding, then the month header, then the weekday
        // header. Split here rather than parsing every row, because the month
        // header legitimately contains digits ("June 2025") that are not days.
        // An off-by-one in `date_at`'s own grid origin is still caught below:
        // it would attribute each screen row to the wrong week and the day
        // numbers would stop matching.
        let grid_y = AREA.y + 3;

        let mut matched = 0;
        for y in AREA.y..AREA.y + AREA.height {
            for x in AREA.x..AREA.x + AREA.width {
                let hit = calendar.date_at(AREA, x, y).map(|d| u32::from(d.day()));
                if y < grid_y {
                    assert_eq!(hit, None, "({x},{y}) is a header row, not a day");
                    continue;
                }
                let drawn = day_in_run_at(&buf, AREA, x, y);
                assert_eq!(
                    hit, drawn,
                    "disagreement at ({x},{y}): date_at says {hit:?}, the screen shows {drawn:?}"
                );
                if hit.is_some() {
                    matched += 1;
                }
            }
        }
        // Two columns per day across a month, so a correct implementation
        // matches far more than the day count. A number near 30 would mean
        // only one column per day is being claimed — the original bug.
        assert!(
            matched >= 56,
            "expected both columns of ~28+ days to be clickable, only matched {matched}"
        );
    }

    /// A single-digit day must be clickable on the digit itself.
    ///
    /// `Monthly` right-aligns into two columns, so day 5 renders as `" 5"` and
    /// its only visible character sits in the run's *second* column. The
    /// original `date_at` accepted only the first, so clicking the number did
    /// nothing while clicking the blank beside it worked — the user-visible
    /// symptom that "only some of the calendar accepts clicks".
    #[test]
    fn a_single_digit_day_is_clickable_on_its_digit() {
        let active = date!(2025 - 06 - 11);
        let buf = render_calendar(active);
        let theme = Theme::none();
        let populated: Vec<Date> = Vec::new();
        let calendar = Calendar::new(active, &populated, &theme);

        // Find the cell whose rendered symbol is the lone digit of day 5.
        let mut found = false;
        for y in AREA.y..AREA.y + AREA.height {
            for x in AREA.x..AREA.x + AREA.width {
                if buf[(x, y)].symbol() != "5" {
                    continue;
                }
                // Only the June 5th cell, not the "5" inside "15" or "25":
                // those have a digit immediately to their left.
                if x > AREA.x && buf[(x - 1, y)].symbol().trim().is_empty() {
                    let hit = calendar.date_at(AREA, x, y);
                    assert_eq!(
                        hit.map(|d| d.day()),
                        Some(5),
                        "clicking the visible digit of a single-digit day at ({x},{y}) must select it"
                    );
                    found = true;
                }
            }
        }
        assert!(found, "the fixture month must contain a single-digit day");
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
