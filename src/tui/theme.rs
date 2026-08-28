use ratatui::prelude::*;
use ratatui::style::palette::tailwind::{BLUE, SLATE};

/// Every style the TUI draws with.
///
/// Widgets never reach for a hard-coded colour; they are handed a `&Theme` so
/// the palette can be swapped wholesale (for example [`Theme::none()`] when the
/// terminal has no colour support).
#[derive(Clone, Debug)]
pub struct Theme {
    /// A calendar day or bar-chart day that has tracked hours.
    pub populated_date: Style,
    /// The day the user is currently looking at.
    pub active_date: Style,
    /// A day with no tracked hours, and calendar days outside the month.
    pub inactive_date: Style,
    /// Background of an even-numbered project row.
    pub row_bg: Style,
    /// Background of an odd-numbered project row.
    pub alt_row_bg: Style,
    /// The header bar above the project list.
    pub list_header: Style,
    /// The selected project row.
    pub selection: Style,
    /// Recoverable problems: parser warnings, a date with no data.
    pub warning: Style,
    /// Hard failures.
    pub error: Style,
    /// The daily-target marker on the weekly bar chart.
    pub goal_marker: Style,
    /// The status line.
    pub status: Style,
}

impl Theme {
    /// Reproduces the palette that was hard-coded across the widgets before the
    /// theme was extracted, so the default look is unchanged.
    pub fn dark() -> Self {
        Self {
            populated_date: Style::new().fg(BLUE.c300).add_modifier(Modifier::BOLD),
            active_date: Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
            inactive_date: Style::new().fg(SLATE.c400).add_modifier(Modifier::ITALIC),
            row_bg: Style::new().bg(SLATE.c950),
            alt_row_bg: Style::new().bg(SLATE.c900),
            list_header: Style::new().fg(SLATE.c100).bg(BLUE.c800),
            selection: Style::new().bg(BLUE.c950).add_modifier(Modifier::BOLD),
            warning: Style::new().fg(Color::Yellow),
            error: Style::new().fg(Color::Red),
            goal_marker: Style::new().fg(SLATE.c400),
            status: Style::new().fg(SLATE.c100),
        }
    }

    /// No foreground or background at all — modifiers only, so the terminal's
    /// own palette shows through.
    pub fn none() -> Self {
        let bold = Style::new().add_modifier(Modifier::BOLD);
        let italic = Style::new().add_modifier(Modifier::ITALIC);
        Self {
            populated_date: bold,
            active_date: bold,
            inactive_date: italic,
            row_bg: Style::new(),
            alt_row_bg: Style::new(),
            list_header: bold,
            selection: bold,
            warning: bold,
            error: bold,
            goal_marker: Style::new(),
            status: Style::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_reproduces_the_pre_theme_palette() {
        let theme = Theme::dark();
        assert_eq!(
            theme.populated_date,
            Style::new().fg(BLUE.c300).add_modifier(Modifier::BOLD)
        );
        assert_eq!(
            theme.active_date,
            Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)
        );
        assert_eq!(
            theme.inactive_date,
            Style::new().fg(SLATE.c400).add_modifier(Modifier::ITALIC)
        );
        assert_eq!(theme.row_bg, Style::new().bg(SLATE.c950));
        assert_eq!(theme.alt_row_bg, Style::new().bg(SLATE.c900));
        assert_eq!(theme.list_header, Style::new().fg(SLATE.c100).bg(BLUE.c800));
        assert_eq!(
            theme.selection,
            Style::new().bg(BLUE.c950).add_modifier(Modifier::BOLD)
        );
        assert_eq!(theme.warning, Style::new().fg(Color::Yellow));
        assert_eq!(theme.error, Style::new().fg(Color::Red));
        assert_eq!(theme.goal_marker, Style::new().fg(SLATE.c400));
        assert_eq!(theme.status, Style::new().fg(SLATE.c100));
    }

    #[test]
    fn none_emits_no_colour_only_modifiers() {
        let theme = Theme::none();
        let styles = [
            theme.populated_date,
            theme.active_date,
            theme.inactive_date,
            theme.row_bg,
            theme.alt_row_bg,
            theme.list_header,
            theme.selection,
            theme.warning,
            theme.error,
            theme.goal_marker,
            theme.status,
        ];
        for style in styles {
            assert_eq!(style.fg, None, "{style:?} should not set a foreground");
            assert_eq!(style.bg, None, "{style:?} should not set a background");
        }
        assert!(theme.populated_date.add_modifier.contains(Modifier::BOLD));
        assert!(theme.inactive_date.add_modifier.contains(Modifier::ITALIC));
    }
}
