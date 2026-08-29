use std::convert::Infallible;
use std::str::FromStr;

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

    /// A palette for light-background terminals: the same eleven roles as
    /// [`Theme::dark`], with the Tailwind lightness scale mirrored (`c950` ↔
    /// `c50`, `c900` ↔ `c100`, `c300` ↔ `c700`, …) so the slate row stripes
    /// and blue accents stay legible on a white page instead of sitting as
    /// near-black blocks. Roles that use a named ANSI colour rather than a
    /// Tailwind value (`active_date`, `warning`, `error`) are unchanged — the
    /// terminal itself adapts those to its own light or dark scheme.
    pub fn light() -> Self {
        Self {
            populated_date: Style::new().fg(BLUE.c700).add_modifier(Modifier::BOLD),
            active_date: Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
            inactive_date: Style::new().fg(SLATE.c600).add_modifier(Modifier::ITALIC),
            row_bg: Style::new().bg(SLATE.c50),
            alt_row_bg: Style::new().bg(SLATE.c100),
            list_header: Style::new().fg(SLATE.c900).bg(BLUE.c200),
            selection: Style::new().bg(BLUE.c100).add_modifier(Modifier::BOLD),
            warning: Style::new().fg(Color::Yellow),
            error: Style::new().fg(Color::Red),
            goal_marker: Style::new().fg(SLATE.c600),
            status: Style::new().fg(SLATE.c900),
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

    /// The 16-colour counterpart to [`Theme::dark`].
    ///
    /// Every field is stated directly rather than derived from `dark()` by a
    /// nearest-colour search: a distance metric over the raw RGB values maps
    /// both `row_bg` (`SLATE.c950`) and `alt_row_bg` (`SLATE.c900`) to the
    /// same `Black`, and the near-black `selection` background to the same
    /// neighbourhood too — which erases the zebra striping and the selection
    /// highlight in precisely the low-colour terminal this palette exists
    /// for. Stating the ANSI colours directly lets the roles that must stay
    /// visually distinct (`row_bg` vs `alt_row_bg` vs `selection`) actually
    /// do so.
    fn dark_ansi16() -> Self {
        Self {
            populated_date: Style::new()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
            active_date: Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
            inactive_date: Style::new()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            row_bg: Style::new().bg(Color::Black),
            alt_row_bg: Style::new().bg(Color::DarkGray),
            list_header: Style::new().fg(Color::White).bg(Color::Cyan),
            selection: Style::new().bg(Color::Blue).add_modifier(Modifier::BOLD),
            warning: Style::new().fg(Color::Yellow),
            error: Style::new().fg(Color::Red),
            goal_marker: Style::new().fg(Color::DarkGray),
            status: Style::new().fg(Color::White),
        }
    }

    /// The 16-colour counterpart to [`Theme::light`], for the same reason
    /// [`Theme::dark_ansi16`] exists: `row_bg` (`SLATE.c50`), `alt_row_bg`
    /// (`SLATE.c100`) and `selection` (`BLUE.c100`) are all pale enough that
    /// a nearest-colour search collapses every one of them to `White`,
    /// leaving the selected row indistinguishable from the list background
    /// except for `BOLD`.
    fn light_ansi16() -> Self {
        Self {
            populated_date: Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD),
            active_date: Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
            inactive_date: Style::new()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            row_bg: Style::new().bg(Color::White),
            alt_row_bg: Style::new().bg(Color::Gray),
            list_header: Style::new().fg(Color::Black).bg(Color::Cyan),
            selection: Style::new().bg(Color::Blue).add_modifier(Modifier::BOLD),
            warning: Style::new().fg(Color::Yellow),
            error: Style::new().fg(Color::Red),
            goal_marker: Style::new().fg(Color::DarkGray),
            status: Style::new().fg(Color::Black),
        }
    }

    /// Resolves the palette to draw with from the config file's `theme` key
    /// plus the terminal's environment, in precedence order:
    ///
    /// 1. `NO_COLOR` present and non-empty forces [`Theme::none`], regardless
    ///    of everything else.
    /// 2. Otherwise the configured preset (`"dark"`, `"light"`, or `"none"`)
    ///    applies; an absent or unrecognised value falls back to `"dark"`
    ///    without erroring, so a typo in the config file can't stop the TUI
    ///    from starting.
    /// 3. Unless that preset is `"none"`, a `COLORTERM` that isn't
    ///    `truecolor`/`24bit` switches to the preset's fixed 16-colour
    ///    variant ([`Theme::dark_ansi16`]/[`Theme::light_ansi16`]) so the
    ///    palette survives an 8/16-colour `TERM` over SSH without losing the
    ///    relationships between roles that must stay visually distinct.
    pub fn resolve(configured: Option<&str>, env: &ThemeEnv) -> Self {
        if env.no_color {
            return Self::none();
        }

        let preset = configured
            .map(|name| name.parse().unwrap_or(Preset::Dark))
            .unwrap_or(Preset::Dark);
        let truecolor = supports_truecolor(env.colorterm.as_deref());

        match (preset, truecolor) {
            (Preset::None, _) => Self::none(),
            (Preset::Dark, true) => Self::dark(),
            (Preset::Dark, false) => Self::dark_ansi16(),
            (Preset::Light, true) => Self::light(),
            (Preset::Light, false) => Self::light_ansi16(),
        }
    }
}

/// The built-in colour presets a config `theme` key can select.
#[derive(Clone, Copy, Debug)]
enum Preset {
    Dark,
    Light,
    None,
}

impl FromStr for Preset {
    // Parsing a preset name never fails: an unrecognised name is handled by
    // falling back to `Preset::Dark` (see `Theme::resolve`) rather than
    // erroring out of TUI start-up over a config typo.
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "light" => Self::Light,
            "none" => Self::None,
            _ => Self::Dark,
        })
    }
}

/// Whether `COLORTERM` advertises 24-bit colour support.
fn supports_truecolor(colorterm: Option<&str>) -> bool {
    matches!(colorterm, Some("truecolor") | Some("24bit"))
}

/// The environment signals that affect colour resolution, captured once so
/// [`Theme::resolve`] stays a pure function no test needs to mutate process
/// state to exercise.
#[derive(Clone, Debug, Default)]
pub struct ThemeEnv {
    no_color: bool,
    colorterm: Option<String>,
}

impl ThemeEnv {
    /// Reads `NO_COLOR` and `COLORTERM` from the process environment.
    pub fn from_env() -> Self {
        Self::parse(
            std::env::var("NO_COLOR").ok().as_deref(),
            std::env::var("COLORTERM").ok().as_deref(),
        )
    }

    /// The `NO_COLOR` convention is "present and non-empty": `NO_COLOR=` with
    /// no value does not count.
    fn parse(no_color: Option<&str>, colorterm: Option<&str>) -> Self {
        Self {
            no_color: no_color.is_some_and(|value| !value.is_empty()),
            colorterm: colorterm.map(str::to_owned),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canary for the extraction that introduced [`Theme`]: every literal
    /// below is transcribed from the pre-refactor sources at commit
    /// `2a97a58` — `src/tui/widgets/colors.rs` (the day styles),
    /// `src/tui/project_list.rs`'s four module `const`s (the list styles) and
    /// the inline `show_surrounding` style in `src/tui/widgets/calendar.rs`.
    /// If this test fails, the default look has drifted from what shipped.
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

    fn env(no_color: bool, colorterm: Option<&str>) -> ThemeEnv {
        ThemeEnv {
            no_color,
            colorterm: colorterm.map(str::to_string),
        }
    }

    #[test]
    fn no_color_beats_an_explicit_config_theme() {
        let t = Theme::resolve(Some("dark"), &env(true, Some("truecolor")));
        assert_eq!(t.populated_date.fg, None);
        assert_eq!(t.row_bg.bg, None);
    }

    #[test]
    fn the_default_is_dark() {
        let t = Theme::resolve(None, &env(false, Some("truecolor")));
        assert_eq!(t.populated_date.fg, Theme::dark().populated_date.fg);
    }

    #[test]
    fn light_and_dark_differ() {
        let d = Theme::resolve(Some("dark"), &env(false, Some("truecolor")));
        let l = Theme::resolve(Some("light"), &env(false, Some("truecolor")));
        assert_ne!(d.row_bg.bg, l.row_bg.bg);
    }

    #[test]
    fn a_non_truecolor_terminal_downgrades_to_ansi() {
        let t = Theme::resolve(Some("dark"), &env(false, None));
        // Every colour must be an indexed/named ANSI value, never Rgb.
        for style in [
            t.populated_date,
            t.active_date,
            t.row_bg,
            t.selection,
            t.list_header,
        ] {
            for c in [style.fg, style.bg].into_iter().flatten() {
                assert!(!matches!(c, Color::Rgb(..)), "{c:?} is not 16-colour safe");
            }
        }
    }

    #[test]
    fn an_unknown_preset_name_falls_back_to_dark_without_erroring() {
        let t = Theme::resolve(Some("chartreuse"), &env(false, Some("truecolor")));
        assert_eq!(t.populated_date.fg, Theme::dark().populated_date.fg);
    }

    #[test]
    fn an_empty_no_color_variable_does_not_count() {
        // The NO_COLOR convention is "present and non-empty".
        assert!(!ThemeEnv::parse(Some(""), None).no_color);
        assert!(ThemeEnv::parse(Some("1"), None).no_color);
    }

    /// Regression: a naive nearest-RGB-neighbour downgrade mapped both
    /// `row_bg` (`SLATE.c950`) and `alt_row_bg` (`SLATE.c900`) to `Black`,
    /// erasing the zebra stripe in exactly the low-colour terminal this
    /// palette exists to serve. Assert the *relationship*, not a specific
    /// colour — pinning exact ANSI values would pass even if a future change
    /// collapsed them back together under a different shared colour.
    #[test]
    fn a_non_truecolor_dark_theme_keeps_rows_and_selection_distinguishable() {
        let t = Theme::resolve(Some("dark"), &env(false, None));
        assert_ne!(t.row_bg.bg, t.alt_row_bg.bg);
        assert_ne!(t.selection.bg, t.row_bg.bg);
        assert_ne!(t.selection.bg, t.alt_row_bg.bg);
    }

    /// Same regression as above, for the light preset: `row_bg` (`SLATE.c50`),
    /// `alt_row_bg` (`SLATE.c100`) and `selection` (`BLUE.c100`) are all pale
    /// enough that a nearest-neighbour search collapsed every one of them to
    /// `White`, leaving the selected row with no colour cue at all.
    #[test]
    fn a_non_truecolor_light_theme_keeps_rows_and_selection_distinguishable() {
        let t = Theme::resolve(Some("light"), &env(false, None));
        assert_ne!(t.row_bg.bg, t.alt_row_bg.bg);
        assert_ne!(t.selection.bg, t.row_bg.bg);
        assert_ne!(t.selection.bg, t.alt_row_bg.bg);
    }
}
