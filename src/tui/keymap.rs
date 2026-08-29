//! The one table every key binding is read from.
//!
//! The keymap used to live in four places — the global `match` in
//! [`App`](super::app::App), a second `match` inside the project list, the
//! help popup's hand-written text, and a table in the README — and they had
//! already drifted apart by four bindings. [`BINDINGS`] is now the only one
//! written by hand: [`lookup`] drives the key layers, the popup renders
//! [`help_rows`], and the README carries the output of [`readme_table`],
//! which a test compares against the file on disk.

use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::event::{AppEvent, Reload};
use super::mode::Mode;

/// A key as [`BINDINGS`] spells it: a code plus the modifiers held with it.
pub type Key = (KeyCode, KeyModifiers);

/// No modifier held. Spelled short so the table stays one row per line.
const NONE: KeyModifiers = KeyModifiers::NONE;

/// Heading of the README table's key column.
const KEYS_HEADING: &str = "Keys";
/// Heading of the README table's description column.
const ACTION_HEADING: &str = "Action";

/// The set of [`Mode`]s a binding is live in.
///
/// Hand-rolled rather than pulled in from `bitflags`: four bits do not earn a
/// dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeMask(u8);

impl ModeMask {
    /// The day view only.
    pub const DAY: Self = Self(1);
    /// The weekly roll-up only.
    pub const WEEK: Self = Self(2);
    /// The full-screen weekly bar chart only.
    pub const ZOOM: Self = Self(4);
    /// The raw-file view only.
    pub const RAW: Self = Self(8);
    /// Every mode. Composed from the bits rather than spelled as a literal
    /// so a mode added later cannot silently fall out of it.
    pub const ALL: Self = Self::DAY.or(Self::WEEK).or(Self::ZOOM).or(Self::RAW);

    /// The union of two masks, for a binding live in some modes but not all.
    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Is a binding carrying this mask live in `mode`?
    pub fn contains(self, mode: Mode) -> bool {
        self.0 & Self::bit(mode).0 != 0
    }

    const fn bit(mode: Mode) -> Self {
        match mode {
            Mode::Day => Self::DAY,
            Mode::Week => Self::WEEK,
            Mode::ZoomedWeek => Self::ZOOM,
            Mode::RawFile => Self::RAW,
        }
    }
}

/// Where the help popup and the README list a binding.
///
/// Rows are shown group by group, in [`Group::ORDER`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// Moving around the day's project list.
    Project,
    /// Moving between dates.
    Date,
    /// Changing what fills the screen.
    View,
    /// Everything else: reload, edit, help, quit.
    General,
}

impl Group {
    /// Every group, in the order the help popup and the README list them.
    const ORDER: [Self; 4] = [Self::Project, Self::Date, Self::View, Self::General];
}

/// One row of the keymap: the keys, what they do, and where they do it.
#[derive(Debug)]
pub struct Binding {
    /// The keys that trigger the binding. Any one of them is enough.
    pub keys: &'static [Key],
    /// What pressing one of `keys` means.
    pub event: AppEvent,
    /// The modes the binding is live in.
    pub modes: ModeMask,
    /// Where the help popup and the README list it.
    pub group: Group,
    /// One line of prose, shown by both the popup and the README.
    pub description: &'static str,
}

impl Binding {
    /// Does `key` trigger this binding?
    fn matches(&self, key: KeyEvent) -> bool {
        let pressed = normalize(key);
        self.keys.contains(&pressed)
    }

    /// The keys as a human reads them, e.g. `"↓ / j"`.
    pub fn rendered_keys(&self) -> String {
        self.keys
            .iter()
            .map(|&(code, modifiers)| render_key(code, modifiers))
            .collect::<Vec<_>>()
            .join(" / ")
    }
}

/// Every key the TUI binds, and what it does.
///
/// A key may appear in more than one row provided the rows' [`ModeMask`]s are
/// disjoint, so the same physical key can mean different things in different
/// views; `no_duplicate_key_within_a_mode` is what keeps that honest.
pub const BINDINGS: &[Binding] = &[
    Binding {
        keys: &[(KeyCode::Down, NONE), (KeyCode::Char('j'), NONE)],
        event: AppEvent::NextProject,
        modes: ModeMask::DAY,
        group: Group::Project,
        description: "select the next project",
    },
    Binding {
        keys: &[(KeyCode::Up, NONE), (KeyCode::Char('k'), NONE)],
        event: AppEvent::PreviousProject,
        modes: ModeMask::DAY,
        group: Group::Project,
        description: "select the previous project",
    },
    Binding {
        keys: &[(KeyCode::Char('g'), NONE)],
        event: AppEvent::FirstProject,
        modes: ModeMask::DAY,
        group: Group::Project,
        description: "jump to the first project",
    },
    Binding {
        keys: &[(KeyCode::Char('G'), NONE)],
        event: AppEvent::LastProject,
        modes: ModeMask::DAY,
        group: Group::Project,
        description: "jump to the last project",
    },
    Binding {
        keys: &[(KeyCode::Enter, NONE)],
        event: AppEvent::CopyNotes,
        modes: ModeMask::DAY,
        group: Group::Project,
        description: "copy the selected project's notes to the clipboard",
    },
    Binding {
        keys: &[(KeyCode::Down, NONE), (KeyCode::Char('j'), NONE)],
        event: AppEvent::NextWeekProject,
        // Disjoint from the Day-only and RawFile-only rows carrying the same
        // keys: same key, different pane, different event.
        modes: ModeMask::WEEK,
        group: Group::Project,
        description: "select the next project in the weekly rollup",
    },
    Binding {
        keys: &[(KeyCode::Up, NONE), (KeyCode::Char('k'), NONE)],
        event: AppEvent::PreviousWeekProject,
        modes: ModeMask::WEEK,
        group: Group::Project,
        description: "select the previous project in the weekly rollup",
    },
    Binding {
        keys: &[(KeyCode::Char('g'), NONE)],
        event: AppEvent::FirstWeekProject,
        modes: ModeMask::WEEK,
        group: Group::Project,
        description: "jump to the first project in the weekly rollup",
    },
    Binding {
        keys: &[(KeyCode::Char('G'), NONE)],
        event: AppEvent::LastWeekProject,
        modes: ModeMask::WEEK,
        group: Group::Project,
        description: "jump to the last project in the weekly rollup",
    },
    Binding {
        keys: &[(KeyCode::Enter, NONE)],
        event: AppEvent::CopyWeekProject,
        modes: ModeMask::WEEK,
        group: Group::Project,
        description: "copy the selected project's week (with hours) to the clipboard",
    },
    Binding {
        keys: &[(KeyCode::Left, NONE), (KeyCode::Char('h'), NONE)],
        event: AppEvent::PreviousDate,
        modes: ModeMask::ALL,
        group: Group::Date,
        description: "go to the previous day",
    },
    Binding {
        keys: &[(KeyCode::Right, NONE), (KeyCode::Char('l'), NONE)],
        event: AppEvent::NextDate,
        modes: ModeMask::ALL,
        group: Group::Date,
        description: "go to the next day",
    },
    Binding {
        keys: &[(KeyCode::Char('H'), NONE)],
        event: AppEvent::PreviousWeek,
        modes: ModeMask::ALL,
        group: Group::Date,
        description: "go back a week",
    },
    Binding {
        keys: &[(KeyCode::Char('L'), NONE)],
        event: AppEvent::NextWeek,
        modes: ModeMask::ALL,
        group: Group::Date,
        description: "go forward a week",
    },
    Binding {
        keys: &[(KeyCode::Char('['), NONE), (KeyCode::PageUp, NONE)],
        event: AppEvent::PreviousMonth,
        modes: ModeMask::ALL,
        group: Group::Date,
        description: "go back a month",
    },
    Binding {
        keys: &[(KeyCode::Char(']'), NONE), (KeyCode::PageDown, NONE)],
        event: AppEvent::NextMonth,
        modes: ModeMask::ALL,
        group: Group::Date,
        description: "go forward a month",
    },
    Binding {
        keys: &[(KeyCode::Char('t'), NONE), (KeyCode::Char('T'), NONE)],
        event: AppEvent::Today,
        modes: ModeMask::ALL,
        group: Group::Date,
        description: "go to today",
    },
    Binding {
        keys: &[(KeyCode::Char('f'), NONE)],
        event: AppEvent::ToggleZoomBar,
        modes: ModeMask::ALL,
        group: Group::View,
        description: "toggle zooming into the weekly bar chart",
    },
    Binding {
        keys: &[(KeyCode::Char('w'), NONE)],
        event: AppEvent::ToggleWeekMode,
        // Live in Day (to enter) and Week (to leave again), the same pair
        // `v` uses for the raw-file view.
        modes: ModeMask::DAY.or(ModeMask::WEEK),
        group: Group::View,
        description: "toggle the week's per-project rollup",
    },
    Binding {
        keys: &[(KeyCode::Char('v'), NONE)],
        event: AppEvent::ToggleRawFile,
        // Live in Day (to enter) and RawFile (to leave again); disjoint from
        // Week/ZoomedWeek, which have no raw file of their own to show.
        modes: ModeMask::DAY.or(ModeMask::RAW),
        group: Group::View,
        description: "view the active date's file as it sits on disk",
    },
    Binding {
        keys: &[(KeyCode::Down, NONE), (KeyCode::Char('j'), NONE)],
        event: AppEvent::ScrollRawFileDown,
        // Disjoint from the Day-only list-navigation row above: same keys,
        // different mode, different meaning. `no_duplicate_key_within_a_mode`
        // is what keeps that honest.
        modes: ModeMask::RAW,
        group: Group::View,
        description: "scroll the raw file down",
    },
    Binding {
        keys: &[(KeyCode::Up, NONE), (KeyCode::Char('k'), NONE)],
        event: AppEvent::ScrollRawFileUp,
        modes: ModeMask::RAW,
        group: Group::View,
        description: "scroll the raw file up",
    },
    Binding {
        keys: &[(KeyCode::Char('r'), NONE)],
        event: AppEvent::ReloadFromDisk(Reload::Rescan),
        modes: ModeMask::ALL,
        group: Group::General,
        description: "reload the current date from disk",
    },
    Binding {
        keys: &[(KeyCode::Char('e'), NONE)],
        event: AppEvent::Edit,
        modes: ModeMask::ALL,
        group: Group::General,
        description: "edit the current date's notes in $EDITOR",
    },
    Binding {
        keys: &[(KeyCode::Char('y'), NONE)],
        event: AppEvent::YankDay,
        modes: ModeMask::ALL,
        group: Group::General,
        description: "copy the day's summary (with hours) to the clipboard",
    },
    Binding {
        keys: &[(KeyCode::Char('Y'), NONE)],
        event: AppEvent::YankWeek,
        modes: ModeMask::ALL,
        group: Group::General,
        description: "copy the week's summary (with hours) to the clipboard",
    },
    Binding {
        keys: &[(KeyCode::Char('?'), NONE)],
        event: AppEvent::ToggleHelp,
        modes: ModeMask::ALL,
        group: Group::General,
        description: "show the help popup",
    },
    Binding {
        keys: &[(KeyCode::Esc, NONE), (KeyCode::Char('q'), NONE)],
        event: AppEvent::Quit,
        modes: ModeMask::ALL,
        group: Group::General,
        description: "quit",
    },
];

/// The keys that dismiss an open overlay.
///
/// This one sits beside [`BINDINGS`] rather than in it because overlays are
/// not on the mode axis: `q` means "quit" in every mode and "close the popup"
/// only while one is open, which a [`ModeMask`] cannot express. It is still
/// documented alongside the rest, so the popup and the README list it too.
static CLOSE_OVERLAY: Binding = Binding {
    keys: &[
        (KeyCode::Char('?'), NONE),
        (KeyCode::Esc, NONE),
        (KeyCode::Char('q'), NONE),
    ],
    event: AppEvent::CloseOverlay,
    modes: ModeMask::ALL,
    group: Group::General,
    description: "close the help popup",
};

/// The binding `key` triggers in `mode`, if any.
pub fn lookup(key: KeyEvent, mode: Mode) -> Option<&'static Binding> {
    BINDINGS
        .iter()
        .find(|binding| binding.modes.contains(mode) && binding.matches(key))
}

/// Does `key` dismiss an open overlay?
pub fn closes_overlay(key: KeyEvent) -> bool {
    CLOSE_OVERLAY.matches(key)
}

/// The help popup's rows for `mode`, as `(keys, description)` pairs.
///
/// Rows arrive ordered by [`Group`] with an empty pair between groups, which
/// is how the popup draws them as separate blocks without needing to know
/// what the groups are.
pub fn help_rows(mode: Mode) -> Vec<(String, &'static str)> {
    let mut rows = Vec::new();
    let mut previous: Option<Group> = None;
    for binding in documented().filter(|binding| binding.modes.contains(mode)) {
        if previous.is_some_and(|group| group != binding.group) {
            rows.push((String::new(), ""));
        }
        previous = Some(binding.group);
        rows.push((binding.rendered_keys(), binding.description));
    }
    rows
}

/// The keybind table the README carries, header row and all.
///
/// `readme_keybind_table_matches_the_binding_table` asserts the README
/// contains this verbatim, so regenerating it is the only supported way to
/// change that section.
pub fn readme_table() -> String {
    let rows: Vec<(String, &'static str)> = documented()
        .map(|binding| (binding.rendered_keys(), binding.description))
        .collect();
    let keys_width = column_width(rows.iter().map(|(keys, _)| keys.as_str()), KEYS_HEADING);
    let action_width = column_width(rows.iter().map(|(_, action)| *action), ACTION_HEADING);

    let mut lines = Vec::with_capacity(rows.len() + 2);
    lines.push(format!(
        "| {KEYS_HEADING:<keys_width$} | {ACTION_HEADING:<action_width$} |"
    ));
    lines.push(format!("| {:-<keys_width$} | {:-<action_width$} |", "", ""));
    lines.extend(
        rows.iter()
            .map(|(keys, action)| format!("| {keys:<keys_width$} | {action:<action_width$} |")),
    );
    lines.join("\n")
}

/// Every documented binding, ordered by [`Group`].
fn documented() -> impl Iterator<Item = &'static Binding> {
    Group::ORDER.into_iter().flat_map(|group| {
        BINDINGS
            .iter()
            .chain([&CLOSE_OVERLAY])
            .filter(move |binding| binding.group == group)
    })
}

/// Width of a markdown column: the widest cell, its heading included.
fn column_width<'a>(cells: impl Iterator<Item = &'a str>, heading: &str) -> usize {
    cells
        .map(|cell| cell.chars().count())
        .max()
        .unwrap_or(0)
        .max(heading.chars().count())
}

/// A key event reduced to what [`BINDINGS`] compares against.
///
/// `SHIFT` is dropped from every [`KeyCode::Char`], because for a character
/// key the modifier carries nothing the character does not already say: `?`
/// versus `/`, `:` versus `;`, `G` versus `g`. Crossterm sets the bit
/// inconsistently across platforms — the Unix parser only sets it when the
/// character is uppercase, while the Windows parser derives it from
/// `dwControlKeyState` and so sets it for *any* shifted key — and a table
/// spelling its rows with [`NONE`] has to match on both. Crossterm itself
/// clears the bit this way once the shifted character is known (see its
/// Kitty "report alternate keys" branch).
///
/// Modifiers on non-character codes are left alone: `SHIFT` really is the
/// only thing distinguishing Shift+Up from Up.
fn normalize(key: KeyEvent) -> Key {
    let mut modifiers = key.modifiers;
    if matches!(key.code, KeyCode::Char(_)) {
        modifiers.remove(KeyModifiers::SHIFT);
    }
    (key.code, modifiers)
}

/// One key, spelled the way the help popup and the README show it.
fn render_key(code: KeyCode, modifiers: KeyModifiers) -> String {
    let name = key_name(code);
    if modifiers.contains(KeyModifiers::CONTROL) {
        format!("Ctrl-{name}")
    } else {
        name.into_owned()
    }
}

fn key_name(code: KeyCode) -> Cow<'static, str> {
    match code {
        KeyCode::Char(c) => Cow::Owned(c.to_string()),
        KeyCode::Enter => Cow::Borrowed("Enter"),
        KeyCode::Esc => Cow::Borrowed("Esc"),
        KeyCode::Up => Cow::Borrowed("↑"),
        KeyCode::Down => Cow::Borrowed("↓"),
        KeyCode::Left => Cow::Borrowed("←"),
        KeyCode::Right => Cow::Borrowed("→"),
        other => Cow::Owned(format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Every mode, so a test cannot quietly cover only some of them.
    const ALL_MODES: [Mode; 4] = [Mode::Day, Mode::Week, Mode::ZoomedWeek, Mode::RawFile];

    #[test]
    fn no_duplicate_key_within_a_mode() {
        for mode in ALL_MODES {
            let mut seen = HashSet::new();
            for b in BINDINGS.iter().filter(|b| b.modes.contains(mode)) {
                for k in b.keys {
                    assert!(seen.insert(*k), "{k:?} bound twice in {mode:?}");
                }
            }
        }
    }

    #[test]
    fn every_binding_is_documented() {
        for b in BINDINGS {
            assert!(!b.description.is_empty(), "{:?} has no description", b.keys);
        }
    }

    #[test]
    fn readme_keybind_table_matches_the_binding_table() {
        let readme = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
            .expect("README.md");
        assert!(
            readme.contains(&readme_table()),
            "README keybind table is stale — regenerate it from BINDINGS with \
             `cargo test -- --ignored print_readme_table --nocapture`"
        );
    }

    #[test]
    fn lookup_respects_mode() {
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert!(lookup(j, Mode::Day).is_some());
        assert!(
            lookup(j, Mode::ZoomedWeek).is_none(),
            "the zoomed chart has no list to navigate"
        );
    }

    /// Every row has to be reachable. A row spelled with a modifier that
    /// [`normalize`] strips before comparison could never fire, and nothing
    /// else would notice — six later tasks add rows to this table.
    #[test]
    fn every_table_key_survives_normalisation() {
        for binding in BINDINGS.iter().chain([&CLOSE_OVERLAY]) {
            for &(code, modifiers) in binding.keys {
                assert_eq!(
                    normalize(KeyEvent::new(code, modifiers)),
                    (code, modifiers),
                    "{code:?} + {modifiers:?} is unreachable: normalize rewrites it"
                );
            }
        }
    }

    /// `?` is Shift+`/`, and the Windows parser derives modifiers from
    /// `dwControlKeyState` rather than from the character, so it arrives with
    /// `SHIFT` set even though the character is not uppercase. Stripping
    /// `SHIFT` only for uppercase characters left the help popup unreachable
    /// on that platform.
    #[test]
    fn shifted_punctuation_matches_its_unmodified_row() {
        let question = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT);
        assert_eq!(
            lookup(question, Mode::Day).map(|b| &b.event),
            Some(&AppEvent::ToggleHelp),
            "? must open the help popup however the platform reports Shift"
        );
        assert!(closes_overlay(question), "and must close it again");
    }

    /// `SHIFT` still means something on a key whose code does not already
    /// encode it, so it is only dropped for characters.
    #[test]
    fn shift_is_kept_on_non_character_keys() {
        let shift_up = KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT);
        assert_eq!(normalize(shift_up), (KeyCode::Up, KeyModifiers::SHIFT));
        assert!(lookup(shift_up, Mode::Day).is_none());
    }

    /// A typo in `bit` — `Week => DAY`, say — would leak the day-only list
    /// bindings into the week view with every other test still green.
    #[test]
    fn each_mode_bit_selects_exactly_that_mode() {
        for mode in ALL_MODES {
            for other in ALL_MODES {
                assert_eq!(
                    ModeMask::bit(mode).contains(other),
                    mode == other,
                    "{mode:?}'s bit answered wrongly for {other:?}"
                );
            }
            assert!(ModeMask::ALL.contains(mode), "ALL must cover {mode:?}");
        }
    }

    /// The list keys belong to the two panes that have a list — the day
    /// view and the weekly rollup — and must mean the pane's *own* event in
    /// each, never leak into a pane with no list, and never resolve to the
    /// other pane's event. Getting the last part wrong is the silent case:
    /// `Enter` in the weekly rollup resolving to `CopyNotes` would yank the
    /// *day's* selected project into a weekly timesheet.
    #[test]
    fn the_list_keys_mean_their_own_panes_event_in_each_pane() {
        let rows: [(KeyCode, &AppEvent, &AppEvent); 3] = [
            (
                KeyCode::Char('G'),
                &AppEvent::LastProject,
                &AppEvent::LastWeekProject,
            ),
            (
                KeyCode::Char('g'),
                &AppEvent::FirstProject,
                &AppEvent::FirstWeekProject,
            ),
            (
                KeyCode::Enter,
                &AppEvent::CopyNotes,
                &AppEvent::CopyWeekProject,
            ),
        ];
        for (code, day_event, week_event) in rows {
            let key = KeyEvent::new(code, KeyModifiers::NONE);
            assert_eq!(lookup(key, Mode::Day).map(|b| &b.event), Some(day_event));
            assert_eq!(lookup(key, Mode::Week).map(|b| &b.event), Some(week_event));
            for mode in [Mode::ZoomedWeek, Mode::RawFile] {
                assert!(lookup(key, mode).is_none(), "{key:?} leaked into {mode:?}");
            }
        }
    }

    /// `j` is the busiest key in the table: three modes bind it, under
    /// disjoint masks, to three different events. Only the zoomed chart —
    /// which has nothing to move — leaves it unbound.
    #[test]
    fn j_means_something_different_in_each_mode_that_binds_it() {
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(
            lookup(j, Mode::Day).map(|b| &b.event),
            Some(&AppEvent::NextProject)
        );
        assert_eq!(
            lookup(j, Mode::Week).map(|b| &b.event),
            Some(&AppEvent::NextWeekProject)
        );
        assert_eq!(
            lookup(j, Mode::RawFile).map(|b| &b.event),
            Some(&AppEvent::ScrollRawFileDown)
        );
        assert!(
            lookup(j, Mode::ZoomedWeek).is_none(),
            "j leaked into the zoomed chart"
        );
    }

    /// `w` has to work both ways round, like `v`: a rollup you can enter but
    /// not leave is worse than no rollup at all.
    #[test]
    fn w_toggles_the_weekly_rollup_from_both_sides() {
        let w = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE);
        for mode in [Mode::Day, Mode::Week] {
            assert_eq!(
                lookup(w, mode).map(|b| &b.event),
                Some(&AppEvent::ToggleWeekMode),
                "w must toggle the rollup from {mode:?}"
            );
        }
    }

    /// Every mode's popup lists the shared bindings; only the day view's also
    /// lists the project list's.
    #[test]
    fn help_rows_distinguish_all_four_modes() {
        for mode in ALL_MODES {
            let rows: Vec<_> = help_rows(mode).into_iter().map(|(_, d)| d).collect();
            assert!(rows.contains(&"go to today"), "{mode:?} lost a shared row");
            assert_eq!(
                rows.contains(&"select the next project"),
                mode == Mode::Day,
                "{mode:?} disagrees about who owns the project list"
            );
        }
    }

    /// Crossterm delivers a capital letter with `SHIFT` already set, so a
    /// table spelling `G` as `(Char('G'), NONE)` has to tolerate it.
    #[test]
    fn shifted_capitals_match_their_unmodified_row() {
        let g = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        let binding = lookup(g, Mode::Day).expect("G is bound in the day view");
        assert_eq!(binding.event, AppEvent::LastProject);
    }

    /// `H`/`L` arrive with `SHIFT` set on most terminals — Shift is what
    /// makes them capital in the first place — so both modifier states must
    /// resolve to the same row.
    #[test]
    fn shifted_week_keys_match_their_unmodified_row() {
        for (c, event) in [('H', &AppEvent::PreviousWeek), ('L', &AppEvent::NextWeek)] {
            for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
                let key = KeyEvent::new(KeyCode::Char(c), modifiers);
                assert_eq!(
                    lookup(key, Mode::Day).map(|b| &b.event),
                    Some(event),
                    "{c} with {modifiers:?} must trigger {event:?}"
                );
            }
        }
    }

    /// A binding whose mask does not include the mode must not fire, and a
    /// modifier the table did not ask for must not be ignored.
    #[test]
    fn a_stray_modifier_does_not_trigger_a_binding() {
        let ctrl_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert!(lookup(ctrl_q, Mode::Day).is_none());
    }

    #[test]
    fn every_mode_keeps_the_bindings_that_mean_the_same_everywhere() {
        for mode in ALL_MODES {
            let quit = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
            assert_eq!(
                lookup(quit, mode).map(|b| &b.event),
                Some(&AppEvent::Quit),
                "q must quit in {mode:?}"
            );
        }
    }

    /// Every overlay dismiss key is documented, so the popup can never tell
    /// the user a way out that [`closes_overlay`] does not honour.
    #[test]
    fn the_documented_close_keys_are_the_ones_that_close() {
        for &(code, modifiers) in CLOSE_OVERLAY.keys {
            assert!(closes_overlay(KeyEvent::new(code, modifiers)));
        }
        assert!(!closes_overlay(KeyEvent::new(
            KeyCode::Char('j'),
            KeyModifiers::NONE
        )));
    }

    /// Regenerates the README section; see the module docs.
    #[test]
    #[ignore = "prints the README table rather than asserting anything"]
    fn print_readme_table() {
        println!("{}", readme_table());
    }
}
