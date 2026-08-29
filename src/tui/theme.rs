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
    ///    `truecolor`/`24bit` downgrades every role to its nearest
    ///    16-colour ANSI approximation, so the palette survives an 8/16
    ///    colour `TERM` over SSH.
    pub fn resolve(configured: Option<&str>, env: &ThemeEnv) -> Self {
        if env.no_color {
            return Self::none();
        }

        let preset = configured
            .map(|name| name.parse().unwrap_or(Preset::Dark))
            .unwrap_or(Preset::Dark);

        let theme = match preset {
            Preset::Dark => Self::dark(),
            Preset::Light => Self::light(),
            Preset::None => return Self::none(),
        };

        if supports_truecolor(env.colorterm.as_deref()) {
            theme
        } else {
            theme.into_ansi16()
        }
    }

    /// Downgrades every role's `fg`/`bg` to the nearest of the 16 named ANSI
    /// colours, for a terminal whose `COLORTERM` doesn't advertise truecolor.
    fn into_ansi16(self) -> Self {
        Self {
            populated_date: downgrade(self.populated_date),
            active_date: downgrade(self.active_date),
            inactive_date: downgrade(self.inactive_date),
            row_bg: downgrade(self.row_bg),
            alt_row_bg: downgrade(self.alt_row_bg),
            list_header: downgrade(self.list_header),
            selection: downgrade(self.selection),
            warning: downgrade(self.warning),
            error: downgrade(self.error),
            goal_marker: downgrade(self.goal_marker),
            status: downgrade(self.status),
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

/// Replaces a style's `fg`/`bg` with their nearest 16-colour ANSI
/// approximation, leaving modifiers untouched.
fn downgrade(style: Style) -> Style {
    Style {
        fg: style.fg.map(ansi16),
        bg: style.bg.map(ansi16),
        ..style
    }
}

/// The 16 named ANSI colours, paired with a representative RGB value used
/// only to find the closest match for a truecolor value.
const ANSI16: [(Color, (u8, u8, u8)); 16] = [
    (Color::Black, (0, 0, 0)),
    (Color::Red, (128, 0, 0)),
    (Color::Green, (0, 128, 0)),
    (Color::Yellow, (128, 128, 0)),
    (Color::Blue, (0, 0, 128)),
    (Color::Magenta, (128, 0, 128)),
    (Color::Cyan, (0, 128, 128)),
    (Color::Gray, (192, 192, 192)),
    (Color::DarkGray, (128, 128, 128)),
    (Color::LightRed, (255, 0, 0)),
    (Color::LightGreen, (0, 255, 0)),
    (Color::LightYellow, (255, 255, 0)),
    (Color::LightBlue, (0, 0, 255)),
    (Color::LightMagenta, (255, 0, 255)),
    (Color::LightCyan, (0, 255, 255)),
    (Color::White, (255, 255, 255)),
];

/// Maps a truecolor value to its nearest ANSI-16 neighbour by squared
/// Euclidean distance. Any colour that isn't `Rgb` (already a named ANSI
/// colour, `Indexed`, or `Reset`) passes through unchanged.
fn ansi16(color: Color) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));

    ANSI16
        .into_iter()
        .min_by_key(|&(_, (cr, cg, cb))| {
            let (cr, cg, cb) = (i32::from(cr), i32::from(cg), i32::from(cb));
            (r - cr).pow(2) + (g - cg).pow(2) + (b - cb).pow(2)
        })
        .map_or(color, |(named, _)| named)
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
}
