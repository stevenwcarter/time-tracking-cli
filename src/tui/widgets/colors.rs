use ratatui::prelude::*;
use ratatui::style::palette::tailwind::*;

/// Shared color scheme for TUI widgets
pub struct WidgetColors;

impl WidgetColors {
    /// Color for populated dates in calendar and active days in charts
    pub const POPULATED_DATE: Color = BLUE.c300;

    /// Color for the currently active/selected date
    pub const ACTIVE_DATE: Color = Color::Red;

    /// Style for populated dates (calendar and charts)
    pub fn populated_style() -> Style {
        Style::new()
            .fg(Self::POPULATED_DATE)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for the active date
    pub fn active_style() -> Style {
        Style::new().fg(Self::ACTIVE_DATE).bold()
    }

    /// Style for inactive/empty days
    pub fn inactive_style() -> Style {
        Style::new().fg(SLATE.c400).add_modifier(Modifier::ITALIC)
    }
}
