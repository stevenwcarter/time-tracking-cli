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

use super::event::AppEvent;
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
    /// Every mode.
    pub const ALL: Self = Self(15);

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
        keys: &[(KeyCode::Char('r'), NONE)],
        event: AppEvent::ReloadFromDisk,
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
/// Crossterm reports an uppercase character with `SHIFT` already set, so `G`
/// would otherwise never match the `(Char('G'), NONE)` row the table spells
/// it with.
fn normalize(key: KeyEvent) -> Key {
    let mut modifiers = key.modifiers;
    if matches!(key.code, KeyCode::Char(c) if c.is_uppercase()) {
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

    #[test]
    fn no_duplicate_key_within_a_mode() {
        for mode in [Mode::Day, Mode::Week, Mode::ZoomedWeek, Mode::RawFile] {
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
            "list nav is Day-only"
        );
    }

    /// Crossterm delivers a capital letter with `SHIFT` already set, so a
    /// table spelling `G` as `(Char('G'), NONE)` has to tolerate it.
    #[test]
    fn shifted_capitals_match_their_unmodified_row() {
        let g = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        let binding = lookup(g, Mode::Day).expect("G is bound in the day view");
        assert_eq!(binding.event, AppEvent::LastProject);
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
        for mode in [Mode::Day, Mode::Week, Mode::ZoomedWeek, Mode::RawFile] {
            let quit = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
            assert_eq!(
                lookup(quit, mode).map(|b| &b.event),
                Some(&AppEvent::Quit),
                "q must quit in {mode:?}"
            );
        }
    }

    /// The popup shows the day view's list keys and no other mode's.
    #[test]
    fn help_rows_are_narrowed_to_the_mode() {
        let day: Vec<_> = help_rows(Mode::Day).into_iter().map(|(_, d)| d).collect();
        let zoom: Vec<_> = help_rows(Mode::ZoomedWeek)
            .into_iter()
            .map(|(_, d)| d)
            .collect();

        assert!(day.contains(&"select the next project"));
        assert!(!zoom.contains(&"select the next project"));
        for rows in [&day, &zoom] {
            assert!(rows.contains(&"go to today"));
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
