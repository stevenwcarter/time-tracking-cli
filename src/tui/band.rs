//! The fixed-height blocks a list pane stacks above and below its list, and
//! the one rule that stops them eating it.
//!
//! Both list panes — the day's [`ProjectListWidget`] and the week's
//! [`WeekListWidget`] — lay a *data-driven* band out against a `Fill(1)`
//! list: a header that grows a row per parser warning, a warnings block that
//! grows with the file. A `Constraint::Length` outranks `Constraint::Fill`,
//! so the band wins every tie, and at ordinary terminal sizes it wins so
//! completely that the list is left with nothing. It is worse than cramped
//! for the day pane: ratatui 0.29's `List` draws **zero** items when the
//! first one is taller than the viewport rather than clipping it, so one
//! row too few comes out as a blank pane, not a short one.
//!
//! [`fit_band`] is the rule — a band never takes the rows the list needs for
//! its first item. [`warning_lines`] is the other half: a block that grows
//! with the file is capped at [`MAX_WARNING_ROWS`] and carries its full
//! count in the title, so a badly parsed day is still unmistakably flagged
//! without the flag being the only thing on screen.
//!
//! [`ProjectListWidget`]: super::project_list::ProjectListWidget
//! [`WeekListWidget`]: super::week_list::WeekListWidget

use ratatui::prelude::*;

/// Rows a warnings block may claim before it starts crowding out the hours
/// the pane exists to show — a title plus four warnings.
///
/// A day can produce a warning per bad entry and a week one per bad entry
/// per day, so this is capped rather than left to grow: the block's job is
/// to stop a reader trusting a total that was parsed out of a malformed
/// file, which the count in the title does even when the individual lines
/// don't fit.
pub(super) const MAX_WARNING_ROWS: usize = 5;

/// The rows a band actually gets: what it asked for, or whatever is left of
/// `available` once the list keeps `floor` rows for itself — whichever is
/// smaller.
///
/// This is deliberately a clamp rather than a `Constraint::Max`/`Min` swap.
/// The cassowary strengths that decide which of two constraints yields are
/// not visible at the call site, and getting them backwards is exactly the
/// defect this exists to prevent; `min` says what happens in the one place
/// someone reads.
pub(super) fn fit_band(requested: u16, available: u16, floor: u16) -> u16 {
    requested.min(available.saturating_sub(floor))
}

/// `warnings` as at most [`MAX_WARNING_ROWS`] styled lines: a
/// `Warnings (N)` title carrying the full count, then as many warnings as
/// the budget allows, then a `… and K more` tail when some were left off.
///
/// `indent` prefixes the warnings themselves but not the title, so a
/// left-aligned block can hang its entries under the count; a centred one
/// passes `""`.
///
/// Returns no lines at all for a clean file — a pane that parsed cleanly
/// must not pay a row to say so.
pub(super) fn warning_lines(warnings: &[String], style: Style, indent: &str) -> Vec<Line<'static>> {
    if warnings.is_empty() {
        return Vec::new();
    }

    // The title always costs a row. Whatever is left goes to warnings,
    // except that overflowing costs one more row to say so — so the block
    // is never taller than the cap either way.
    let budget = MAX_WARNING_ROWS - 1;
    let shown = if warnings.len() <= budget {
        warnings.len()
    } else {
        budget - 1
    };

    let styled = |text: String| Line::styled(text, style);
    let mut lines = vec![styled(format!("Warnings ({})", warnings.len()))];
    lines.extend(
        warnings
            .iter()
            .take(shown)
            .map(|warning| styled(format!("{indent}{warning}"))),
    );
    if let Some(hidden) = warnings.len().checked_sub(shown).filter(|n| *n > 0) {
        lines.push(styled(format!("{indent}… and {hidden} more")));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_band_that_fits_keeps_every_row_it_asked_for() {
        assert_eq!(fit_band(4, 20, 4), 4);
    }

    #[test]
    fn a_band_yields_the_rows_the_list_needs() {
        // 12 rows, a list floor of 4: the band may have 8 however loudly it
        // asks for 30.
        assert_eq!(fit_band(30, 12, 4), 8);
    }

    /// An area smaller than the floor itself must saturate to zero rather
    /// than wrapping to `u16::MAX` and handing the band the whole pane.
    #[test]
    fn an_area_smaller_than_the_floor_leaves_the_band_nothing() {
        assert_eq!(fit_band(9, 3, 4), 0);
    }

    #[test]
    fn a_clean_file_costs_no_rows() {
        assert!(warning_lines(&[], Style::new(), "").is_empty());
    }

    #[test]
    fn a_flood_is_capped_and_counted() {
        let warnings: Vec<String> = (0..12).map(|i| format!("bad entry {i}")).collect();
        let lines = warning_lines(&warnings, Style::new(), "  ");
        assert_eq!(lines.len(), MAX_WARNING_ROWS);
        assert_eq!(lines[0].to_string(), "Warnings (12)");
        assert_eq!(lines[MAX_WARNING_ROWS - 1].to_string(), "  … and 9 more");
    }

    /// Exactly at the budget every warning is listed, rather than the last
    /// one being traded for a "… and 1 more" that costs the same row.
    #[test]
    fn a_file_that_fits_the_budget_lists_every_warning() {
        let warnings: Vec<String> = (0..MAX_WARNING_ROWS - 1)
            .map(|i| format!("bad entry {i}"))
            .collect();
        let lines = warning_lines(&warnings, Style::new(), "");
        assert_eq!(lines.len(), MAX_WARNING_ROWS);
        assert!(
            !format!("{lines:?}").contains("more"),
            "nothing was left off, so nothing should say so"
        );
    }
}
