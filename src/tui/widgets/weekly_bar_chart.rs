use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::HashMap;
use time::Date;

use crate::time_utils::WeekdayExt;
use crate::tui::theme::Theme;

/// Fallback daily-hours target for a chart built without a real
/// [`TuiContext`](crate::tui::context::TuiContext) (e.g. in a test), mirroring
/// `TuiContext`'s own default.
const DEFAULT_DAILY_TARGET_HOURS: f64 = 8.0;

pub struct WeeklyBarChart<'a> {
    active_date: Date,
    week_dates: &'a [Date; 7],
    theme: &'a Theme,
    week_data: Option<&'a HashMap<Date, u32>>, // Date -> total minutes
    /// Hours of tracked time that count as a full day; drives the chart's
    /// y-axis ceiling and goal line. Set via `set_daily_target_hours`.
    target_hours: f64,
}

/// One day's worth of bar data, computed once and then either read by a
/// test or turned into a `Bar` by `prepare_bars`.
struct BarValue {
    date: Date,
    /// Tracked minutes scaled to tenths of an hour, so `BarChart`'s integer
    /// axis can carry one decimal place of precision.
    tenths: u64,
    /// Raw tracked minutes. `tenths` floors anything under six minutes to
    /// zero, so style selection reads this instead — a day with a few
    /// tracked minutes should still count as populated.
    minutes: u32,
    /// The text shown at the foot of the bar.
    text: String,
}

impl<'a> WeeklyBarChart<'a> {
    pub fn new(active_date: Date, week_dates: &'a [Date; 7], theme: &'a Theme) -> Self {
        Self {
            active_date,
            week_dates,
            theme,
            week_data: None,
            target_hours: DEFAULT_DAILY_TARGET_HOURS,
        }
    }

    pub fn set_weekly_data(&mut self, data: &'a HashMap<Date, u32>) {
        self.week_data = Some(data);
    }

    /// Override the daily-hours target read from configuration. Without a
    /// call the chart falls back to `DEFAULT_DAILY_TARGET_HOURS`.
    pub fn set_daily_target_hours(&mut self, target_hours: f64) {
        self.target_hours = target_hours;
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

    /// Raw tracked minutes for one day, or zero if no data is loaded (or
    /// the day has none). Shared by `bar_values` and `week_max_minutes` so
    /// there is exactly one place that reads the week's data map.
    fn minutes_for(&self, date: Date) -> u32 {
        self.week_data
            .and_then(|data| data.get(&date))
            .copied()
            .unwrap_or(0)
    }

    /// The tallest tracked day this week, in raw minutes. The chart ceiling
    /// must never clip this bar, whatever the configured daily target is.
    fn week_max_minutes(&self) -> u32 {
        self.week_dates
            .iter()
            .map(|&date| self.minutes_for(date))
            .max()
            .unwrap_or(0)
    }

    /// Per-day bar data for the week, split out of `prepare_bars` because
    /// `Bar`'s `value` field is private in ratatui 0.29, so a test can't
    /// read it back off a built `Bar` — tests assert against this instead.
    fn bar_values(&self, bar_width: u16) -> Vec<BarValue> {
        self.week_dates
            .iter()
            .map(|&date| {
                let minutes = self.minutes_for(date);
                let hours = minutes as f64 / 60.0;
                let tenths = u64::from(minutes) * 10 / 60; // Scale by 10 for one decimal place; integer avoids lossy float→u64 cast

                // Format text to fit within bar width dynamically
                let text = if bar_width >= 5 {
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

                BarValue {
                    date,
                    tenths,
                    minutes,
                    text,
                }
            })
            .collect()
    }

    /// Style for one day's bar: today's date wins over having data, and
    /// `bar.minutes` (not the scaled `tenths`, which floors anything under
    /// six minutes to zero) decides whether a day counts as populated.
    fn style_for(&self, bar: &BarValue) -> Style {
        if bar.date == self.active_date {
            self.theme.active_date
        } else if bar.minutes > 0 {
            self.theme.populated_date
        } else {
            self.theme.inactive_date
        }
    }

    fn prepare_bars(&self, bar_width: u16) -> Vec<Bar<'_>> {
        if self.week_data.is_none() {
            return vec![]; // Empty data if not loaded
        }

        self.bar_values(bar_width)
            .into_iter()
            .map(|bar| {
                let style = self.style_for(&bar);
                // Day abbreviation and day of month below the bar
                let label = format!("{}\n{}", bar.date.weekday().short_name(), bar.date.day());

                Bar::default()
                    .value(bar.tenths)
                    .label(Line::from(label).style(style))
                    .text_value(bar.text) // Hours at bottom of bar
                    .style(style)
                    .value_style(style)
            })
            .collect()
    }
    /// Calculate dynamic bar width and gap based on the available width.
    /// The y-axis ceiling is independent of the area — see `ceiling_for`.
    fn calculate_bar_dimensions(&self, area: Rect) -> (u16, u16) {
        // Account for borders and padding
        let content_width = area.width.saturating_sub(4); // left border + padding

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

        (bar_width, bar_gap)
    }

    /// The y-axis ceiling for this chart, in tenths of an hour. Deliberately
    /// ignores `area`: the axis must reflect the week's data and the
    /// configured daily target, not how many rows the terminal happens to
    /// give the widget.
    pub fn ceiling_for(&self, _area: Rect) -> u64 {
        chart_ceiling(self.week_max_minutes(), self.target_hours)
    }

    /// Row inside `inner_area` the goal line sits on, or `None` if the area
    /// is too short to have a bar row at all.
    fn goal_marker_row(&self, inner_area: Rect, ceiling: u64) -> Option<u16> {
        let bars_height = inner_area.height.checked_sub(1)?; // bottom row holds the day labels
        if bars_height == 0 {
            return None;
        }
        // `.min(ceiling)` guards the subtraction below in case the invariant
        // that the target never exceeds the ceiling is ever broken upstream.
        let target = target_tenths(self.target_hours).min(ceiling);
        // `.max(1)`: integer division floors, so a target that is a tiny
        // fraction of a very tall ceiling (a long day dwarfing a modest
        // target, in a chart short enough to have only a handful of bar
        // rows) can floor `filled` to zero. Left unclamped that lands the
        // marker at `bars_height` rows down — one past the last bar row,
        // on the day-labels row — instead of just above it.
        let filled = ((target * u64::from(bars_height) / ceiling) as u16).max(1);
        Some(inner_area.y + bars_height - filled)
    }
}

/// The daily target expressed in tenths of an hour, floored at one hour so
/// a tiny configured target still produces a sane, visible ceiling.
fn target_tenths(target_hours: f64) -> u64 {
    (target_hours * 10.0).round().max(10.0) as u64
}

/// Chart ceiling in tenths of an hour: at least the daily target, and
/// always at least the tallest bar, rounded up to a whole hour.
fn chart_ceiling(week_max_minutes: u32, target_hours: f64) -> u64 {
    let max_tenths = u64::from(week_max_minutes) * 10 / 60;
    let needed = target_tenths(target_hours).max(max_tenths);
    needed.div_ceil(10) * 10 // round up to a whole hour
}

impl Widget for &mut WeeklyBarChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let title = format!("{} {}", self.active_date.month(), self.active_date.year());

        // Calculate dynamic dimensions and the data-driven y-axis ceiling
        let (bar_width, bar_gap) = self.calculate_bar_dimensions(area);
        let ceiling = self.ceiling_for(area);

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

        // Render total hours in the upper right corner, one row below the
        // block's own top edge and inset from its right edge — *not*
        // `Block::title_top`, and not flush against `inner_area`'s own edges
        // either. `App::render_day` draws an app-level frame around this
        // widget's `area` *after* this widget renders, overwriting row 0
        // and the rightmost column with its own border, so anything this
        // widget draws there (including a block title) never reaches the
        // screen; row 1 and a margin off the right edge do. The width,
        // though, correctly comes from `Line::width` (display columns)
        // rather than the previous version's `String::len` (UTF-8 bytes),
        // so a multi-byte character in the total no longer mis-positions it.
        const RIGHT_MARGIN: u16 = 2;
        let total_line = Line::from(total_text).style(self.theme.warning);
        let total_width = (total_line.width() as u16).min(inner_area.width);
        let total_area = Rect {
            // Floored at `inner_area.x`: on a narrow `Mode::ZoomedWeek`
            // terminal (no minimum-size floor there, unlike `Mode::Day`)
            // `total_width + RIGHT_MARGIN` can exceed the inner width, and
            // an unfloored `saturating_sub` lands at buffer column 0 —
            // pinning the text to the outer left edge — rather than at the
            // chart's own left edge.
            x: inner_area
                .right()
                .saturating_sub(total_width + RIGHT_MARGIN)
                .max(inner_area.x),
            y: area.y + 1,
            width: total_width,
            height: 1,
        };
        Paragraph::new(total_line).render(total_area, buf);

        // Render the chart in the inner area
        let chart = BarChart::default()
            .data(BarGroup::default().bars(&bars))
            .bar_width(bar_width)
            .bar_gap(bar_gap)
            .max(ceiling);

        chart.render(inner_area, buf);

        // Draw the goal line across the chart, on top of the bars, so a bar
        // that reaches or exceeds the target still visibly pierces through it.
        if let Some(row) = self.goal_marker_row(inner_area, ceiling) {
            let marker = "─".repeat(inner_area.width as usize);
            buf.set_string(inner_area.x, row, marker, self.theme.goal_marker);
        }
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
        data.insert(date!(2026 - 08 - 23), 3u32); // 3 tracked minutes: floors to a zero bar
        let mut chart = WeeklyBarChart::new(date!(2026 - 08 - 24), &week, &theme);
        chart.set_weekly_data(&data);

        let values = chart.bar_values(6);

        assert_eq!(values.len(), 7);
        assert_eq!(values[2].date, date!(2026 - 08 - 24));
        assert_eq!(values[2].tenths, 80, "8h in tenths of an hour");
        assert_eq!(
            values[2].minutes, 480,
            "bar_values must carry the raw minutes through, not just the scaled value"
        );
        assert_eq!(
            values[0].tenths, 0,
            "a day with no data is a zero bar, not absent"
        );
        assert_eq!(values[1].tenths, 0, "3 minutes floors to a zero-height bar");
        assert_eq!(
            values[1].minutes, 3,
            "but the raw minutes must survive the floor, for style_for to read"
        );
    }

    #[test]
    fn a_few_tracked_minutes_still_style_as_populated() {
        // Regression guard: `tenths` floors anything under six minutes to a
        // zero-height bar, but the day still has tracked time and must not
        // render with the same style as a day with none.
        let theme = Theme::none();
        let week = week();
        let chart = WeeklyBarChart::new(date!(2026 - 08 - 24), &week, &theme);
        let bar = BarValue {
            date: date!(2026 - 08 - 23),
            tenths: 0,
            minutes: 3,
            text: "0.0h".to_string(),
        };

        assert_eq!(chart.style_for(&bar), theme.populated_date);
        assert_ne!(chart.style_for(&bar), theme.inactive_date);
    }

    #[test]
    fn ceiling_is_the_target_when_the_week_is_light() {
        // 3h max in an 8h-target week → ceiling stays at 8h (80 tenths).
        assert_eq!(chart_ceiling(180, 8.0), 80);
    }

    #[test]
    fn ceiling_grows_past_the_target_for_a_long_day() {
        // 10h30m max exceeds the 8h target → ceiling rounds up to the next whole hour.
        assert_eq!(chart_ceiling(630, 8.0), 110);
    }

    #[test]
    fn a_day_exactly_on_an_hour_is_not_over_rounded() {
        // 11h exactly → ceiling is 11h. A bar at full height is correct, not clipped.
        assert_eq!(chart_ceiling(660, 8.0), 110);
    }

    #[test]
    fn ceiling_never_clips_the_tallest_bar() {
        for minutes in [0u32, 1, 59, 60, 480, 601, 1439] {
            let ceiling = chart_ceiling(minutes, 8.0);
            let bar = u64::from(minutes) * 10 / 60;
            assert!(
                bar <= ceiling,
                "{minutes}min bar {bar} exceeds ceiling {ceiling}"
            );
        }
    }

    #[test]
    fn ceiling_is_independent_of_terminal_height() {
        // The regression: the old formula returned a different value per height.
        assert_eq!(chart_ceiling(480, 8.0), chart_ceiling(480, 8.0));
        let tall = WeeklyBarChart::new(date!(2026 - 08 - 24), &week(), &Theme::none())
            .ceiling_for(Rect::new(0, 0, 80, 44));
        let short = WeeklyBarChart::new(date!(2026 - 08 - 24), &week(), &Theme::none())
            .ceiling_for(Rect::new(0, 0, 80, 12));
        assert_eq!(tall, short, "the y-axis must not depend on terminal height");
    }

    #[test]
    fn a_custom_target_moves_the_ceiling() {
        assert_eq!(chart_ceiling(180, 6.0), 60);
    }

    #[test]
    fn total_hours_is_right_aligned_one_row_below_the_top_edge() {
        // Regression guard for two bugs in the old hand-rolled `Rect`: the
        // column position must come from display width (`Line::width`), not
        // `String::len` (UTF-8 bytes) — those only coincide for pure ASCII —
        // and it must be measured against the block's padded inner area, not
        // the raw outer `area`. Row 1 (not row 0) is deliberate: see the
        // comment above the total-hours render in `Widget::render`.
        let theme = Theme::none();
        let week = week();
        let mut data = HashMap::new();
        data.insert(date!(2026 - 08 - 24), 480u32); // 8h -> "8.0h total"
        let mut chart = WeeklyBarChart::new(date!(2026 - 08 - 24), &week, &theme);
        chart.set_weekly_data(&data);

        let area = Rect::new(0, 0, 60, 15);
        let mut buf = Buffer::empty(area);
        (&mut chart).render(area, &mut buf);

        let row: String = (0..area.width)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            row.trim_end().ends_with("8.0h total"),
            "total hours must be right-aligned one row below the top edge: {row:?}"
        );
    }

    #[test]
    fn set_daily_target_hours_changes_the_ceiling() {
        let theme = Theme::none();
        let week = week();
        let mut chart = WeeklyBarChart::new(date!(2026 - 08 - 24), &week, &theme);

        assert_eq!(chart.ceiling_for(Rect::new(0, 0, 80, 30)), 80, "8h default");

        chart.set_daily_target_hours(6.0);
        assert_eq!(chart.ceiling_for(Rect::new(0, 0, 80, 30)), 60);
    }

    #[test]
    fn goal_marker_never_lands_on_the_day_labels_row() {
        // Regression guard: `filled` is an integer-division floor of the
        // target's proportional height, so a target that is a tiny fraction
        // of a very tall ceiling (a day dwarfing a modest target) can floor
        // to zero in a chart short enough to have only a handful of bar
        // rows — the zoomed view on a short terminal, roughly 5-6 rows.
        // Unclamped, that lands the marker one row *below* the bars area,
        // on the row the day-of-month labels occupy.
        let theme = Theme::none();
        let week = week();
        let mut data = HashMap::new();
        data.insert(date!(2026 - 08 - 24), 1439u32); // 23h59m dwarfs a 1h target
        let mut chart = WeeklyBarChart::new(date!(2026 - 08 - 24), &week, &theme);
        chart.set_weekly_data(&data);
        chart.set_daily_target_hours(1.0);

        let ceiling = chart.ceiling_for(Rect::new(0, 0, 80, 30));
        let inner_area = Rect::new(0, 0, 80, 6); // bars_height = 5
        let label_row = inner_area.y + inner_area.height - 1;

        let row = chart
            .goal_marker_row(inner_area, ceiling)
            .expect("a 6-row inner area has room for a bar row");
        assert!(
            row < label_row,
            "goal marker at row {row} must stay above the day-labels row {label_row}"
        );
    }

    #[test]
    fn total_hours_stays_right_aligned_in_a_narrow_zoomed_view() {
        // `Mode::ZoomedWeek` renders the chart at the raw terminal size with
        // no minimum-size floor — unlike `Mode::Day`, which now refuses to
        // lay out below 60x15 (see `ui::MIN_COLS`/`MIN_ROWS`), so this narrow
        // width is the one a real terminal can actually reach. Regression
        // guard: the previous `saturating_sub` had no floor at `inner_area.x`,
        // so once `total_width + RIGHT_MARGIN` exceeded the inner width it
        // saturated all the way to buffer column 0 — pinning the text to the
        // outer left edge — instead of stopping at the chart's own left edge.
        use crate::tui::app::App;
        use crate::tui::context::TuiContext;
        use crate::tui::mode::Mode;
        use crate::tui::testing::fixture_date;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_weekly_data(HashMap::from([(fixture_date(), 480u32)]));
        app.mode = Mode::ZoomedWeek;

        // Width 12: narrow enough that `total_width + RIGHT_MARGIN` (12)
        // exactly matches the inner width available, so an unfloored
        // `saturating_sub` would land at 0 — but still wide enough that the
        // full "8.0h total" fits without truncation, isolating the position
        // bug from the (separate, already-handled) narrow-bar-width case.
        let mut terminal = Terminal::new(TestBackend::new(12, 10)).expect("test backend");
        terminal
            .draw(|frame| frame.render_widget(&mut app, frame.area()))
            .expect("draw");
        let buf = terminal.backend().buffer().clone();

        let row: String = (0..12)
            .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            row.trim_end().ends_with("8.0h total"),
            "total hours text truncated or missing in a narrow zoomed view: {row:?}"
        );
        assert!(
            row.starts_with(' '),
            "total hours must stay clear of column 0 (the chart's own left edge), not pinned to the outer buffer edge: {row:?}"
        );
    }
}
