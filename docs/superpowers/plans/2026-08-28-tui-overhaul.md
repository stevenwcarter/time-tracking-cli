# TUI Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the Ratatui TUI's plumbing (injectable context, mode/overlay dispatch, single binding table, off-loop async loads, render-on-change) and layer 26 selected `WHATS-NEXT.md` items on top of it.

**Architecture:** Three phases on one branch. Phase 1 is a behaviour-preserving substrate: a `TuiContext` replaces global `Config::get()` reads, a `Mode` + `Option<Overlay>` pair replaces two view booleans, a `BINDINGS` table replaces three drifted copies of the keymap, a cloneable `AppEventSender` plus a latest-wins generation guard moves data loading off the event loop, and `DataService` gains a parsed-data cache and a `read_dir`-based date scan. Phases 2 and 3 are additive features and terminal-robustness work built on that substrate.

**Tech Stack:** Rust edition 2024, ratatui 0.29 (`widget-calendar`), crossterm 0.28, tokio 1.x, `time` 0.3, `interim` 0.2, `copypasta`, `unicode-width` (new), anyhow, tracing.

**Spec:** `docs/superpowers/specs/2026-08-28-tui-overhaul-design.md`

## Global Constraints

- Rust **edition 2024** for every crate. This repo has **no `rustfmt.toml`** — verified — so `cargo fmt --all` takes the edition from each `Cargo.toml`, and both crates are already 2024. Do not add a `rustfmt.toml`; if you ever do, it must be edition 2024 to match, or `cargo fmt --all` and a bare `rustfmt` will disagree and leave permanent stray diffs.
- `cargo clippy --all-targets --all-features` must be **warning-free**; `cargo fmt --all` must produce no diff.
- Conventional commits, enforced by Husky + commitlint: `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `test:`.
- **`ttcli` stdout must not change.** Task 1 pins it. The single intentional divergence is weekly project *tie* ordering, which is unspecified today.
- **No new write path** to the user's markdown files in this branch. `$EDITOR` stays the only mutation route.
- Exactly one new third-party dependency is authorised: `unicode-width`. No `insta`, no `notify`, no `libc`.
- `tempfile` may be added to the **`cli` package's `[dev-dependencies]`**. It is already a dev-dependency of the root package and so already in `Cargo.lock` — this adds no new vendored code.
- Two new config keys, both optional and additive: `theme` (`"dark"` | `"light"` | `"none"`, default `"dark"`) and `daily_target_hours` (default `8`).
- No new `unwrap()` or `expect()` anywhere under `src/tui/`.
- **Every test that constructs an `App` must be `#[tokio::test] async fn`, not `#[test]`.** `App::new` builds an `EventHandler`, which calls `tokio::spawn`; a plain `#[test]` panics with "there is no reactor running". Discovered in Task 2. Do not work around this by making the spawn conditional on test-only state — that would fork production behaviour.
- Every task ends green: `cargo test` passes and the working tree is committed.

---

## File Structure

**New files**

| File | Responsibility |
|---|---|
| `src/tui/context.rs` | `TuiContext` — the injected, owned snapshot of config for the TUI |
| `src/tui/theme.rs` | `Theme`, `Preset`, and env/config resolution (`NO_COLOR`, `COLORTERM`) |
| `src/tui/keymap.rs` | `Binding`, `ModeMask`, `BINDINGS`, lookup, and help/README row generation |
| `src/tui/mode.rs` | `Mode`, `Overlay`, `Handled`; per-mode render and key dispatch |
| `src/tui/widgets/date_prompt.rs` | The `:` date-entry overlay widget |
| `src/tui/widgets/raw_file.rs` | Scrollable raw day-file view (`Mode::RawFile`) |
| `src/tui/week_list.rs` | Weekly per-project rollup pane (`Mode::Week`) |
| `src/tui/testing.rs` | `#[cfg(test)]` helpers: fixture data, `render_to_string`, `TuiContext::for_test` |
| `tests/cli_output_characterization.rs` | Byte-for-byte `ttcli` / `ttcli --week` stdout pinning |

**Modified files**

| File | Change |
|---|---|
| `src/tui/app.rs` | Holds `TuiContext`, `Mode`, `Overlay`, `dirty`, `load_gen`, month memo, status line, clipboard |
| `src/tui/ui.rs` | Dispatches render to mode then overlay; breakpoints; too-small notice |
| `src/tui/event.rs` | `AppEventSender`; `DataLoaded`/`LoadFailed`/`CopyToClipboard`/new motions; `TICK_FPS` 30 → 4 |
| `src/tui/project_list.rs` | Width-keyed cached body, wrapping, `Handled` return, theme, dead time + warnings |
| `src/tui/widgets/weekly_bar_chart.rs` | Takes `week_dates` + `Theme`; target-based scale; `Block::title_top` |
| `src/tui/widgets/calendar.rs` | Takes `Theme` instead of the inline `SLATE.c400` italic |
| `src/tui/widgets/help_popup.rs` | Rows generated from `BINDINGS`; modal; clamped sizing |
| `src/tui/widgets/colors.rs` | Deleted — replaced by `theme.rs` |
| `src/tui/mod.rs` | Builds `TuiContext` in `tui()`; declares the new modules |
| `src/data_svc.rs` | Parsed-data cache, `read_dir` scan, `ensure_data_dir`, `get_weekly_summary` |
| `src/display/mod.rs` | `show_weekly_summary` reduced to a formatter call over `WeeklySummary` |
| `src/display/{default,plain,markdown}.rs` | `weekly_projects` signature takes `&[WeeklyProject]` |
| `src/config.rs` | `theme` + `daily_target_hours` fields and config comments |
| `cli/src/main.rs` | Webserver ctrl-c line only when the webserver spawned |
| `README.md` | Keybind table regenerated from `BINDINGS` |
| `Cargo.toml` | `unicode-width` |

---

# Phase 1 — Substrate

## Task 1: Pin CLI stdout before touching the weekly aggregation

The W18 extraction moves the weekly per-project fold out of `show_weekly_summary`. Its own note claims output stays identical "because the formatter calls stay in `display/mod.rs`" — an appeal to an invariant, not a test. This task writes the test that makes that claim checkable, **before** anything moves.

**Two facts that shape this task, both verified in the current code:**

1. The `ttcli` binary is defined by the **`cli` package**, so `CARGO_BIN_EXE_ttcli` is only available to integration tests under `cli/tests/`. The test cannot live in the root package.
2. `show_single_day` calls `create_day_file_if_not_exists` (`src/display/mod.rs:324`), so a single-day run **writes a template file into the data directory**. Pointing the binary at a checked-in fixture directory would mutate the repo and make the test non-hermetic. Every run therefore copies its fixture into a fresh temp directory first.

**Files:**
- Create: `cli/tests/cli_output_characterization.rs`
- Create: `cli/tests/fixtures/week_no_ties/*.md` (six of the seven days)
- Create: `cli/tests/fixtures/week_with_ties/*.md`
- Create: `cli/tests/golden/*.txt` (generated in Step 5)
- Modify: `cli/Cargo.toml` — add `tempfile = "3.8"` under `[dev-dependencies]`

**Interfaces:**
- Consumes: nothing.
- Produces: a green characterization suite Task 11 must keep green. No library API.

- [ ] **Step 1: Add the dev-dependency**

```toml
[dev-dependencies]
tempfile = "3.8"
```

- [ ] **Step 2: Create the tie-free fixture week**

Files under `cli/tests/fixtures/week_no_ties/`. Every project must have a **distinct** weekly total so the descending sort has no ties. Example — `2026-08-24.md`:

```markdown
# Monday 2026-08-24

8-10:30 client-bd
  - Reviewed the migration plan
10:30-12 internal
  - Sprint planning
1-3:15 admin
  - Expense report
```

Write `2026-08-22`, `2026-08-24`, `2026-08-25`, `2026-08-26`, `2026-08-27`, `2026-08-28`. Arrange distinct weekly totals: `client-bd` > `internal` > `admin` > `ops`. **Deliberately omit `2026-08-23.md`** so the `display_no_file_found` branch is covered, and give one day a file with a header but no time entries so `display_no_data_found` is covered.

- [ ] **Step 3: Create the tie fixture week**

`cli/tests/fixtures/week_with_ties/` — same shape, but arrange for `alpha` and `zulu` to land on **exactly** the same weekly total (4 hours each) so the tie branch is exercised.

- [ ] **Step 4: Write the characterization test**

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

/// Copy a fixture week into a fresh temp dir. Required because
/// `show_single_day` creates a template file for a missing date, which
/// would otherwise mutate the checked-in fixtures.
fn staged(fixture: &str) -> tempfile::TempDir {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture);
    let dir = tempfile::tempdir().expect("tempdir");
    for entry in std::fs::read_dir(&src).expect("fixture dir must exist") {
        let entry = entry.expect("dir entry");
        std::fs::copy(entry.path(), dir.path().join(entry.file_name())).expect("copy fixture");
    }
    dir
}

fn run_ttcli(args: &[&str], data_dir: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_ttcli"))
        .args(args)
        .arg("--data-directory")
        .arg(data_dir)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run ttcli");
    assert!(
        out.status.success(),
        "ttcli exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("stdout was not utf-8")
}

/// Golden-file comparison. `BLESS_GOLDEN=1` (re)writes the golden.
fn compare_golden(name: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.txt"));
    if std::env::var("BLESS_GOLDEN").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing golden {}; rerun with BLESS_GOLDEN=1", path.display()));
    assert_eq!(expected, actual, "stdout changed for {name}");
}

#[test]
fn weekly_summary_output_is_stable_across_formatters() {
    for f in ["default", "plain", "markdown"] {
        let dir = staged("week_no_ties");
        let got = run_ttcli(
            &["--week", "--date", "2026-08-24", "--formatter", f],
            dir.path(),
        );
        compare_golden(&format!("weekly_{f}"), &got);
    }
}

#[test]
fn single_day_output_is_stable_across_formatters() {
    for f in ["default", "plain", "markdown"] {
        let dir = staged("week_no_ties");
        let got = run_ttcli(
            &["--date", "2026-08-24", "--noedit", "--formatter", f],
            dir.path(),
        );
        compare_golden(&format!("day_{f}"), &got);
    }
}

#[test]
fn missing_day_still_renders_no_file_found() {
    let dir = staged("week_no_ties");
    let got = run_ttcli(&["--week", "--date", "2026-08-24", "--formatter", "plain"], dir.path());
    assert!(got.contains("2026-08-23"), "the omitted day must still appear in the week");
}
```

- [ ] **Step 5: Write the tie-stability test**

Ties are ordered by `HashMap` iteration today, so this asserts a property rather than exact bytes. It is **expected to be unstable until Task 11** adds the name tiebreak, so it ships ignored with a pointer to the task that un-ignores it:

```rust
#[test]
#[ignore = "unstable until Task 11 sorts weekly projects by (minutes desc, name asc)"]
fn weekly_tie_ordering_is_deterministic() {
    let dir = staged("week_with_ties");
    let first = run_ttcli(&["--week", "--date", "2026-08-24"], dir.path());
    for _ in 0..20 {
        let again = run_ttcli(&["--week", "--date", "2026-08-24"], dir.path());
        assert_eq!(first, again, "tie ordering varied between runs");
    }
}
```

- [ ] **Step 6: Bless the goldens**

Run: `BLESS_GOLDEN=1 cargo test -p cli --test cli_output_characterization`
Then: `cargo test -p cli --test cli_output_characterization`
Expected: all non-ignored tests PASS.

- [ ] **Step 7: Verify the goldens are real and the repo is unpolluted**

Run: `wc -l cli/tests/golden/*.txt && git status --porcelain cli/tests/fixtures/`
Expected: every golden non-empty; `weekly_default.txt` contains the weekly header and all four project names; **`git status` on the fixtures reports nothing** (a modified or new fixture file means `staged()` is not being used somewhere).

A zero-byte golden means `run_ttcli` silently produced nothing — fix that before proceeding rather than blessing an empty file.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add cli/ && git commit -m "test: pin ttcli stdout before the weekly aggregation extraction"
```

---

## Task 2: Inject a TuiContext instead of the Config singleton (W19)

**Files:**
- Create: `src/tui/context.rs`, `src/tui/theme.rs`, `src/tui/testing.rs`
- Modify: `src/tui/mod.rs`, `src/tui/app.rs:63-70`, `src/tui/widgets/calendar.rs`, `src/tui/widgets/weekly_bar_chart.rs`, `src/tui/project_list.rs:7-10`
- Delete: `src/tui/widgets/colors.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `TuiContext { week_start_day: Weekday, data_dir: PathBuf, daily_target_hours: f64, formatter: Formatter, theme: Theme }`
  - `TuiContext::from_config(&Config) -> Result<Self>`
  - `TuiContext::for_test() -> Self` (Saturday week start, temp data dir, 8.0 target, `Formatter::Plain`, `Theme::none()`)
  - `App::new(ctx: TuiContext) -> Self`
  - `Theme` with fields `populated_date, active_date, inactive_date, row_bg, alt_row_bg, list_header, selection, warning, error, goal_marker, status: Style`, plus `Theme::dark()` and `Theme::none()`
  - `testing::render_to_string(app: &mut App, w: u16, h: u16) -> String`
  - Test builders on `App`, all `#[cfg(test)]` and chainable:
    `with_active_date(Date)`, `with_data(TimeTrackingData)`, `with_raw_content(&str)`
  - `testing::fixture_day() -> TimeTrackingData` — three projects (`admin`, `client-bd`, `internal`),
    each with two notes, no warnings, no dead time. Every later task's render tests build on it,
    so its shape must not change once tasks start landing.
  - `testing::fixture_day_with_projects(n: usize) -> TimeTrackingData`
  - `ProjectListWidget::new(data: &TimeTrackingData, theme: &Theme) -> Self`
  - `App.loading: bool` — declared here, set by Task 6, rendered by Task 12

  (`testing::fixture_week_summary()` arrives in Task 11, once `WeeklySummary` exists.)

- [ ] **Step 1: Write the failing test**

`src/tui/testing.rs`:

```rust
#![cfg(test)]
use ratatui::{Terminal, backend::TestBackend};
use super::app::App;

/// Render an App into an off-screen buffer and flatten it to a string,
/// one line per terminal row, trailing spaces trimmed.
pub fn render_to_string(app: &mut App, w: u16, h: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("test backend");
    terminal
        .draw(|frame| frame.render_widget(app, frame.area()))
        .expect("draw");
    let buf = terminal.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
```

`src/tui/context.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn app_can_be_constructed_without_parsing_argv() {
        // Regression: App::new() used to call Config::get(), which runs
        // Args::parse() and so panicked or consumed the test harness's argv.
        let ctx = TuiContext::for_test();
        let app = crate::tui::app::App::new(ctx);
        assert!(app.running);
    }

    #[test]
    fn for_test_context_uses_saturday_and_no_color() {
        let ctx = TuiContext::for_test();
        assert_eq!(ctx.week_start_day, time::Weekday::Saturday);
        assert_eq!(ctx.theme.populated_date.fg, None);
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test --lib tui::context`
Expected: FAIL — `TuiContext` does not exist.

- [ ] **Step 3: Implement `Theme`**

`src/tui/theme.rs` — Task 22 adds preset resolution; this task ships `dark()` (reproducing today's colours exactly) and `none()`:

```rust
use ratatui::prelude::*;
use ratatui::style::palette::tailwind::*;

#[derive(Clone, Debug)]
pub struct Theme {
    pub populated_date: Style,
    pub active_date: Style,
    pub inactive_date: Style,
    pub row_bg: Style,
    pub alt_row_bg: Style,
    pub list_header: Style,
    pub selection: Style,
    pub warning: Style,
    pub error: Style,
    pub goal_marker: Style,
    pub status: Style,
}

impl Theme {
    /// Reproduces the pre-refactor hardcoded palette byte-for-byte.
    pub fn dark() -> Self {
        Self {
            populated_date: Style::new().fg(BLUE.c300).add_modifier(Modifier::BOLD),
            active_date: Style::new().fg(Color::Red).bold(),
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

    /// No fg/bg at all — modifiers only, so the terminal palette shows through.
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
```

- [ ] **Step 4: Implement `TuiContext`**

```rust
use crate::config::{Config, Formatter};
use crate::time_utils::parse_weekday;
use anyhow::{Context, Result};
use std::path::PathBuf;
use time::Weekday;
use super::theme::Theme;

#[derive(Clone, Debug)]
pub struct TuiContext {
    pub week_start_day: Weekday,
    pub data_dir: PathBuf,
    pub daily_target_hours: f64,
    pub formatter: Formatter,
    pub theme: Theme,
}

impl TuiContext {
    pub fn from_config(config: &Config) -> Result<Self> {
        Ok(Self {
            week_start_day: parse_weekday(config.get_week_start_day())
                .context("could not parse week start day")?,
            data_dir: crate::get_time_tracking_dir()?,
            daily_target_hours: 8.0, // Task 23 reads this from config
            formatter: config.get_configured_formatter().cloned().unwrap_or(Formatter::Default),
            theme: Theme::dark(), // Task 22 resolves this from config + env
        })
    }

    #[cfg(test)]
    pub fn for_test() -> Self {
        Self {
            week_start_day: Weekday::Saturday,
            data_dir: std::env::temp_dir().join("ttcli-test"),
            daily_target_hours: 8.0,
            formatter: Formatter::Plain,
            theme: Theme::none(),
        }
    }
}
```

- [ ] **Step 5: Thread it through `App` and the widgets**

- `App` gains `pub ctx: TuiContext`; `App::new(ctx)` sets `active_date` from `Config::get().date` **at the `tui()` call site**, not inside `App` — pass it in as part of construction so `App` never touches `Config`.
- `src/tui/mod.rs`: `tui()` builds the context.

```rust
pub async fn tui() -> Result<()> {
    let config = Config::get();
    let ctx = context::TuiContext::from_config(config)?;
    let terminal = ratatui::init();
    let result = app::App::new(ctx).with_active_date(config.date).run(terminal).await;
    ratatui::restore();
    result
}
```

- `Calendar::new(app)` becomes `Calendar::new(active_date, &populated_dates, &theme)`.
- `WeeklyBarChart::new(active_date)` becomes `WeeklyBarChart::new(active_date, &theme)` (Task 3 adds `week_dates`).
- `project_list.rs`: delete the four module-level `const`s at lines 7-10 and read from `theme`.
- Delete `src/tui/widgets/colors.rs` and its `pub use` in `widgets/mod.rs`.

- [ ] **Step 6: Run the tests**

Run: `cargo test --lib tui::`
Expected: PASS.

- [ ] **Step 7: Verify no `Config::get()` remains below `tui()`**

Run: `grep -rn 'Config::get()' src/tui/`
Expected: **no matches** except inside `src/tui/mod.rs`'s `tui()`. Any other hit is a miss — fix it now.

- [ ] **Step 8: Lint and commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "refactor(tui): inject TuiContext instead of reading the Config singleton"
```

---

## Task 3: Hoist week derivation out of the render path (W12)

`prepare_bars` reaches into the global `Config` and re-runs `parse_weekday` (a chain of case-insensitive string comparisons) plus `get_week_dates` (a heap-allocated `Vec<Date>`) on **every frame**, for a value that can only change when the active date changes. `load_data_for_active_date` already computes exactly these at `app.rs:186-188` and then discards them.

**Files:**
- Modify: `src/tui/app.rs:171-214`, `src/tui/widgets/weekly_bar_chart.rs:18-48`, `src/tui/ui.rs:14,36`

**Interfaces:**
- Consumes: `TuiContext` (Task 2).
- Produces:
  - `App.week_dates: [Date; 7]`
  - `WeeklyBarChart::new(active_date: Date, week_dates: &'a [Date; 7], theme: &'a Theme) -> Self`

- [ ] **Step 1: Write the failing test**

`src/tui/widgets/weekly_bar_chart.rs`:

`Bar`'s `value` field is private in ratatui 0.29, so the assertion cannot read it back off a built `Bar`. Split the computation out: a new private `bar_values(&self) -> Vec<(Date, u64, String)>` holds the logic, and `prepare_bars` becomes a thin map from it into `Bar`s. The test targets `bar_values`.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::Theme;
    use time::macros::date;

    fn week() -> [Date; 7] {
        [
            date!(2026 - 08 - 22), date!(2026 - 08 - 23), date!(2026 - 08 - 24),
            date!(2026 - 08 - 25), date!(2026 - 08 - 26), date!(2026 - 08 - 27),
            date!(2026 - 08 - 28),
        ]
    }

    #[test]
    fn bar_values_come_from_the_passed_week_not_the_global_config() {
        let theme = Theme::none();
        let week = week();
        let mut data = HashMap::new();
        data.insert(date!(2026 - 08 - 24), 480u32); // 8h on the third day
        let mut chart = WeeklyBarChart::new(date!(2026 - 08 - 24), &week, &theme);
        chart.set_weekly_data(&data);

        let values = chart.bar_values(6);

        assert_eq!(values.len(), 7);
        assert_eq!(values[2].0, date!(2026 - 08 - 24));
        assert_eq!(values[2].1, 80, "8h in tenths of an hour");
        assert_eq!(values[0].1, 0, "a day with no data is a zero bar, not absent");
    }
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test --lib tui::widgets::weekly_bar_chart`
Expected: FAIL — `WeeklyBarChart::new` takes one argument.

- [ ] **Step 3: Implement**

- Add `pub week_dates: [Date; 7]` to `App`, defaulting to the week containing `active_date`.
- In `load_data_for_active_date`, replace the local `week_start_day` / `week_dates` derivation with `self.ctx.week_start_day` and store the result: `self.week_dates = get_week_dates(&active_date, self.ctx.week_start_day).try_into().expect("get_week_dates always returns 7");`

  `get_week_dates` returns a `Vec<Date>` of exactly 7 — add a unit test in `src/time_utils.rs` asserting that so the `expect` is guarded rather than assumed:

```rust
#[test]
fn get_week_dates_always_returns_seven_days() {
    for start in [Weekday::Monday, Weekday::Saturday, Weekday::Sunday] {
        assert_eq!(get_week_dates(&date!(2026 - 08 - 24), start).len(), 7);
    }
}
```

- `WeeklyBarChart` gains `week_dates: &'a [Date; 7]` and `theme: &'a Theme`; delete the `parse_weekday` / `get_week_dates` / `Config` imports and the derivation inside `prepare_bars`.
- `ui.rs` passes `&self.week_dates` and `&self.ctx.theme` at both construction sites (line 14 zoomed, line 36 inline).

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib`
Expected: PASS.

- [ ] **Step 5: Verify the render path is Config-free**

Run: `grep -n 'Config\|parse_weekday\|get_week_dates' src/tui/widgets/weekly_bar_chart.rs`
Expected: no matches.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "perf(tui): derive week dates once per load instead of every frame"
```

---

## Task 4: Mode, Overlay, and focus-aware key dispatch (W20)

Today `handle_key_events` (`app.rs:217`) forwards **every** keypress to `project_list_widget` before matching app keys, with no awareness of what is on screen. With the help popup open, `j`/`k` still move the hidden list and `h`/`l` still change the date behind it; `Esc` — the key users press to dismiss a modal — quits the application. `ui.rs:13` early-returns on `zoom_bar`, so `?` does nothing visible while `f` is active.

**Files:**
- Create: `src/tui/mode.rs`
- Modify: `src/tui/app.rs:24-43,217-242`, `src/tui/ui.rs:11-68`, `src/tui/project_list.rs:62-90`

**Interfaces:**
- Consumes: `TuiContext` (Task 2).
- Produces:
  - `enum Mode { Day, Week, ZoomedWeek, RawFile }` — `Week` and `RawFile` render a placeholder until Tasks 20 and 16
  - `enum Overlay { Help, DatePrompt(String) }` — `DatePrompt` unreachable until Task 17
  - `enum Handled { Consumed, Emit(AppEvent), Ignored }`
  - `App.mode: Mode`, `App.overlay: Option<Overlay>`
  - `ProjectListWidget::handle_key_event(&mut self, KeyEvent) -> Handled`

- [ ] **Step 1: Write the failing tests**

`src/tui/mode.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{app::App, context::TuiContext};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use time::macros::date;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[tokio::test]
    async fn overlay_swallows_keys_the_mode_would_otherwise_handle() {
        let mut app = App::new(TuiContext::for_test()).with_active_date(date!(2026 - 08 - 24));
        app.overlay = Some(Overlay::Help);

        app.handle_key_events(key('l')).unwrap();
        app.handle_key_events(key('j')).unwrap();

        // The date must not have advanced behind the popup.
        assert_eq!(app.active_date, date!(2026 - 08 - 24));
    }

    #[tokio::test]
    async fn esc_closes_the_overlay_instead_of_quitting() {
        let mut app = App::new(TuiContext::for_test());
        app.overlay = Some(Overlay::Help);

        app.handle_key_events(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();

        assert!(app.overlay.is_none(), "Esc should close the overlay");
        assert!(app.running, "Esc must not quit while an overlay is open");
    }

    #[tokio::test]
    async fn esc_quits_when_no_overlay_is_open() {
        let mut app = App::new(TuiContext::for_test());
        app.handle_key_events(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
        app.drain_pending_events();
        assert!(!app.running);
    }
}
```

`drain_pending_events` is a `#[cfg(test)]` helper on `App` that pops everything currently queued on the event channel and runs it through the **non-async** part of `handle_app_event`, so key-dispatch tests can assert on resulting state without a full tokio loop. Add it in this task:

```rust
#[cfg(test)]
pub fn drain_pending_events(&mut self) {
    while let Ok(Event::App(e)) = self.events.try_next() {
        self.apply_sync_event(e);
    }
}
```

Split `handle_app_event` into `apply_sync_event(&mut self, AppEvent)` (everything that does not await: `Quit`, `ToggleHelp`, date changes, `ToggleZoomBar`) and the async remainder (`Edit`, load dispatch). This split is what makes the whole key layer testable — do it here, not later.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::mode`
Expected: FAIL — `Overlay` does not exist.

- [ ] **Step 3: Implement `mode.rs`**

```rust
use crate::tui::event::AppEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Day, Week, ZoomedWeek, RawFile }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Overlay { Help, DatePrompt(String) }

/// A key layer's verdict. `Ignored` falls through to the next layer;
/// `Consumed` and `Emit` stop there.
#[derive(Clone, Debug, PartialEq)]
pub enum Handled { Consumed, Emit(AppEvent), Ignored }
```

- [ ] **Step 4: Rewire dispatch in `app.rs`**

Replace `zoom_bar: bool` and `show_help: bool` with `mode: Mode` and `overlay: Option<Overlay>`. `handle_key_events` becomes:

```rust
pub fn handle_key_events(&mut self, key: KeyEvent) -> Result<()> {
    // 1. Overlay wins outright.
    if self.overlay.is_some() {
        match self.handle_overlay_key(key) {
            Handled::Consumed => return Ok(()),
            Handled::Emit(e) => { self.events.send(e); return Ok(()); }
            Handled::Ignored => return Ok(()), // overlays are modal: swallow the rest
        }
    }
    // 2. Then the active mode.
    match self.handle_mode_key(key) {
        Handled::Consumed => return Ok(()),
        Handled::Emit(e) => { self.events.send(e); return Ok(()); }
        Handled::Ignored => {}
    }
    // 3. Then global bindings (Task 5 replaces this match with the table).
    self.handle_global_key(key)
}
```

`handle_overlay_key` for `Overlay::Help` returns `Handled::Consumed` for every key, closing the overlay on `Esc`/`q`/`?` and ignoring the rest — this alone fixes the "Esc quits" bug. `handle_mode_key` forwards to `project_list_widget` only in `Mode::Day`.

- [ ] **Step 5: Rewire `ui.rs`**

```rust
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.mode {
            Mode::Day => self.render_day(area, buf),
            Mode::ZoomedWeek => self.render_zoomed_week(area, buf),
            Mode::Week => self.render_week(area, buf),      // Task 20
            Mode::RawFile => self.render_raw_file(area, buf), // Task 16
        }
        if let Some(overlay) = &self.overlay {
            match overlay {
                Overlay::Help => HelpPopup::new(&self.ctx.theme, self.mode).render(area, buf),
                Overlay::DatePrompt(buffer) => { /* Task 17 */ }
            }
        }
    }
}
```

`Mode::Week` and `Mode::RawFile` render a centered "not yet implemented" paragraph for now; Tasks 20 and 16 replace them. The overlay now renders in **every** mode, which is half of W6.

- [ ] **Step 6: Return `Handled` from the project list**

`ProjectListWidget::handle_key_event` returns `Handled::Consumed` where it returned `true` and `Handled::Ignored` where it returned `false`. Leave `Enter` returning `Handled::Consumed` and still calling `copy_selected_notes_to_clipboard` — Task 12 converts it to `Handled::Emit(AppEvent::CopyToClipboard(..))`.

- [ ] **Step 7: Run the tests**

Run: `cargo test --lib tui::`
Expected: PASS, including all three dispatch tests.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "refactor(tui): replace view booleans with a mode enum and focus-aware dispatch"
```

---

## Task 5: One binding table for the keymap, the help popup, and the README (W25)

The real keymap lives in three places that have already drifted by four bindings: the `match` at `app.rs:227`, the second match in `ProjectListWidget::handle_key_event`, the hardcoded string at `help_popup.rs:14-19`, and the README table at `README.md:192-199`. `app.rs` implements nine bindings; the popup and README document six.

**Carried from Task 4's review — read this with R1 below.** `handle_mode_key` currently delegates to `ProjectListWidget::handle_key_event`, and *that widget's internal `match`* is the second private keymap copy R1 exists to eliminate. Replacing `handle_global_key` with a `lookup()` call is therefore only half the job — you must also hollow out the widget's match so list navigation is driven by `BINDINGS` rows carrying `ModeMask::DAY`. If you finish and `ProjectListWidget` still owns a `match key_event.code`, R1 is not satisfied.

**Also carried from Task 4's review (Minor, cheap to fix here).** Opening an overlay is asynchronous — `?` queues `AppEvent::ToggleHelp` — while closing one is synchronous, mutating `self.overlay` directly inside `handle_overlay_key`. That leaves a one-iteration window after `?` in which `overlay` is still `None` and a following `j` reaches the project list behind the popup that is about to open. Not reachable by human typing at 30fps, but it is the same bug class Task 4 just fixed. Since you own the keymap, make open and close use the same mechanism.

**Pre-flight Ruling R1 — read before implementing.** `BINDINGS` carries *every* binding, list navigation and raw-file scrolling included. A key may appear in more than one row **provided their `ModeMask`s are disjoint**: `j` is one row with `ModeMask::DAY` (select next project) and a separate row with `ModeMask::RAW` (scroll down, added in Task 16). The mode layer dispatches by calling `lookup(key, mode)` and matching the resulting `AppEvent` — it does **not** keep a second private `match`. This is what keeps the `no_duplicate_key_within_a_mode` test meaningful while letting the same physical key mean different things in different modes, and it is what removes the third drifted copy of the keymap rather than merely adding a fourth.

**Files:**
- Create: `src/tui/keymap.rs`
- Modify: `src/tui/app.rs`, `src/tui/widgets/help_popup.rs`, `README.md:190-199`

**Interfaces:**
- Consumes: `Mode` (Task 4), `AppEvent` (existing).
- Produces:
  - `struct Binding { keys: &'static [(KeyCode, KeyModifiers)], event: AppEvent, modes: ModeMask, group: Group, description: &'static str }`
  - `const BINDINGS: &[Binding]`
  - `fn lookup(key: KeyEvent, mode: Mode) -> Option<&'static Binding>`
  - `fn help_rows(mode: Mode) -> Vec<(String, &'static str)>`
  - `fn readme_table() -> String`

- [ ] **Step 1: Write the failing tests**

```rust
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
        let readme = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/README.md")
        ).expect("README.md");
        assert!(
            readme.contains(&readme_table()),
            "README keybind table is stale — regenerate it from BINDINGS"
        );
    }

    #[test]
    fn lookup_respects_mode() {
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert!(lookup(j, Mode::Day).is_some());
        assert!(lookup(j, Mode::ZoomedWeek).is_none(), "list nav is Day-only");
    }
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::keymap`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement the table**

Seed it with exactly the nine bindings `app.rs` implements today plus the five the list widget owns. Later tasks add rows; **do not add unimplemented rows here.**

A hand-rolled bitmask keeps this dependency-free — do not reach for the `bitflags` crate:

```rust
#[derive(Clone, Copy, Debug)]
pub struct ModeMask(u8);

impl ModeMask {
    pub const DAY: Self = Self(1);
    pub const WEEK: Self = Self(2);
    pub const ZOOM: Self = Self(4);
    pub const RAW: Self = Self(8);
    pub const ALL: Self = Self(15);
    pub const fn or(self, o: Self) -> Self { Self(self.0 | o.0) }
    pub fn contains(self, m: Mode) -> bool { self.0 & Self::bit(m).0 != 0 }
    const fn bit(m: Mode) -> Self { match m {
        Mode::Day => Self::DAY, Mode::Week => Self::WEEK,
        Mode::ZoomedWeek => Self::ZOOM, Mode::RawFile => Self::RAW } }
}
```

`readme_table()` emits the exact markdown table body the README carries, so the test above is a real equality check rather than a loose `contains` of one row.

- [ ] **Step 4: Drive `handle_global_key` and `HelpPopup` from it**

`handle_global_key` becomes `if let Some(b) = lookup(key, self.mode) { self.events.send(b.event.clone()); }`. `HelpPopup::new(theme, mode)` renders `help_rows(mode)` grouped by `Group`.

- [ ] **Step 5: Regenerate the README table**

Add a `#[test] fn print_readme_table()` marked `#[ignore]` that prints `readme_table()`, run it with `cargo test -- --ignored print_readme_table --nocapture`, and paste the output over `README.md:192-199`. Re-run the suite so `readme_keybind_table_matches_the_binding_table` passes.

- [ ] **Step 6: Run the tests**

Run: `cargo test --lib tui::keymap && cargo test`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "refactor(tui): generate the keymap, help popup, and README from one table"
```

---

## Task 6: Cloneable event sender and off-loop loads (W24)

`EventHandler::send` takes `&mut self` and the sender is private (`event.rs:57,98`), so only `App` can emit an app event while it holds the loop — a watcher task is unimplementable. And `handle_app_event` awaits the load inline, so holding `l`/`h` stalls the UI on three concurrent file scans.

**Files:**
- Modify: `src/tui/event.rs:53-113`, `src/tui/app.rs:99-134,171-214`

**Interfaces:**
- Consumes: `TuiContext` (Task 2), `Handled` (Task 4).
- Produces:
  - `struct AppEventSender(mpsc::UnboundedSender<Event>)` with `send(&self, AppEvent)` — `&self`, cloneable
  - `EventHandler::sender(&self) -> AppEventSender`
  - `AppEvent::DataLoaded(u64, Box<LoadPayload>)`, `AppEvent::LoadFailed(u64, String)`
  - `struct LoadPayload { day: Option<TimeTrackingData>, populated: Vec<Date>, weekly: HashMap<Date, u32> }`
  - `App.load_gen: u64`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn stale_load_results_are_discarded() {
    let mut app = App::new(TuiContext::for_test());
    app.load_gen = 7;

    let stale = LoadPayload { day: None, populated: vec![date!(2026 - 01 - 01)], weekly: HashMap::new() };
    app.apply_sync_event(AppEvent::DataLoaded(6, Box::new(stale)));
    assert!(app.populated_dates.is_empty(), "generation 6 is stale and must be dropped");

    let fresh = LoadPayload { day: None, populated: vec![date!(2026 - 02 - 02)], weekly: HashMap::new() };
    app.apply_sync_event(AppEvent::DataLoaded(7, Box::new(fresh)));
    assert_eq!(app.populated_dates, vec![date!(2026 - 02 - 02)]);
}

#[test]
fn sender_can_be_cloned_and_used_without_mut() {
    let handler = EventHandler::new();
    let tx = handler.sender();
    let tx2 = tx.clone();
    tx.send(AppEvent::ReloadFromDisk);
    tx2.send(AppEvent::Today);
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test --lib tui::app`
Expected: FAIL — `AppEventSender` does not exist.

- [ ] **Step 2a: Restore exhaustiveness in `handle_app_event` (carried from Task 4's review — Important)**

Task 4 left `handle_app_event` with a catch-all `sync_event => self.apply_sync_event(sync_event)` arm, so it no longer fails to compile when a new `AppEvent` variant appears. On its own that would be tolerable — every variant the plan still adds is synchronous. The problem is the arm sitting directly below it in `apply_sync_event`:

```rust
AppEvent::ReloadFromDisk | AppEvent::Edit => {}   // "Both await; handle_app_event owns them."
```

That is an active template for `AppEvent::MyNewThing => {}`, at which point the event is silently dropped in *both* functions and nothing fails. Replace the catch-all with an explicit alternation:

```rust
e @ (AppEvent::ToggleZoomBar
    | AppEvent::ToggleHelp
    | AppEvent::Today
    | AppEvent::NextDate
    | AppEvent::PreviousDate
    | AppEvent::Quit) => self.apply_sync_event(e),
```

Add your own new variants (`DataLoaded`, `LoadFailed`) to whichever side owns them. This task rewrites this function anyway, which is why the fix lands here rather than as a Task 4 fix round.

- [ ] **Step 2b: Add the missing structural guard on `start()` (carried from Task 2's re-review)**

Task 2 split `EventHandler::new()` (channels only) from `start()` (spawns the poller), and `App::run` calls `start()` as its first statement. But **nothing pins that call.** A future refactor that drops the `start()` line would hang the real TUI on an empty channel while the whole test suite stays green — `App::run` takes `DefaultTerminal`, which is concrete over `CrosstermBackend<Stdout>`, so `run` is not callable from a test and no test can cover it.

Add the guard at the top of `EventHandler::next`, where it fires on the first await instead of hanging:

```rust
debug_assert!(
    self.task.is_none(),
    "EventHandler::start() was never called — the poller is not running and next() will block forever"
);
```

`start()` is `take()`-and-spawn, so `task.is_none()` means it ran. This costs nothing in release and converts a silent hang into an immediate, named panic in debug.

- [ ] **Step 3: Implement the sender**

```rust
#[derive(Clone, Debug)]
pub struct AppEventSender(mpsc::UnboundedSender<Event>);

impl AppEventSender {
    pub fn send(&self, app_event: AppEvent) {
        // The receiver only drops at shutdown; a failed send is expected then.
        let _ = self.0.send(Event::App(app_event));
    }
}

impl EventHandler {
    pub fn sender(&self) -> AppEventSender { AppEventSender(self.sender.clone()) }
}
```

Keep `EventHandler::send(&mut self, ..)` as-is so existing call sites compile unchanged.

- [ ] **Step 4: Spawn loads with a generation guard**

Replace the inline `load_data_for_active_date().await` in `handle_app_event` with a dispatcher:

```rust
fn spawn_load(&mut self) {
    self.load_gen += 1;
    let gen = self.load_gen;
    let tx = self.events.sender();
    let date = self.active_date;
    let week = self.week_dates;
    let ctx = self.ctx.clone();
    self.loading = true;
    self.dirty = true;
    tokio::spawn(async move {
        match load_payload(date, week, &ctx).await {
            Ok(p) => tx.send(AppEvent::DataLoaded(gen, Box::new(p))),
            Err(e) => tx.send(AppEvent::LoadFailed(gen, e.to_string())),
        }
    });
}
```

`load_payload` is a free `async fn` holding the `tokio::join!` currently at `app.rs:191-195`. On receipt:

```rust
AppEvent::DataLoaded(gen, payload) => {
    if gen != self.load_gen { return; }   // superseded — drop it
    self.loading = false;
    self.apply_payload(*payload);
    self.dirty = true;
}
AppEvent::LoadFailed(gen, msg) => {
    if gen != self.load_gen { return; }
    self.loading = false;
    tracing::warn!("load failed: {msg}");
    self.set_status(format!("Load failed: {msg}"));  // Task 12 renders it
    self.dirty = true;
}
```

`set_status` is a no-op stub in this task; Task 12 gives it a field and a footer.

- [ ] **Step 5: Run the tests**

Run: `cargo test --lib`
Expected: PASS.

- [ ] **Step 6: Sanity-run the real TUI**

Run: `cargo run -p cli -- --tui`
Hold `h` for a few seconds, then `l`, then `q`. Expected: the UI keeps repainting while scrubbing rather than freezing; the date lands where you stopped, not somewhere behind it.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "perf(tui): move data loads off the event loop with a latest-wins guard"
```

---

## Task 7: Redraw only on state change (W7)

**Two items carried from Task 6's review — both are requirements of this task.**

**(a) Abort superseded loads.** `spawn_load` keeps no `JoinHandle`, so a superseded load runs to completion in the background. Each one calls `find_populated_dates` over a ~90-day window, which spawns one task per date plus seven for the week — roughly 97 tasks per keypress, all contending on the single `Arc<Mutex<HashMap>>` cache. Holding `l` at key-repeat for three seconds is on the order of **7,000 live tasks**. The generation guard keeps the *result* correct; nothing bounds the *work*. Store the `JoinHandle` on `App`, `abort()` the previous one when a new load supersedes it, and log rather than silently drop a panic from the handle. (Task 10 separately collapses the 90-date fan-out into one `read_dir`, which reduces the per-load cost — but that is a smaller multiplier, not a substitute for bounding it.)

**(b) Your `dirty` seam has a gap.** This task's steps set `dirty` in `handle_key_events`, in `apply_sync_event`, and after `apply_payload`. `handle_app_event`'s own two arms — `ReloadFromDisk` and `Edit` — sit outside all three. That is harmless today only because `go_to_date` happens to run inside `apply_sync_event`; it is exactly the seam that will silently stop repainting when someone moves it. Set `dirty` there too, or set it centrally in the event-dispatch path rather than at each call site.


`run()` calls `terminal.draw` at the top of every loop iteration (`app.rs:78`) and the loop turns once per event, so the 30 FPS tick at `event.rs:9` forces thirty full re-renders per second forever with zero input and no animation. Each render rebuilds a `CalendarEventStore` from up to ninety dates and reallocates a `String` per project and per note bullet.

**Files:**
- Modify: `src/tui/app.rs:73-97`, `src/tui/event.rs:9`

**Interfaces:**
- Consumes: `AppEventSender` (Task 6).
- Produces: `App.dirty: bool`; `TICK_FPS = 4.0`.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn handled_keys_mark_the_app_dirty() {
    let mut app = App::new(TuiContext::for_test());
    app.dirty = false;
    app.handle_key_events(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE)).unwrap();
    assert!(app.dirty, "a handled key must request a redraw");
}

#[tokio::test]
async fn an_unbound_key_does_not_mark_the_app_dirty() {
    let mut app = App::new(TuiContext::for_test());
    app.dirty = false;
    app.handle_key_events(KeyEvent::new(KeyCode::Char('\u{1}'), KeyModifiers::NONE)).unwrap();
    assert!(!app.dirty, "an unbound key must not force a repaint");
}
```

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test --lib tui::app`
Expected: FAIL — no `dirty` field.

- [ ] **Step 3: Implement**

Set `dirty = true` centrally: at the end of `handle_key_events` when any layer returned `Consumed`/`Emit`, in `apply_sync_event`, after `apply_payload`, and on `Event::Crossterm(CrosstermEvent::Resize(..))` — which is currently discarded by the catch-all `_ => {}` at `app.rs:87`.

```rust
while self.running {
    if self.dirty {
        terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
        self.dirty = false;
    }
    match self.events.next().await.context("couldn't read events")? {
        Event::Tick => self.tick(),
        Event::Crossterm(crossterm::event::Event::Resize(_, _)) => self.dirty = true,
        Event::Crossterm(crossterm::event::Event::Key(k))
            if k.kind == crossterm::event::KeyEventKind::Press => self.handle_key_events(k)?,
        Event::Crossterm(_) => {}
        Event::App(e) => { /* unchanged */ }
    }
}
```

Set `dirty = true` once before the loop so the first frame paints. Drop `TICK_FPS` from `30.0` to `4.0`.

**Do not remove `Event::Tick`** — Task 12 expires the status line on it and Task 21 relies on the loop staying alive.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib`
Expected: PASS.

- [ ] **Step 5: Verify idle cost actually dropped**

Run: `cargo run --release -p cli -- --tui` and, in another shell, `top -p $(pgrep -f 'ttcli --tui') -b -n 5 | tail -20`
Expected: CPU at or near 0.0% while idle. Before this task it sits meaningfully above zero. Press a key and confirm the screen still updates immediately, then resize the terminal and confirm it repaints.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "perf(tui): redraw on state change instead of unconditionally at 30fps"
```

---

## Task 8: Width-aware, cached project-list rendering (W13 + W28)

Two items, merged because W28 makes W13's cache width-dependent — doing them separately means building the cache twice. Today `render_list` rebuilds `Vec<ListItem>` every frame, and `From<&ProjectItem> for ListItem` (`project_list.rs:233`) does a `format!` for the header plus a `format!` + `push_str` per note into a fresh `String` per project. Meanwhile nothing wraps: ratatui silently clips any line wider than the list area, so a long note just disappears at the right edge, and `{:<25}` pads by `char` count, so a CJK or emoji project name misaligns the hours column.

**Files:**
- Modify: `src/tui/project_list.rs:26-59,195-247`, `Cargo.toml`

**Interfaces:**
- Consumes: `Theme` (Task 2).
- Produces:
  - `ProjectItem.rendered: RefCell<Option<(u16, Text<'static>)>>` — body cached per width
  - `fn wrap_note(note: &str, width: u16, hanging_indent: usize) -> Vec<String>`
  - `fn pad_display_width(name: &str, cols: usize) -> String`

- [ ] **Step 1: Add the dependency**

```toml
unicode-width = "0.2"
```

- [ ] **Step 2: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_by_display_width_not_char_count() {
        // Each CJK glyph occupies two columns.
        assert_eq!(pad_display_width("日本語", 10).width(), 10);
        assert_eq!(pad_display_width("abc", 10).width(), 10);
    }

    #[test]
    fn wraps_long_notes_with_a_hanging_indent() {
        let lines = wrap_note("alpha beta gamma delta epsilon", 16, 5);
        assert!(lines.len() > 1, "a 30-char note must wrap at width 16");
        assert!(lines[0].len() <= 16);
        assert!(lines[1].starts_with("     "), "continuation lines are indented");
        for l in &lines { assert!(l.width() <= 16, "no line may exceed the width"); }
    }

    #[test]
    fn a_word_longer_than_the_width_is_hard_broken_not_dropped() {
        let lines = wrap_note("supercalifragilisticexpialidocious", 10, 2);
        let joined: String = lines.iter().map(|l| l.trim().to_string()).collect();
        assert!(joined.contains("supercali"), "content must survive wrapping");
    }

    #[test]
    fn body_is_rebuilt_when_the_width_changes() {
        let item = ProjectItem::new("admin".into(), 1.0, vec!["a fairly long note here".into()]);
        let narrow = item.body(20).height();
        let wide = item.body(80).height();
        assert!(narrow > wide, "a narrower pane needs more lines");
    }

    #[test]
    fn body_is_reused_at_the_same_width() {
        let item = ProjectItem::new("admin".into(), 1.0, vec!["note".into()]);
        let a = item.body(40);
        let b = item.body(40);
        assert_eq!(a, b);
        assert_eq!(item.rebuild_count(), 1, "same width must not rebuild");
    }
}
```

`rebuild_count()` is a `#[cfg(test)]` counter on `ProjectItem` incremented inside the cache-miss branch — it is what makes "is this actually memoized?" testable rather than assumed.

- [ ] **Step 3: Run them to make sure they fail**

Run: `cargo test --lib tui::project_list`
Expected: FAIL — helpers do not exist.

- [ ] **Step 4: Implement**

```rust
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Pad `name` with spaces to `cols` display columns, truncating with `…`
/// if it is wider. Uses display width, not char count, so CJK and emoji
/// keep the hours column aligned.
fn pad_display_width(name: &str, cols: usize) -> String {
    let w = name.width();
    if w <= cols {
        format!("{name}{}", " ".repeat(cols - w))
    } else {
        let mut out = String::new();
        let mut used = 0;
        for c in name.chars() {
            let cw = c.width().unwrap_or(0);
            if used + cw > cols.saturating_sub(1) { break; }
            out.push(c);
            used += cw;
        }
        out.push('…');
        out.push_str(&" ".repeat(cols.saturating_sub(used + 1)));
        out
    }
}
```

`wrap_note` greedily packs whitespace-separated words to `width` display columns, prefixing continuation lines with `hanging_indent` spaces, and hard-breaks any single word wider than the available room so content is never dropped.

`ProjectItem::body(&self, width: u16) -> Text<'static>` checks `self.rendered`; on a miss it builds the header line (`pad_display_width(name, 25)` + hours, pluralised exactly as today: `" {name}{hours} hour"` when `total_hours == 1.` else `" hours"`) plus wrapped bullets, stores `(width, text)`, and returns a clone. `render_list` passes `area.width.saturating_sub(4)` and applies only the alternating background at render time.

- [ ] **Step 5: Run the tests**

Run: `cargo test --lib tui::project_list`
Expected: PASS.

- [ ] **Step 6: Sanity-check visually**

Run: `cargo run -p cli -- --tui` on a day with a long note; shrink the terminal to ~60 columns.
Expected: notes wrap with a hanging indent instead of vanishing at the right edge.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "perf(tui): cache wrapped project-list bodies per width"
```

---

## Task 9: Cache parsed day data, not just file text (W23)

A single arrow-key press triggers `find_populated_dates` over ~90 dates plus `get_weekly_data` over 7 — roughly 97 `parse_day` calls. The cache stores only the file `String`, so every one of those markdown parses re-runs **on a full cache hit**.

**Files:**
- Modify: `src/data_svc.rs:17-26,99-112,228-239`

**Interfaces:**
- Consumes: nothing.
- Produces: `CacheEntry.parsed: Option<TimeTrackingData>`; `parse_day` served from cache.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn parse_day_is_memoized_between_calls() {
    let dir = tempfile::tempdir().unwrap();
    let svc = DataService::new_with_dir(60, dir.path().to_path_buf());
    let d = date!(2026 - 08 - 24);
    std::fs::write(dir.path().join("2026-08-24.md"), "8-10 admin\n  - note\n").unwrap();

    let first = svc.parse_day(&d).await.unwrap().unwrap();
    let second = svc.parse_day(&d).await.unwrap().unwrap();

    assert_eq!(first.total_minutes, second.total_minutes);
    assert_eq!(svc.parse_count(), 1, "second call must be served from cache");
}

#[tokio::test]
async fn touching_the_file_invalidates_the_parsed_cache() {
    let dir = tempfile::tempdir().unwrap();
    let svc = DataService::new_with_dir(60, dir.path().to_path_buf());
    let d = date!(2026 - 08 - 24);
    let path = dir.path().join("2026-08-24.md");

    std::fs::write(&path, "8-10 admin\n").unwrap();
    assert_eq!(svc.parse_day(&d).await.unwrap().unwrap().total_minutes, 120);

    // Sleep past filesystem mtime granularity, then rewrite.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    std::fs::write(&path, "8-12 admin\n").unwrap();

    assert_eq!(svc.parse_day(&d).await.unwrap().unwrap().total_minutes, 240);
}
```

`new_with_dir` and `parse_count` are new test seams: `DataService::new_with_dir(timeout, dir)` builds a service with an explicit data directory (no `Config`), and `parse_count()` is a `#[cfg(test)]` `AtomicUsize` bumped on every real parse. Both are needed because `DataService::get()` is a `OnceLock` reaching into `Config`.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib data_svc`
Expected: FAIL — `new_with_dir` does not exist.

- [ ] **Step 3: Implement**

Add `data_dir: PathBuf` to `DataService` (defaulted from `get_time_tracking_dir()` in `get()`), add `parsed: Option<TimeTrackingData>` to `CacheEntry`, and make `parse_day` check the cached parse before falling back to `parse_time_tracking_data` + store. `TimeTrackingData` derives `Clone`, so cached copies are handed out directly. Invalidation is unchanged — the existing mtime comparison in `get_cached_content` and `invalidate_date` already cover it; the parsed value simply lives in the same entry and dies with it.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib data_svc`
Expected: PASS. Note the second test takes ~1.1s by design (filesystem mtime granularity) — that is expected, not a hang.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "perf: cache parsed day data in DataService"
```

---

## Task 10: Directory listing instead of 90 per-date probes (W27)

Every arrow-key press spawns a `JoinSet` with one task per date across three months. Each `check_date_has_data` → `parse_day` → `read_day` → `get_file_path` call re-resolves the home directory and stats — and at `data_svc.rs:63-69` **creates** — the data directory before the file's own `exists()`/`metadata()`. That is several hundred syscalls to recompute a map that is identical 29 days out of 30.

**Files:**
- Modify: `src/data_svc.rs:62-75,131-176`, `src/tui/app.rs`

**Interfaces:**
- Consumes: `new_with_dir` (Task 9).
- Produces:
  - `DataService::ensure_data_dir(&self) -> Result<()>`
  - `DataService::existing_dates(&self, start: Date, end: Date) -> Result<Vec<Date>>`
  - `App.month_memo: HashMap<(i32, u8), Vec<Date>>`

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn existing_dates_lists_only_files_that_are_there() {
    let dir = tempfile::tempdir().unwrap();
    let svc = DataService::new_with_dir(60, dir.path().to_path_buf());
    std::fs::write(dir.path().join("2026-08-24.md"), "8-10 admin\n").unwrap();
    std::fs::write(dir.path().join("2026-08-26.md"), "8-10 admin\n").unwrap();
    std::fs::write(dir.path().join("notes.md"), "not a date\n").unwrap();
    std::fs::write(dir.path().join("2026-08-25.txt"), "wrong extension\n").unwrap();

    let got = svc.existing_dates(date!(2026 - 08 - 01), date!(2026 - 08 - 31)).await.unwrap();
    assert_eq!(got, vec![date!(2026 - 08 - 24), date!(2026 - 08 - 26)]);
}

#[tokio::test]
async fn an_existing_but_empty_file_is_not_populated() {
    let dir = tempfile::tempdir().unwrap();
    let svc = DataService::new_with_dir(60, dir.path().to_path_buf());
    std::fs::write(dir.path().join("2026-08-24.md"), "# just a header\n").unwrap();

    let got = svc.find_populated_dates(date!(2026 - 08 - 01), date!(2026 - 08 - 31)).await.unwrap();
    assert!(got.is_empty(), "a file with no logged time is not a populated date");
}

#[tokio::test]
async fn reading_does_not_create_the_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist");
    let svc = DataService::new_with_dir(60, missing.clone());

    let _ = svc.read_day(&date!(2026 - 08 - 24)).await.unwrap();

    assert!(!missing.exists(), "a read must not create the data directory");
}
```

That third test pins the behaviour change: today `get_file_path` creates the directory as a side effect of **every read**.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib data_svc`
Expected: FAIL — `existing_dates` does not exist, and the third test fails because reads currently create the directory.

- [ ] **Step 3: Implement**

- `get_file_path` no longer creates the directory — it only joins the formatted filename.
- `ensure_data_dir()` does the `create_dir_all`, called from `create_day_file_if_not_exists` and once at TUI/CLI startup.
- `existing_dates` does one `read_dir`, keeps entries matching `^\d{4}-\d{2}-\d{2}\.md$` (parse with `Date::parse(stem, DATE_FORMAT)`, no regex crate), filters to the range, and sorts. A missing directory yields an empty `Vec`, not an error.
- `find_populated_dates` calls `existing_dates` and parses only those, keeping the existing `!projects.is_empty() && total_minutes > 0` predicate so "populated" means exactly what it did.

- [ ] **Step 4: Add the month memo to `App`**

`App.month_memo: HashMap<(i32, u8), Vec<Date>>`. `spawn_load` (Task 6) checks whether the displayed month is already memoized; a date change within the same month reuses it and skips the populated-dates scan entirely. Cleared by `invalidate_date`, editor exit, and `r`.

```rust
#[tokio::test]
async fn same_month_navigation_reuses_the_memo() {
    let mut app = App::new(TuiContext::for_test());
    app.month_memo.insert((2026, 8), vec![date!(2026 - 08 - 24)]);
    assert!(app.month_scan_needed(date!(2026 - 08 - 25)).is_none());
    assert!(app.month_scan_needed(date!(2026 - 09 - 01)).is_some());
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test`
Expected: PASS, **including Task 1's characterization suite** — the directory-creation change touches the CLI path too.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "perf: list the data directory once instead of probing 90 dates per keypress"
```

---

## Task 11: Extract the weekly aggregation into DataService (W18)

The per-project weekly rollup exists but is computed inline in `show_weekly_summary` (`src/display/mod.rs:189-260`), interleaved with `println!` and `formatter.display_*` calls, so the TUI cannot reach it.

**Files:**
- Modify: `src/data_svc.rs`, `src/display/mod.rs:189-287`, `src/display/default.rs`, `src/display/plain.rs`, `src/display/markdown.rs`
- Modify: `cli/tests/cli_output_characterization.rs` (remove one `#[ignore]`)

**Interfaces:**
- Consumes: Task 9's parsed cache, Task 1's goldens.
- Produces:
  - `struct WeeklyProject { name: String, total_minutes: u32, notes: Vec<String> }`
  - `struct WeeklySummary { total_minutes: u32, dead_time_minutes: u32, projects: Vec<WeeklyProject>, warnings: Vec<String>, per_day: HashMap<Date, u32>, days: Vec<(Date, String, Option<TimeTrackingData>)> }`
  - `DataService::get_weekly_summary(&self, dates: &[Date]) -> Result<WeeklySummary>`
  - `DisplayFormatter::weekly_projects(&self, projects: &[WeeklyProject]) -> String` and the matching `display_weekly_projects`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn weekly_projects_sort_by_minutes_desc_then_name_asc() {
    let dir = tempfile::tempdir().unwrap();
    let svc = DataService::new_with_dir(60, dir.path().to_path_buf());
    // zulu and alpha tie at 120 minutes; beta is larger.
    std::fs::write(dir.path().join("2026-08-24.md"), "8-10 zulu\n10-12 alpha\n12-15 beta\n").unwrap();

    let week: Vec<Date> = (22..=28).map(|d| date!(2026 - 08 - 01).replace_day(d).unwrap()).collect();
    let s = svc.get_weekly_summary(&week).await.unwrap();

    let names: Vec<&str> = s.projects.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["beta", "alpha", "zulu"], "minutes desc, then name asc");
}

#[tokio::test]
async fn get_weekly_data_is_a_projection_of_the_summary() {
    let dir = tempfile::tempdir().unwrap();
    let svc = DataService::new_with_dir(60, dir.path().to_path_buf());
    std::fs::write(dir.path().join("2026-08-24.md"), "8-10 admin\n").unwrap();
    let week: Vec<Date> = (22..=28).map(|d| date!(2026 - 08 - 01).replace_day(d).unwrap()).collect();

    let summary = svc.get_weekly_summary(&week).await.unwrap();
    let per_day = svc.get_weekly_data(&week).await.unwrap();

    assert_eq!(summary.per_day, per_day);
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib data_svc`
Expected: FAIL — `get_weekly_summary` does not exist.

- [ ] **Step 3: Implement `get_weekly_summary`**

**Finding carried from Task 1's re-review — read before you move the warning block.** The filter
`!warning.contains("Error parsing time range '#'")` is **provably dead code** under the current
parser. Verified in `time-tracking-parser`'s `parser.rs`: the branch that emits
`Error parsing time range '{}'` sits in the `else` of `if !line.starts_with(char::is_numeric)`, so a
`#`-leading line always routes to the notes branch and can never produce a warning containing `'#'`.
Preserve the filter **verbatim anyway** — this task is behaviour-preserving, and a dead branch is not
yours to delete. Do not "simplify" it away; note it and move on. A separate follow-up can remove it
deliberately.

Move the collection loop verbatim: same `total_week_minutes` / `total_week_dead_minutes` accumulation, the same warning filter (`!warning.contains("Error parsing time range '#'")`) with the same `format!("{}: {}", format_day_with_date(day_date), warning)` shape, and the same per-note `format!("{}: {}", format_day_with_date(day_date), note)`. Notes stay in day order because `dates` is iterated in order. Then:

```rust
let mut projects: Vec<WeeklyProject> = week_projects
    .into_iter()
    .map(|(name, (total_minutes, notes))| WeeklyProject { name, total_minutes, notes })
    .collect();
projects.sort_by(|a, b| {
    b.total_minutes.cmp(&a.total_minutes).then_with(|| a.name.cmp(&b.name))
});
```

The `.then_with(|| a.name.cmp(&b.name))` is the tiebreak — it is what makes Task 1's ignored test pass.

- [ ] **Step 4: Change the formatter signature**

`weekly_projects` / `display_weekly_projects` take `&[WeeklyProject]`. In all three impls, change only the iteration (`p.name`, `p.total_minutes`, `p.notes` instead of `p.0`, `p.1.0`, `p.1.1`) — **do not touch a single format string, separator, or emoji.** Any change there shows up as a golden diff.

- [ ] **Step 5: Reduce `show_weekly_summary`**

It becomes: derive `week_dates`, call `get_weekly_summary`, then make the identical sequence of formatter calls in the identical order — header, totals, the `println!` warnings block, projects, breakdowns header, per-day loop, trailing `"=".repeat(80)`. The `daily_data` the per-day loop needs is `summary.days`.

- [ ] **Step 6: Un-ignore the tie test**

Delete the `#[ignore]` attribute on `weekly_tie_ordering_is_deterministic` in `cli/tests/cli_output_characterization.rs`.

- [ ] **Step 7: Run the full suite**

Run: `cargo test`
Expected: PASS — **including all four goldens unchanged**. A golden diff here means the extraction altered CLI output; investigate rather than re-blessing. Re-blessing a golden in this task defeats the entire point of Task 1.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "refactor: extract weekly aggregation from the stdout printer into DataService"
```

---

# Phase 2 — Features

## Task 12: Status line and App-owned clipboard (W3)

`Enter` yanking notes is the TUI's headline action and it produces zero feedback either way. On a headless box or over SSH with no clipboard backend it silently does nothing, the failure going only to the rolling log file the user cannot see because the alternate screen owns the terminal. A failed load renders identically to an empty day, and `r` gives no sign it did anything.

**Files:**
- Modify: `src/tui/app.rs`, `src/tui/project_list.rs:134-150,188-193`, `src/tui/event.rs`

**Interfaces:**
- Consumes: `Handled` (Task 4), `AppEventSender` (Task 6).
- Produces:
  - `AppEvent::CopyToClipboard(String, String)` — `(payload, success_message)`
  - `App.status: Option<(String, Instant)>`, `App.set_status(impl Into<String>)`
  - `App.clipboard: Option<copypasta::ClipboardContext>` — created lazily, once
  - Footer moves from `ProjectListWidget::render_footer` to `App`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn enter_emits_a_copy_intent_rather_than_doing_io() {
    let mut w = ProjectListWidget::new(&fixture_day(), &Theme::none());
    match w.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)) {
        Handled::Emit(AppEvent::CopyToClipboard(payload, msg)) => {
            assert!(payload.starts_with("- "), "notes are copied as a bullet list");
            assert!(msg.contains("admin"), "the toast names the project");
        }
        other => panic!("expected a copy intent, got {other:?}"),
    }
}

#[tokio::test]
async fn status_expires_after_its_ttl() {
    let mut app = App::new(TuiContext::for_test());
    app.set_status("Copied 4 notes for admin");
    assert!(app.status.is_some());

    app.status = Some(("stale".into(), Instant::now() - Duration::from_secs(10)));
    app.tick();
    assert!(app.status.is_none(), "an expired status must clear on tick");
}

#[tokio::test]
async fn an_expiring_status_requests_a_redraw() {
    let mut app = App::new(TuiContext::for_test());
    app.status = Some(("stale".into(), Instant::now() - Duration::from_secs(10)));
    app.dirty = false;
    app.tick();
    assert!(app.dirty, "clearing the toast must repaint so it actually disappears");
}

#[tokio::test]
async fn a_failed_load_surfaces_on_the_status_line() {
    let mut app = App::new(TuiContext::for_test());
    app.load_gen = 3;
    app.apply_sync_event(AppEvent::LoadFailed(3, "permission denied".into()));
    let (msg, _) = app.status.clone().expect("a failed load must set a status");
    assert!(msg.contains("permission denied"));
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::`
Expected: FAIL — `set_status` does not exist and `Enter` still performs I/O.

**Carried from Task 6 — a user-visible regression it introduced and this task must close. Task 6's review found it is worse than first described, so read this whole paragraph.**

It is not only a startup flash. `go_to_date` moves `active_date` and `week_dates` immediately, while `data`, `populated_dates` and `weekly_data` only move when the payload lands. So on **every date change** the day pane renders the *previous* date's project list underneath the *new* date's header — stale data presented as current, which reads worse than an empty state — and across a week boundary the bar chart draws the new `week_dates` against the old `weekly_data`, i.e. all-zero bars. Before loads went async this mismatch lasted a single frame; now it lasts the whole load.

Setting `status = "Loading…"` only *labels* the stale content. Decide deliberately whether to suppress the mismatched pane while a load is in flight rather than annotate it, and pin whichever you choose with a test that changes date and asserts on the frame drawn *before* the payload arrives. A test that only exercises startup, or only asserts after the payload lands, misses this entirely.

**One more thing carried from Task 7.** `App::tick` is currently `pub fn tick(&self) {}` — it takes `&self`, so it **structurally cannot** set `dirty` or mutate the status. That was deliberate in Task 7 (it guarantees a tick can never force a repaint, which is what takes idle cost to zero). Your status-expiry work needs it to become `&mut self`, and when you change it you must set `dirty` **only when a status actually expired** — not on every tick, or you reinstate the 4-per-second repaint Task 7 just removed. Pin that with a test: a tick with no expiring status must leave `dirty` false.

**The original startup note follows.** Moving loads off the event loop means startup now draws the empty state *before* the first load lands, so a cold cache shows a flash of "no data" before the day appears. `App.loading` is already set by `spawn_load`; rendering it is this task's job, and it is the reason `set_status` has been a no-op stub since Task 6. Make sure the loading indicator actually covers the startup window, not just subsequent navigations — a test that only exercises a keypress-triggered load would miss it.

- [ ] **Step 3: Implement**

- `App.status: Option<(String, Instant)>`; `STATUS_TTL: Duration = Duration::from_secs(4)`. `tick()` clears an expired status and sets `dirty`. This is the tick's only remaining job now that Task 7 decoupled redraw.
- `App.clipboard: Option<ClipboardContext>` built on first use. Handling `CopyToClipboard(payload, msg)`: on success `set_status(msg)`, on failure `set_status("Clipboard unavailable")` **and** `tracing::warn!`.
- Delete `copy_selected_notes_to_clipboard` from `ProjectListWidget` along with its `copypasta` import; `Enter` returns `Handled::Emit(AppEvent::CopyToClipboard(notes_text, format!("Copied {} notes for {}", n, name)))`.
- Move `render_footer` up to `App`, rendering `status` when set and `"? for help"` otherwise. `ProjectListWidget::render` drops its footer row. **This is what keeps the hint visible on the empty screen**, which Task 19 depends on.
- Replace the two `tracing::warn!` swallows at `app.rs:75` and `app.rs:91` with `set_status` + `tracing::warn!`.
- Set `status` to `"Loading…"` while `loading` is true.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib tui::`
Expected: PASS.

- [ ] **Step 5: Verify by hand**

Run: `cargo run -p cli -- --tui`, select a project, press `Enter`.
Expected: a toast naming the project, which disappears after ~4 seconds.
Then: `WAYLAND_DISPLAY= DISPLAY= cargo run -p cli -- --tui` and press `Enter`.
Expected: "Clipboard unavailable" rather than silence.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "feat(tui): add a status line and move clipboard IO into App"
```

---

## Task 13: Show the active date on the project pane (W9)

The block built at `ui.rs:51-53` with `self.active_date.format(DATE_FORMAT).unwrap()` is attached only to the "No data found" paragraph in the `else` branch. When `project_list_widget` is `Some` the block is **dropped unused** and the date string never reaches the screen — so after a few `h`/`l` presses there is no textual confirmation of which day is on screen.

**Files:**
- Modify: `src/tui/ui.rs:51-63`, `src/tui/project_list.rs:161-174`

**Interfaces:**
- Consumes: `Theme` (Task 2), `render_to_string` (Task 2).
- Produces: `fn format_pane_title(date: Date) -> String` → `"Thu 2026-08-27"`.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn the_day_pane_shows_the_active_date_with_its_weekday() {
    let mut app = App::new(TuiContext::for_test())
        .with_active_date(date!(2026 - 08 - 27))
        .with_data(fixture_day());
    let screen = render_to_string(&mut app, 100, 30);
    assert!(screen.contains("Thu 2026-08-27"), "got:\n{screen}");
}

#[tokio::test]
async fn the_empty_pane_also_shows_the_active_date() {
    let mut app = App::new(TuiContext::for_test()).with_active_date(date!(2026 - 08 - 27));
    let screen = render_to_string(&mut app, 100, 30);
    assert!(screen.contains("Thu 2026-08-27"), "got:\n{screen}");
}
```

This is the test that would have caught the current bug: the second assertion passes today, the first does not.

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test --lib tui::ui`
Expected: FAIL on the first assertion.

- [ ] **Step 3: Implement**

`format_pane_title` renders `"{weekday_short} {date}"` via the existing `WeekdayExt::short_name` and `DATE_FORMAT`, returning a `Result`-free `String` (fall back to the bare ISO date if formatting fails — **no `unwrap`**). Attach the bordered block to **both** branches, so the title shows whether or not the day has data.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib tui::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "feat(tui): show the active date and weekday on the project pane"
```

---

## Task 14: Dead time and parser warnings in the day header (W2)

Dead-time detection is a headline README feature computed on every parse, yet the TUI — the surface the user keeps open all day — is the only one that hides it. `ProjectListWidget::new` copies only `start_time`, `end_time` and `total_minutes` off the `TimeTrackingData` it is handed.

**Files:**
- Modify: `src/tui/project_list.rs:12-59,176-187`

**Interfaces:**
- Consumes: `Theme` (Task 2).
- Produces: `ProjectListWidget` carries `dead_time: String`, `dead_decimal: String`, `warnings: Vec<String>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn the_header_shows_dead_time() {
    let mut data = fixture_day();
    data.dead_time_minutes = 95;
    let mut app = App::new(TuiContext::for_test()).with_data(data);
    let screen = render_to_string(&mut app, 100, 30);
    assert!(screen.to_lowercase().contains("dead"), "got:\n{screen}");
}

#[tokio::test]
async fn parser_warnings_are_rendered() {
    let mut data = fixture_day();
    data.warnings = vec!["Error parsing time range 'x-y'".into()];
    let mut app = App::new(TuiContext::for_test()).with_data(data);
    let screen = render_to_string(&mut app, 100, 30);
    assert!(screen.contains("Error parsing time range"), "got:\n{screen}");
}

#[tokio::test]
async fn a_clean_day_renders_no_warning_block() {
    let mut app = App::new(TuiContext::for_test()).with_data(fixture_day());
    let screen = render_to_string(&mut app, 100, 30);
    assert!(!screen.to_lowercase().contains("warning"), "got:\n{screen}");
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::project_list`
Expected: FAIL — nothing renders dead time.

- [ ] **Step 3: Implement**

Capture `data.dead_time_minutes` (formatted via the parser's `formatted_dead_time_minutes()` / `formatted_dead_decimal()`) and `data.warnings` in `ProjectListWidget::new`. `render_header` gains a dead-time line beside Working Time, styled `theme.warning` below 90 minutes and `theme.error` at 90 or above — the same threshold `format_day_summary_impl` uses in `src/display/mod.rs`. Warnings render as a block below the header using `theme.error`. Grow the header `Constraint::Length(2)` to fit the extra rows, computing the height from what is actually present so a clean day loses no list space.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib tui::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "feat(tui): show dead time and parser warnings in the day header"
```

---

## Task 15: Week and month date motions (W1)

The TUI renders a three-month populated-date calendar that invites the user to look at a day last month, but the only route there is thirty to forty presses of `h`.

**Files:**
- Modify: `src/tui/keymap.rs`, `src/tui/event.rs`, `src/tui/app.rs`

**Interfaces:**
- Consumes: `BINDINGS` (Task 5), `month_offset` (existing, `app.rs:257`).
- Produces: `AppEvent::{NextWeek, PreviousWeek, NextMonth, PreviousMonth}` and four `BINDINGS` rows.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn shift_l_advances_a_week() {
    let mut app = App::new(TuiContext::for_test()).with_active_date(date!(2026 - 08 - 24));
    app.handle_key_events(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::SHIFT)).unwrap();
    app.drain_pending_events();
    assert_eq!(app.active_date, date!(2026 - 08 - 31));
}

#[tokio::test]
async fn bracket_steps_a_month_and_clamps_at_a_short_month_end() {
    let mut app = App::new(TuiContext::for_test()).with_active_date(date!(2026 - 01 - 31));
    app.handle_key_events(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE)).unwrap();
    app.drain_pending_events();
    // February has no 31st — land on the last valid day rather than failing.
    assert_eq!(app.active_date, date!(2026 - 02 - 28));
}

#[tokio::test]
async fn page_down_is_an_alias_for_next_month() {
    let mut app = App::new(TuiContext::for_test()).with_active_date(date!(2026 - 08 - 24));
    app.handle_key_events(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)).unwrap();
    app.drain_pending_events();
    assert_eq!(app.active_date, date!(2026 - 09 - 24));
}
```

The month-clamp case matters: `month_offset` operates on a first-of-month date today, so stepping from a 31st needs an explicit day clamp.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::`
Expected: FAIL — `H`/`L`/`[`/`]` are unbound.

- [ ] **Step 3: Implement**

Add the four events and four `BINDINGS` rows (`H`/`L` for week; `[`/`]` plus `PageUp`/`PageDown` for month, all `ModeMask::ALL`). Week stepping is `active_date ± 7 days` via `checked_add(7.days())`. Month stepping reuses `month_offset` on the first of the month, then re-applies `min(original_day, days_in_target_month)`.

Note `H`/`L` arrive with `KeyModifiers::SHIFT` set on most terminals — match on `KeyCode::Char('L')` and accept either `NONE` or `SHIFT` so the binding fires everywhere. Add a lookup test for both modifier states.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib tui::`
Expected: PASS, including the no-duplicate-key test from Task 5.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "feat(tui): add week and month date motions"
```

---

## Task 16: Raw file view (W15)

The prefix/suffix fencing feature means a day file can be full of text yet parse to zero entries, and the TUI's only response is "No data found for date" with no way to see why short of suspending to `$EDITOR`.

**Files:**
- Create: `src/tui/widgets/raw_file.rs`
- Modify: `src/tui/mode.rs`, `src/tui/ui.rs`, `src/tui/keymap.rs`, `src/tui/app.rs`

**Interfaces:**
- Consumes: `Mode::RawFile` (Task 4), `DataService::read_day`.
- Produces: `App.raw_content: Option<String>`, `App.raw_scroll: u16`, `AppEvent::ToggleRawFile`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn v_enters_raw_mode_and_shows_the_file_text() {
    let mut app = App::new(TuiContext::for_test());
    app.raw_content = Some("```timetracking\n8-10 admin\n```".into());
    app.mode = Mode::RawFile;
    let screen = render_to_string(&mut app, 80, 20);
    assert!(screen.contains("8-10 admin"), "got:\n{screen}");
}

#[tokio::test]
async fn raw_mode_scrolls_with_j_and_k() {
    let mut app = App::new(TuiContext::for_test());
    app.raw_content = Some((0..100).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n"));
    app.mode = Mode::RawFile;
    app.handle_key_events(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)).unwrap();
    assert_eq!(app.raw_scroll, 1);
    app.handle_key_events(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE)).unwrap();
    assert_eq!(app.raw_scroll, 0);
}

#[tokio::test]
async fn raw_scroll_does_not_go_negative() {
    let mut app = App::new(TuiContext::for_test());
    app.mode = Mode::RawFile;
    app.handle_key_events(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE)).unwrap();
    assert_eq!(app.raw_scroll, 0);
}

#[tokio::test]
async fn a_missing_file_says_so_rather_than_rendering_blank() {
    let mut app = App::new(TuiContext::for_test());
    app.raw_content = None;
    app.mode = Mode::RawFile;
    let screen = render_to_string(&mut app, 80, 20);
    assert!(screen.contains("No file"), "got:\n{screen}");
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::`
Expected: FAIL — `Mode::RawFile` renders the placeholder from Task 4.

- [ ] **Step 3: Implement**

`v` emits `ToggleRawFile`, which flips between `Mode::Day` and `Mode::RawFile` and, on entry, spawns a read of `DataService::read_day(&active_date)` delivered as an app event (reuse Task 6's sender rather than blocking). `RawFileView` renders a `Paragraph` with `.scroll((raw_scroll, 0))` inside a block titled with the file path. `j`/`k`/arrows scroll, clamped to `0..=lines.saturating_sub(visible)`. The mode is registered in `BINDINGS` with `ModeMask::DAY | ModeMask::RAW`.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib tui::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "feat(tui): view the raw day file without leaving the TUI"
```

---

## Task 17: Jump-to-date prompt (W16)

The CLI user can type `ttcli 'last friday'`; the TUI user, looking at a rendered calendar of that very month, cannot jump to a visible date. `config.rs:301` already makes exactly the call this needs.

**Files:**
- Create: `src/tui/widgets/date_prompt.rs`
- Modify: `src/tui/mode.rs`, `src/tui/ui.rs`, `src/tui/app.rs`, `src/tui/keymap.rs`

**Interfaces:**
- Consumes: `Overlay::DatePrompt(String)` (Task 4), `interim::parse_date_string`.
- Produces: `fn parse_prompt(input: &str, now: OffsetDateTime) -> Result<Date, String>`; `AppEvent::OpenDatePrompt`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn parses_an_iso_date() {
    let now = datetime!(2026 - 08 - 28 12:00 UTC);
    assert_eq!(parse_prompt("2026-08-14", now).unwrap(), date!(2026 - 08 - 14));
}

#[test]
fn parses_a_natural_language_date() {
    let now = datetime!(2026 - 08 - 28 12:00 UTC); // a Friday
    assert_eq!(parse_prompt("last friday", now).unwrap(), date!(2026 - 08 - 21));
}

#[test]
fn rejects_gibberish_with_a_message() {
    let now = datetime!(2026 - 08 - 28 12:00 UTC);
    assert!(parse_prompt("not a date at all", now).is_err());
}

#[tokio::test]
async fn typing_into_the_prompt_does_not_move_the_date() {
    let mut app = App::new(TuiContext::for_test()).with_active_date(date!(2026 - 08 - 24));
    app.overlay = Some(Overlay::DatePrompt(String::new()));
    for c in "last friday".chars() {
        app.handle_key_events(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)).unwrap();
    }
    app.drain_pending_events();
    assert_eq!(app.active_date, date!(2026 - 08 - 24), "l, i, d etc must not act as bindings");
    assert_eq!(app.overlay, Some(Overlay::DatePrompt("last friday".into())));
}

#[tokio::test]
async fn esc_cancels_without_changing_the_date() {
    let mut app = App::new(TuiContext::for_test()).with_active_date(date!(2026 - 08 - 24));
    app.overlay = Some(Overlay::DatePrompt("2026-01-01".into()));
    app.handle_key_events(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
    app.drain_pending_events();
    assert!(app.overlay.is_none());
    assert_eq!(app.active_date, date!(2026 - 08 - 24));
}

#[tokio::test]
async fn bad_input_keeps_the_prompt_open_and_reports() {
    let mut app = App::new(TuiContext::for_test());
    app.overlay = Some(Overlay::DatePrompt("gibberish".into()));
    app.handle_key_events(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert!(app.overlay.is_some(), "an unparseable date must not close the prompt");
    assert!(app.status.as_ref().unwrap().0.to_lowercase().contains("date"));
}
```

The fourth test is the one that proves Task 4's dispatch ordering is real: `l`, `i`, `d`, `f`, `r` and `a` are all live bindings elsewhere.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::`
Expected: FAIL — `parse_prompt` does not exist.

**Two hazards found during Task 4's review — handle both.**

1. `handle_overlay_key`'s `Overlay::DatePrompt(_)` arm currently returns `Handled::Consumed` for *every* key including `Esc`, and `render_overlay` draws nothing for it. Today that is unreachable because nothing constructs the variant — but the moment you make `:` construct it, an invisible modal that only Ctrl-C escapes becomes reachable. Wire `Esc` to clear the overlay as part of making the prompt real, not afterwards.
2. `is_ctrl_c` is checked *before* the overlay match, so Ctrl-C quits the application rather than cancelling your prompt. Conventionally Ctrl-C cancels a prompt. Decide deliberately which behaviour you want and pin it with a test either way — do not leave it to fall out of dispatch ordering.

- [ ] **Step 3: Implement**

```rust
pub fn parse_prompt(input: &str, now: OffsetDateTime) -> Result<Date, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Enter a date".into());
    }
    interim::parse_date_string(trimmed, now, interim::Dialect::Us)
        .map(|dt| dt.date())
        .map_err(|_| format!("Could not parse date: {trimmed}"))
}
```

`:` emits `OpenDatePrompt`, setting `overlay = Some(Overlay::DatePrompt(String::new()))`. The overlay's key handler takes `Char(c)` → push, `Backspace` → pop, `Esc` → clear the overlay, `Enter` → parse; on success set `active_date`, clear the overlay, and trigger a load; on failure `set_status(err)` and leave the overlay open. The widget renders a one-line input box with a visible cursor block at the end of the buffer.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib tui::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "feat(tui): add a jump-to-date prompt accepting natural-language dates"
```

---

## Task 18: Yank the day and week summaries (W10)

The stated workflow is pasting per-project totals into a timesheet or standup note, but `Enter` yanks one project's bullets **without its hours**. The `DisplayFormatter` trait already declares non-printing `day_summary` / `weekly_projects` / `weekly_totals` String-returning variants next to every `display_*` method for exactly this purpose, and the TUI currently never touches the formatter layer at all.

**Files:**
- Modify: `src/tui/app.rs`, `src/tui/keymap.rs`, `src/tui/event.rs`

**Interfaces:**
- Consumes: `CopyToClipboard` (Task 12), `ctx.formatter` (Task 2), `WeeklySummary` (Task 11).
- Produces: `AppEvent::{YankDay, YankWeek}` and two `BINDINGS` rows.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn y_yanks_a_day_summary_containing_project_hours() {
    let mut app = App::new(TuiContext::for_test()).with_raw_content("8-10 admin\n  - note\n");
    app.handle_key_events(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)).unwrap();
    match app.take_pending_copy() {
        Some((payload, _)) => {
            assert!(payload.contains("admin"));
            assert!(payload.contains('2'), "hours must be in the yanked summary");
        }
        None => panic!("y must emit a copy intent"),
    }
}

#[tokio::test]
async fn capital_y_yanks_the_week_summary() {
    let mut app = App::new(TuiContext::for_test());
    app.weekly_summary = Some(fixture_week_summary());
    app.handle_key_events(KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT)).unwrap();
    let (payload, _) = app.take_pending_copy().expect("Y must emit a copy intent");
    assert!(payload.contains("client-bd"));
}

#[tokio::test]
async fn yanking_an_empty_day_reports_rather_than_copying_nothing() {
    let mut app = App::new(TuiContext::for_test());
    app.handle_key_events(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)).unwrap();
    assert!(app.take_pending_copy().is_none());
    assert!(app.status.as_ref().unwrap().0.to_lowercase().contains("nothing"));
}
```

`take_pending_copy` is a `#[cfg(test)]` helper that drains the event queue and returns the first `CopyToClipboard` payload.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::`
Expected: FAIL — `y` is unbound.

- [ ] **Step 3: Implement**

Build the formatter from `ctx.formatter` via the same mapping `Config::get_formatter` uses (`src/config.rs:389`) — extract that mapping into a shared `Formatter::build(&self) -> Box<dyn DisplayFormatter>` so the TUI and CLI cannot drift. `y` calls `day_summary(&raw_content, "", prefix, suffix)`; `Y` renders `weekly_totals` + `weekly_projects` over `WeeklySummary`. Both emit `CopyToClipboard(text, "Copied day summary" / "Copied week summary")`; an empty source sets a status instead of copying an empty string.

`prefix`/`suffix` come from config — add them to `TuiContext` in this task (`prefix: Option<String>`, `suffix: Option<String>`) rather than reaching for `Config::get()`, and update `TuiContext::for_test` accordingly.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib tui::`
Expected: PASS. Re-run `grep -rn 'Config::get()' src/tui/` and confirm still no matches outside `tui()`.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "feat(tui): yank the day or week summary to the clipboard"
```

---

## Task 19: A useful empty state (W11)

This is the first screen a new user sees if they launch `--tui` before writing anything, and the screen they hit on every weekend or future date. It states a negative fact and offers no way forward.

**Files:**
- Modify: `src/tui/ui.rs:55-63`, `src/tui/app.rs`

**Interfaces:**
- Consumes: `raw_content` (Task 16), `format_pane_title` (Task 13), App-level footer (Task 12).
- Produces: `enum DayState { Populated, FileWithNoEntries, NoFile }`, `App.day_state() -> DayState`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn no_file_offers_to_create_one() {
    let mut app = App::new(TuiContext::for_test()).with_active_date(date!(2026 - 08 - 30));
    assert_eq!(app.day_state(), DayState::NoFile);
    let screen = render_to_string(&mut app, 100, 30);
    assert!(screen.contains("Sun 2026-08-30"), "got:\n{screen}");
    assert!(screen.contains("press e"), "got:\n{screen}");
}

#[tokio::test]
async fn a_file_that_parses_to_nothing_says_so_and_points_at_v() {
    let mut app = App::new(TuiContext::for_test())
        .with_active_date(date!(2026 - 08 - 30))
        .with_raw_content("# just a heading, no entries\n");
    assert_eq!(app.day_state(), DayState::FileWithNoEntries);
    let screen = render_to_string(&mut app, 100, 30);
    assert!(screen.contains("no time entries"), "got:\n{screen}");
    assert!(screen.contains("press v"), "got:\n{screen}");
}

#[tokio::test]
async fn the_help_hint_survives_the_empty_screen() {
    let mut app = App::new(TuiContext::for_test()).with_active_date(date!(2026 - 08 - 30));
    let screen = render_to_string(&mut app, 100, 30);
    assert!(screen.contains("? for help"), "the hint must not vanish on an empty day:\n{screen}");
}
```

The third test is the regression: today the hint lives inside `ProjectListWidget::render`, so it disappears exactly when the user most needs it. Task 12 already moved the footer; this pins it.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::ui`
Expected: FAIL — `day_state` does not exist.

- [ ] **Step 3: Implement**

`day_state()` returns `Populated` when `project_list_widget.is_some()`, `FileWithNoEntries` when `raw_content.is_some()`, else `NoFile`. The empty branch renders the pane title plus a centered call to action: `NoFile` → "No file for this day yet. press e to create and edit it · press t for today"; `FileWithNoEntries` → "This file has no time entries the parser recognised. press v to see the raw text · press e to edit".

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib tui::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "feat(tui): give the empty-date screen a call to action"
```

---

## Task 20: Weekly per-project rollup pane (W17)

The consultant's actual weekly billing question is "how many hours did client-bd get this week", and timesheets are filed weekly. That answer exists behind `ttcli --week` and behind the SPA's WeeklySummary page but not in the TUI, where the bar chart teases weekly data yet only answers "how long did I work Tuesday".

**Files:**
- Create: `src/tui/week_list.rs`
- Modify: `src/tui/ui.rs`, `src/tui/app.rs`, `src/tui/keymap.rs`, `src/tui/event.rs`

**Interfaces:**
- Consumes: `WeeklySummary` (Task 11), `Mode::Week` (Task 4), `CopyToClipboard` (Task 12).
- Produces: `WeekListWidget`, `App.weekly_summary: Option<WeeklySummary>`, `AppEvent::ToggleWeekMode`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn week_mode_lists_projects_with_hours_biggest_first() {
    let mut app = App::new(TuiContext::for_test());
    app.weekly_summary = Some(fixture_week_summary()); // client-bd 18h, internal 9.5h, admin 6h
    app.mode = Mode::Week;
    let screen = render_to_string(&mut app, 100, 30);
    let bd = screen.find("client-bd").expect("client-bd must be listed");
    let admin = screen.find("admin").expect("admin must be listed");
    assert!(bd < admin, "biggest project first:\n{screen}");
    assert!(screen.contains("18"), "hours must be shown:\n{screen}");
}

#[tokio::test]
async fn week_mode_shows_the_week_total_and_dead_time() {
    let mut app = App::new(TuiContext::for_test());
    app.weekly_summary = Some(fixture_week_summary());
    app.mode = Mode::Week;
    let screen = render_to_string(&mut app, 100, 30);
    assert!(screen.contains("33.5") || screen.contains("33"), "week total:\n{screen}");
    assert!(screen.to_lowercase().contains("dead"), "week dead time:\n{screen}");
}

#[tokio::test]
async fn enter_in_week_mode_yanks_that_projects_week_notes() {
    let mut app = App::new(TuiContext::for_test());
    app.weekly_summary = Some(fixture_week_summary());
    app.mode = Mode::Week;
    app.handle_key_events(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    let (payload, _) = app.take_pending_copy().expect("Enter must emit a copy intent");
    assert!(payload.contains("client-bd"), "the selected (first) project is yanked");
}

#[tokio::test]
async fn week_mode_with_no_data_renders_an_empty_state_not_a_panic() {
    let mut app = App::new(TuiContext::for_test());
    app.weekly_summary = None;
    app.mode = Mode::Week;
    let screen = render_to_string(&mut app, 100, 30);
    assert!(!screen.trim().is_empty());
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::week_list`
Expected: FAIL — `Mode::Week` still renders Task 4's placeholder.

- [ ] **Step 3: Implement**

`spawn_load` additionally calls `DataService::get_weekly_summary(&week_dates)` and delivers it in `LoadPayload`. `WeekListWidget` mirrors `ProjectListWidget`'s structure — header (week range, total hours, dead time), a selectable list of `WeeklyProject` rows with hours, `j`/`k`/`g`/`G` navigation, `Enter` emitting `CopyToClipboard` of that project's notes and hours. `w` toggles between `Mode::Day` and `Mode::Week`.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib tui::`
Expected: PASS.

- [ ] **Step 5: Verify against the CLI**

Run: `cargo run -p cli -- --week --date 2026-08-24` and, separately, the TUI's `w` pane on the same week.
Expected: the same projects in the same order with the same hours. A mismatch means the pane is not consuming `WeeklySummary` faithfully.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "feat(tui): add a weekly per-project rollup pane"
```

---

## Task 21: Auto-refresh when the day file changes on disk (W5)

The last unchecked TUI line in `TODO.md`. The intended workflow is the TUI open in one pane while the user edits the same markdown file in Obsidian or neovim — the repo ships a neovim plugin — and today they must remember to press `r` while the chart and totals silently go stale.

**Files:**
- Modify: `src/tui/app.rs`, `src/tui/mod.rs`

**Interfaces:**
- Consumes: `AppEventSender` (Task 6).
- Produces: `fn spawn_mtime_watch(path: PathBuf, tx: AppEventSender) -> tokio::task::JoinHandle<()>`; `App.watch: Option<JoinHandle<()>>`.

- [ ] **Step 1: Write the failing test**

`EventHandler::next` takes `&mut self`, so these tests drive a bare channel and construct the sender directly rather than borrowing a whole handler. Give `AppEventSender` a `#[cfg(test)] pub fn from_raw(tx: mpsc::UnboundedSender<Event>) -> Self` for exactly this.

```rust
#[tokio::test]
async fn the_watcher_posts_a_reload_when_the_file_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("2026-08-24.md");
    std::fs::write(&path, "8-10 admin\n").unwrap();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let _watch = spawn_mtime_watch(path.clone(), AppEventSender::from_raw(tx));

    // Sleep past filesystem mtime granularity before touching the file.
    tokio::time::sleep(Duration::from_millis(1200)).await;
    std::fs::write(&path, "8-12 admin\n").unwrap();

    let got = tokio::time::timeout(Duration::from_secs(4), async {
        while let Some(ev) = rx.recv().await {
            if matches!(ev, Event::App(AppEvent::ReloadFromDisk)) {
                return;
            }
        }
    })
    .await;
    assert!(got.is_ok(), "watcher did not post a reload within 4s");
}

#[tokio::test]
async fn the_watcher_is_quiet_when_nothing_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("2026-08-24.md");
    std::fs::write(&path, "8-10 admin\n").unwrap();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let _watch = spawn_mtime_watch(path, AppEventSender::from_raw(tx));

    tokio::time::sleep(Duration::from_millis(2500)).await;

    assert!(
        rx.try_recv().is_err(),
        "an unchanged file must not trigger any reload"
    );
}
```

The second test is the important one — a watcher that fires every second is worse than no watcher, because Task 7 just made idle cost zero.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::app`
Expected: FAIL — `spawn_mtime_watch` does not exist.

- [ ] **Step 3: Implement**

```rust
pub fn spawn_mtime_watch(path: PathBuf, tx: AppEventSender) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut last = tokio::fs::metadata(&path).await.ok().and_then(|m| m.modified().ok());
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            let now = tokio::fs::metadata(&path).await.ok().and_then(|m| m.modified().ok());
            if now != last {
                last = now;
                tx.send(AppEvent::ReloadFromDisk);
            }
        }
    })
}
```

`App` aborts and respawns the watch whenever `active_date` changes, and aborts it on quit. A file that does not exist yields `None`, so its later creation is itself a change — which is the right behaviour when the user runs `e` or creates the day elsewhere.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib tui::app`
Expected: PASS. These tests take ~4s by design.

- [ ] **Step 5: Verify by hand**

Run `cargo run -p cli -- --tui` in one pane; in another, append a line to today's file.
Expected: the TUI updates within about a second without a keypress.

- [ ] **Step 6: Update TODO.md**

Tick the line: `- [x] Poll for changes in the file and update TUI for live preview`.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "feat(tui): auto-refresh when the day file changes on disk"
```

---

# Phase 3 — Polish

## Task 22: Config-driven theme with env detection (W21)

On a light-background terminal the near-black slate row stripes sit as dark blocks over a white page and the pale blue calendar days lose contrast; over SSH to an 8/16-colour `TERM` the truecolor values are approximated unpredictably. Grep confirms no occurrence of `NO_COLOR`, `COLORTERM` or any theme concept anywhere in the repo today.

**Files:**
- Modify: `src/tui/theme.rs`, `src/tui/context.rs`, `src/config.rs:113-152,159-183,398-411`

**Interfaces:**
- Consumes: `Theme` (Task 2).
- Produces:
  - `Config.theme: Option<String>`
  - `enum Preset { Dark, Light, None }`, `Preset::from_str`
  - `Theme::resolve(configured: Option<&str>, env: &ThemeEnv) -> Theme`
  - `struct ThemeEnv { no_color: bool, colorterm: Option<String> }` with `ThemeEnv::from_env()`

- [ ] **Step 1: Write the failing tests**

Resolution is pure over an injected `ThemeEnv`, so no test mutates process env:

```rust
fn env(no_color: bool, colorterm: Option<&str>) -> ThemeEnv {
    ThemeEnv { no_color, colorterm: colorterm.map(str::to_string) }
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
    for style in [t.populated_date, t.active_date, t.row_bg, t.selection, t.list_header] {
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
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::theme`
Expected: FAIL — `Theme::resolve` does not exist.

- [ ] **Step 3: Implement**

`Theme::light()` mirrors `dark()` with light-appropriate values (`SLATE.c50` / `SLATE.c100` row backgrounds, `BLUE.c700` populated, `BLUE.c100` selection). `Theme::to_ansi16(self)` maps every `Color::Rgb`/tailwind value to the nearest named ANSI colour. `resolve` applies precedence `NO_COLOR` → config → default, then downgrades unless `COLORTERM` is `truecolor` or `24bit`.

Add `theme: Option<String>` to `Config` (default `Some("dark".into())`) and extend `write_config_comments`:

```rust
file.write_all(b"\n# TUI theme preset: \"dark\", \"light\", or \"none\".\n")?;
file.write_all(b"# \"none\" emits no colors so your terminal palette shows through.\n")?;
file.write_all(b"# NO_COLOR in the environment forces \"none\".\n")?;
file.write_all(b"#theme = \"dark\"\n")?;
```

`TuiContext::from_config` switches to `Theme::resolve(config.theme.as_deref(), &ThemeEnv::from_env())`.

- [ ] **Step 4: Run the tests**

Run: `cargo test`
Expected: PASS, including the existing `config.rs` round-trip tests — the new field is optional and must not break them.

- [ ] **Step 5: Verify by hand**

Run: `NO_COLOR=1 cargo run -p cli -- --tui` — expect no colour at all.
Run: `COLORTERM= cargo run -p cli -- --tui` — expect the 16-colour palette.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "feat(tui): add config-driven light, dark, and no-color themes"
```

---

## Task 23: Scale the weekly chart to the data, not the terminal (W8)

`max_value` is `(content_height as u64 * 10).max(160)` (`weekly_bar_chart.rs:127`), so the y-axis ceiling is derived from how many rows the widget happens to get. An eight-hour day renders at half height inline and shrinks further the taller the terminal gets — zoom with `f` on a 44-row terminal and the ceiling lands near forty hours, turning a full working day into a stub.

**Files:**
- Modify: `src/tui/widgets/weekly_bar_chart.rs:95-186`, `src/config.rs`, `src/tui/context.rs`

**Interfaces:**
- Consumes: `TuiContext.daily_target_hours` (Task 2).
- Produces:
  - `Config.daily_target_hours: Option<f64>` (default 8.0)
  - `fn chart_ceiling(week_max_minutes: u32, target_hours: f64) -> u64` — returns tenths of an hour, matching the existing `minutes * 10 / 60` bar scale
  - `WeeklyBarChart::ceiling_for(&self, area: Rect) -> u64` — the widget-level wrapper the
    height-independence test calls. It ignores `area` entirely for the ceiling (that is the
    point of the test) and returns `chart_ceiling(self.week_max_minutes(), self.target_hours)`.
    *(Pre-flight Ruling R2: implied by Task 23's test but not previously declared.)*

- [ ] **Step 1: Write the failing tests**

```rust
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
        assert!(bar <= ceiling, "{minutes}min bar {bar} exceeds ceiling {ceiling}");
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
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::widgets::weekly_bar_chart`
Expected: FAIL — `chart_ceiling` does not exist.

- [ ] **Step 3: Implement**

```rust
/// Chart ceiling in tenths of an hour: at least the daily target, and
/// always at least the tallest bar, rounded up to a whole hour.
fn chart_ceiling(week_max_minutes: u32, target_hours: f64) -> u64 {
    let target_tenths = (target_hours * 10.0).round().max(10.0) as u64;
    let max_tenths = u64::from(week_max_minutes) * 10 / 60;
    let needed = target_tenths.max(max_tenths);
    needed.div_ceil(10) * 10 // round up to a whole hour
}
```

`calculate_bar_dimensions` stops returning `max_value`; the ceiling comes from `chart_ceiling(week_max, ctx.daily_target_hours)`. Draw a goal marker row at the target using `theme.goal_marker`.

Also replace the hand-rolled total-hours overlay at lines 166-175 — computed from `area` rather than the block's inner area, and using `total_text.len()` (**bytes**) as a column count — with `Block::title_top(Line::from(total_text).right_aligned())`.

Add `daily_target_hours: Option<f64>` to `Config` with a config comment, and read it in `TuiContext::from_config`.

- [ ] **Step 4: Run the tests**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 5: Verify by hand**

Run the TUI on a week with an ~8h day, press `f`, and resize the terminal tall and short.
Expected: the bar keeps the same proportion of the frame; the goal line sits at 8h.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "fix(tui): scale the weekly chart to the data instead of terminal height"
```

---

## Task 24: Responsive layout with a minimum-size notice (W26)

On a stock 80×24 terminal the fixed `Vertical[Length(12), Min(9)]` band leaves the project list twelve rows, of which the header takes two and the footer one — nine rows for the actual content. Narrower than 24 columns the calendar consumes everything and the chart is squeezed to nothing.

**Files:**
- Modify: `src/tui/ui.rs:22-49`, `src/tui/widgets/help_popup.rs:29-35`

**Interfaces:**
- Consumes: `Mode` (Task 4).
- Produces:
  - `enum Breakpoint { TooSmall, Compact, Narrow, Full }`
  - `fn breakpoint(area: Rect) -> Breakpoint`
  - `const MIN_COLS: u16 = 60; const MIN_ROWS: u16 = 15;`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn breakpoints_are_chosen_by_size() {
    assert_eq!(breakpoint(Rect::new(0, 0, 50, 20)), Breakpoint::TooSmall);
    assert_eq!(breakpoint(Rect::new(0, 0, 100, 12)), Breakpoint::TooSmall);
    assert_eq!(breakpoint(Rect::new(0, 0, 80, 20)), Breakpoint::Compact);
    assert_eq!(breakpoint(Rect::new(0, 0, 80, 30)), Breakpoint::Narrow);
    assert_eq!(breakpoint(Rect::new(0, 0, 120, 30)), Breakpoint::Full);
}

#[tokio::test]
async fn a_tiny_terminal_gets_a_notice_naming_the_required_size() {
    let mut app = App::new(TuiContext::for_test()).with_data(fixture_day());
    let screen = render_to_string(&mut app, 50, 10);
    assert!(screen.contains("60"), "the notice names the required width:\n{screen}");
    assert!(screen.contains("15"), "the notice names the required height:\n{screen}");
}

#[tokio::test]
async fn a_narrow_terminal_drops_the_calendar_for_the_chart() {
    let mut app = App::new(TuiContext::for_test()).with_data(fixture_day());
    let narrow = render_to_string(&mut app, 80, 30);
    let wide = render_to_string(&mut app, 140, 30);
    // The calendar renders a weekday header row; it should be gone when narrow.
    assert!(wide.contains("Su"), "the wide layout keeps the calendar:\n{wide}");
    assert!(!narrow.contains("Su"), "the narrow layout drops it:\n{narrow}");
}

#[tokio::test]
async fn a_short_terminal_keeps_the_project_list_usable() {
    let mut app = App::new(TuiContext::for_test()).with_data(fixture_day_with_projects(6));
    let screen = render_to_string(&mut app, 100, 20);
    let listed = ["admin", "client-bd", "internal"].iter().filter(|p| screen.contains(**p)).count();
    assert!(listed >= 3, "the collapsed chart band must give the list room:\n{screen}");
}

#[tokio::test]
async fn no_render_panics_at_any_plausible_size() {
    for (w, h) in [(1, 1), (10, 3), (40, 10), (60, 15), (80, 24), (200, 60), (400, 100)] {
        let mut app = App::new(TuiContext::for_test()).with_data(fixture_day());
        let _ = render_to_string(&mut app, w, h);
    }
}
```

That last test is cheap insurance — ratatui panics on some zero-width layout arithmetic, and Task 23's `title_top` plus this task's constraints are both places that can produce it.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::ui`
Expected: FAIL — `breakpoint` does not exist; the 1×1 case likely panics.

- [ ] **Step 3: Implement**

`TooSmall` below 60×15; `Compact` below 22 rows (chart band collapses to `Length(0)`, list takes the rest); `Narrow` below 100 columns (calendar dropped, chart full width); `Full` otherwise. On very wide terminals cap the chart at ~140 columns and centre. `popup_area` clamps with `Constraint::Length(min(needed, area.dim - 4))` instead of a flat 60% square, so help stays readable at both extremes.

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib tui::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "feat(tui): make the layout responsive with a minimum-size notice"
```

---

## Task 25: Complete, modal, generated help (W6)

Date navigation appears in neither the popup nor the README, so a user who reads both learns no way to look at yesterday. Tasks 4 and 5 already made the popup modal and table-driven; this task closes the remaining gaps and pins them.

**Files:**
- Modify: `src/tui/widgets/help_popup.rs`, `README.md`, `src/tui/keymap.rs`

**Interfaces:**
- Consumes: `help_rows` / `readme_table` (Task 5), `Breakpoint` (Task 24).
- Produces: nothing new — this is the closing verification of W6 and W25.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn every_implemented_binding_appears_in_the_help_popup() {
    let mut app = App::new(TuiContext::for_test()).with_data(fixture_day());
    app.overlay = Some(Overlay::Help);
    let screen = render_to_string(&mut app, 120, 40);
    for b in BINDINGS.iter().filter(|b| b.modes.contains(Mode::Day)) {
        assert!(
            screen.contains(b.description),
            "binding {:?} is missing from help:\n{screen}",
            b.keys
        );
    }
}

#[tokio::test]
async fn help_renders_over_the_zoomed_chart() {
    // Regression: ui.rs used to early-return on zoom_bar before the help check,
    // so `?` did nothing visible while `f` was active.
    let mut app = App::new(TuiContext::for_test()).with_data(fixture_day());
    app.mode = Mode::ZoomedWeek;
    app.overlay = Some(Overlay::Help);
    let screen = render_to_string(&mut app, 120, 40);
    assert!(screen.contains("Help"), "got:\n{screen}");
}

#[tokio::test]
async fn help_lists_the_date_motions() {
    let mut app = App::new(TuiContext::for_test());
    app.overlay = Some(Overlay::Help);
    let screen = render_to_string(&mut app, 120, 40);
    for expected in ["previous day", "next week", "next month", "jump to date"] {
        assert!(screen.contains(expected), "help omits {expected}:\n{screen}");
    }
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test --lib tui::widgets::help_popup`
Expected: FAIL — descriptions do not all match, or the popup is clipped at 60%.

- [ ] **Step 3: Implement**

Group `help_rows` output under `Group` headings, size the popup to its content via Task 24's clamped `popup_area`, and make the descriptions in `BINDINGS` read as the strings the tests expect. Confirm every binding added in Tasks 15–20 carries a row.

- [ ] **Step 4: Regenerate the README table**

Run: `cargo test -- --ignored print_readme_table --nocapture`, paste over the README table, then run `cargo test --lib tui::keymap` so `readme_keybind_table_matches_the_binding_table` passes.

- [ ] **Step 5: Run the tests**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "docs(tui): generate complete help and README keybinds from the binding table"
```

---

## Task 26: Stop advertising a webserver that is not running (W14)

In `--tui` mode `cli/src/main.rs:91` writes to stdout while the spawned TUI task is concurrently entering the alternate screen, so it either corrupts the first frame or lurks on the normal screen as the first thing the user sees after quitting. Either way it teaches `ctrl-c` as the quit key and blames a webserver that is not running, when `q` is the actual quit key.

**Files:**
- Modify: `cli/src/main.rs:34-98`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn tui_only_launch_prints_no_webserver_banner() {
    // --tui with no TTY exits immediately; we only care about stdout.
    let dir = staged("week_no_ties");
    let out = Command::new(env!("CARGO_BIN_EXE_ttcli"))
        .args(["--tui", "--data-directory"])
        .arg(dir.path())
        .env("TERM", "dumb")
        .output()
        .expect("run ttcli");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("webserver"),
        "a TUI-only launch must not mention the webserver: {stdout}"
    );
}
```

Add it to `cli/tests/cli_output_characterization.rs`, reusing `staged`. If `--tui` cannot start without a TTY in CI it will exit non-zero — that is fine, the assertion is on stdout only, so do **not** assert `status.success()` here.

- [ ] **Step 2: Run it to make sure it fails**

Run: `cargo test -p cli --test cli_output_characterization tui_only`
Expected: FAIL — the banner is printed unconditionally whenever `set` is non-empty.

- [ ] **Step 3: Implement**

Track whether the webserver actually spawned:

```rust
let mut webserver_running = false;
#[cfg(feature = "webapp")]
if let Some(true) = config.serve && let Some(port) = config.port {
    webserver_running = true;
    // .. existing spawn
}
// ..
if !set.is_empty() {
    if webserver_running {
        println!("Other jobs are running (webserver or tui), press ctrl-c to quit (webserver)");
    }
    while let Some(res) = set.join_next().await { /* unchanged */ }
    return Ok(());
}
```

A TUI-only run prints nothing — the TUI owns the screen and documents its own quit key.

- [ ] **Step 4: Run the tests**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 5: Verify by hand**

Run: `cargo run -p cli -- --tui`, then quit with `q`.
Expected: nothing about a webserver on screen before or after.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add -A && git commit -m "fix(cli): only print the webserver hint when the webserver is running"
```

---

## Task 27: Documentation, final review, and finish the branch

**Files:**
- Modify: `README.md`, `CLAUDE.md`, `TODO.md`, `WHATS-NEXT.md`

- [ ] **Step 1: Update `README.md`**

The keybind table is already generated (Task 25). Add to the TUI feature bullet: week mode, raw-file view, jump-to-date, auto-refresh, yank summaries. Document both new config keys with their defaults.

- [ ] **Step 2: Update `CLAUDE.md`**

The module table lists `tui/` as "`app.rs` (state + event loop), `ui.rs` (rendering), custom widgets" — now stale. Replace with the real layout: `context.rs`, `theme.rs`, `keymap.rs`, `mode.rs`, `week_list.rs`, `project_list.rs`, plus `widgets/`. Add one line noting that TUI config is injected via `TuiContext` and never read from the `Config` singleton below `tui()`.

- [ ] **Step 2b: Make the feature-combo tests runnable (carried from Task 26's re-review)**

Task 26's regression test is gated `#[cfg(all(feature = "webapp", not(feature = "tui")))]`, which correctly targets the only vulnerable arm — but **nothing in the repo ever builds that combination.** The re-reviewer checked every path: `.github/workflows/{build,release}.yml` only run `cargo build`, the `justfile`'s `test:` recipe is a `watchexec` watch loop over default features, `.husky/pre-commit` runs only the frontend's tests, and plain `cargo test --workspace` is default-features. So a future regression of that drop-timing bug is caught by nothing automated.

Add a one-shot `justfile` recipe that runs the full gate including the feature combinations, e.g.:

```make
gate:
    SKIP_YARN=1 cargo clippy --all-targets --all-features -- -D warnings
    SKIP_YARN=1 cargo clippy -p cli --no-default-features --features tui --all-targets -- -D warnings
    SKIP_YARN=1 cargo clippy -p cli --no-default-features --features webapp --all-targets -- -D warnings
    cargo fmt --all -- --check
    SKIP_YARN=1 cargo test --workspace
    SKIP_YARN=1 cargo test -p cli --no-default-features --features webapp
```

**Do NOT add or modify a CI workflow.** That is an outward-facing change to what runs on the user's PRs and it is theirs to decide — surface it in the final summary instead. Note in the recipe's comment that the last line is the only thing exercising the webapp-only arm.

- [ ] **Step 3: Verify the whole suite and the lints**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

All four must pass. Additionally re-run the invariant greps:

```bash
grep -rn 'Config::get()' src/tui/          # expect: only src/tui/mod.rs
grep -rn 'unwrap()\|expect(' src/tui/      # expect: only #[cfg(test)] code
git status --porcelain cli/tests/fixtures/ # expect: empty
```

- [ ] **Step 4: Request a code review**

Use `superpowers:requesting-code-review`. Address anything Critical or Important; record Minor findings and decide explicitly.

- [ ] **Step 5: Strip the shipped items from `WHATS-NEXT.md`**

Only after review passes, and only naming items the branch actually shipped:

```bash
todo-parser WHATS-NEXT.md --strip W1 --strip W2 --strip W3 --strip W5 --strip W6 \
  --strip W7 --strip W8 --strip W9 --strip W10 --strip W11 --strip W12 --strip W13 \
  --strip W14 --strip W15 --strip W16 --strip W17 --strip W18 --strip W19 --strip W20 \
  --strip W21 --strip W23 --strip W24 --strip W25 --strip W26 --strip W27 --strip W28
```

W4, W22, W29 and W30 stay — they were not shipped.

- [ ] **Step 6: Commit the docs and the stripped findings file**

```bash
git add -A && git commit -m "docs: update README and CLAUDE.md for the TUI overhaul"
```

- [ ] **Step 7: Finish the branch**

Use `superpowers:finishing-a-development-branch`.
