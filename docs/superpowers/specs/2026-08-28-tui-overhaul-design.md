# TUI Overhaul: Substrate, Features, and Terminal Robustness

**Date:** 2026-08-28
**Branch:** `whats-next/2026-08-28`
**Scope:** 26 items selected from `WHATS-NEXT.md` (W1–W3, W5–W13, W14–W21, W23–W28), delivered as one branch in three phases with a single merge.
**Out of scope:** W4 + W22 (in-process write path / `a` append prompt), W29 (Ctrl-Z suspend), W30 (mouse support). These stay in `WHATS-NEXT.md`.

The TUI today is ~1,160 lines across `src/tui/`. This branch rewrites most of its plumbing, then layers user-facing features on the result. Phase 1 is behaviour-preserving except where noted; Phases 2 and 3 are additive.

---

## Goals

1. Make the TUI's state, config, and key dispatch injectable and testable — `src/tui/` currently has **zero tests**, and `App::new()` reaches into a `OnceLock` initialised by `Args::parse()`, so constructing an `App` outside the real binary parses argv.
2. Take idle cost to ~zero and make date scrubbing responsive.
3. Close the capability gap against the CLI and the SPA — dead time, warnings, weekly per-project rollups, arbitrary date jumps.
4. Make the tool legible outside a dark 24-bit 100×30 terminal.

## Non-goals

- No changes to the React SPA, GraphQL schema, or CLI output. `ttcli` stdout must be byte-identical before and after (see Invariants).
- No new write path to the user's markdown files. `$EDITOR` remains the only mutation route.
- No `insta`/snapshot-test dependency.

---

## Decisions taken during design

| Decision | Choice |
|---|---|
| Delivery | One branch, three phases, one merge |
| Async loads (W24) | Latest-wins **generation guard**; superseded results dropped, no `AbortHandle` |
| Mode dispatch (W20) | `mode: Mode` + `overlay: Option<Overlay>`, one overlay level |
| Theme (W21) | Three named presets + `NO_COLOR` / `COLORTERM` detection; **no** per-role override table |
| Chart scale (W8) | `daily_target_hours` config key (default 8) with a goal marker; ceiling = `max(target, week max)` |
| Tests | `TestBackend` buffer assertions + pure-logic units; no new dependency |
| Long lines (W28) | Folded into W13 — cached list body is width-keyed from the start |
| mtime watch (W5) | Dedicated 1 Hz task using W24's sender, **not** inside `tick()` |

New dependency: `unicode-width`. New config keys: `theme`, `daily_target_hours`.

---

## Architecture

### `TuiContext` (W19)

```rust
pub struct TuiContext {
    pub week_start_day: Weekday,
    pub data_dir: PathBuf,
    pub daily_target_hours: f64,
    pub formatter: Formatter,
    pub theme: Theme,
}
```

Built once in `tui()` from `Config::get()`, owned by `App`, passed by `&` into widget constructors. `App::new(ctx)` replaces the no-arg constructor. `TuiContext::for_test(..)` is the seam the render tests use.

This removes every in-render `Config::get()` — notably `weekly_bar_chart.rs:46`, which re-runs `parse_weekday` (a chain of case-insensitive string comparisons) and `get_week_dates` (heap-allocating a `Vec<Date>`) on **every frame** for a value that can only change when the active date changes (W12). `App` additionally stores `week_dates: [Date; 7]`, computed in the load path, which already derives exactly these values at `app.rs:188` and discards them.

The context is held behind `&mut self` so a later settings action can mutate it and trigger a reload.

### `Theme` (W21)

Replaces `WidgetColors`' associated consts, `project_list.rs:7-10`'s second private copy of the row backgrounds, and the inline `SLATE.c400` italic at `calendar.rs:45`. Roles: `populated_date`, `active_date`, `inactive_date`, `row_bg`, `alt_row_bg`, `list_header`, `selection`, `warning`, `error`, `goal_marker`, `status`.

Resolution order, highest priority first:

1. `NO_COLOR` present and non-empty in the env → `Preset::None`
2. `theme` key in `config.toml` (`"dark"` | `"light"` | `"none"`)
3. Default `"dark"`

Then, unless the preset is `None`: if `COLORTERM` is neither `truecolor` nor `24bit`, map every role to its 16-colour ANSI approximation. `Preset::None` emits no `fg` or `bg` at all — modifiers (bold, italic) only — so the terminal's own palette shows through.

`write_config_comments` gains commented examples for both new keys. `Config` already round-trips through `toml::to_string_pretty`, so optional fields are purely additive.

### `Mode` and `Overlay` (W20)

```rust
enum Mode    { Day, Week, ZoomedWeek, RawFile }
enum Overlay { Help, DatePrompt(String) }
```

Replaces `zoom_bar: bool` and `show_help: bool`. Key dispatch order is **overlay → mode → global bindings**; render order is mode, then overlay on top. This fixes the current bug where `handle_key_events` forwards every keypress to `project_list_widget` before matching app keys with no awareness of what is on screen — so today, with the help popup open, `j`/`k` still move the hidden list and `h`/`l` still change the date behind it, while `Esc` (the key users reflexively press to dismiss a modal) quits the application.

Exactly one overlay level. `DatePrompt(String)` owns its input buffer; a mode never sees a key an overlay consumed.

Both layers, and `ProjectListWidget`, return the same verdict type so the dispatcher can tell "I consumed this" from "I want the app to do something" from "not mine":

```rust
enum Handled { Consumed, Emit(AppEvent), Ignored }
```

`Ignored` falls through to the next layer; `Consumed` and `Emit` stop there. This is what lets `ProjectListWidget` hand its clipboard action up to `App` (W3) instead of performing I/O itself.

### Binding table (W25)

```rust
struct Binding {
    keys: &'static [(KeyCode, KeyModifiers)],
    event: AppEvent,
    modes: ModeMask,          // where this binding is live
    group: &'static str,      // "Date motion" | "View" | "Actions"
    description: &'static str,
}
const BINDINGS: &[Binding] = &[ .. ];
```

One source of truth for three consumers that have already drifted by four bindings: `handle_key_events`' match arm, `ProjectListWidget::handle_key_event`'s second match, `help_popup.rs:14-19`'s hardcoded string, and the README table at `README.md:192-199`. `HelpPopup` renders its rows from the table filtered by current mode; a small `xtask`-free unit test asserts the README table matches the generated one, so the doc cannot drift silently.

### Event loop and async loads (W24, W7, W5)

`EventHandler` exposes `sender() -> AppEventSender`, a wrapper over a clone of the existing private `mpsc::UnboundedSender<Event>`. Today `send` takes `&mut self` and the sender is private, so only `App` can emit an app event while it holds the loop — which makes a watcher task unimplementable.

Loads move off the event loop with a latest-wins generation guard:

```rust
self.load_gen += 1;
let gen = self.load_gen;
let tx = self.events.sender();
tokio::spawn(async move {
    let payload = load_for(date, ctx_snapshot).await;
    let _ = tx.send(AppEvent::DataLoaded(gen, Box::new(payload)));
});
```

On receipt, `gen != self.load_gen` means a newer load has started and the payload is dropped. Stale work runs to completion and is discarded — no abort machinery, so no half-populated cache to reason about. Failed loads become `AppEvent::LoadFailed(gen, String)` and surface on the status line.

Rendering becomes conditional: a `dirty: bool` on `App` is set at startup, after every handled key or app event, after a `DataLoaded` applies, and on `Event::Resize` (which is currently discarded at `app.rs:87`). `terminal.draw` runs only when it is set. `TICK_FPS` drops 30 → 4; the tick's only remaining job is expiring the status line (an expiry sets `dirty`, so the toast actually disappears). Today `run()` draws unconditionally at the top of every loop iteration, so the 30 FPS tick forces thirty full re-renders per second forever with no input and no animation.

The mtime watch (W5) is a **dedicated 1 Hz task** holding a sender clone, comparing `fs::metadata` mtime for the active date against a stored `SystemTime` and posting `ReloadFromDisk` on change. It is re-targeted when the active date changes. This closes the last unchecked TUI line in `TODO.md`.

### `DataService` (W23, W27, W18)

**W23 — cache parsed data.** `CacheEntry` gains `parsed: Option<TimeTrackingData>` alongside the raw `String`, keyed by date plus file mtime. `TimeTrackingData` already derives `Clone`, so cached copies are cheap to hand out. Today the cache stores only the file `String`, so a single arrow-key press re-runs `parse_time_tracking_data` ~97 times (≈90 populated-date probes + 7 weekly) **even on a full cache hit**. Existing invalidation (mtime comparison, `invalidate_date`) is unchanged.

**W27 — directory listing instead of per-date probes.** `find_populated_dates` currently walks every date in a ~90-day range, and each `check_date_has_data` → `parse_day` → `read_day` → `get_file_path` call re-resolves the home directory and stats (and, at `data_svc.rs:63`, *creates*) the data directory before the file's own `exists()`/`metadata()`. Replace with a single `read_dir` of the data directory, parsing `YYYY-MM-DD.md` filenames into dates, intersecting with the requested range, and parsing only files that actually exist. `get_file_path`'s directory-creation side effect is lifted into an explicit `ensure_data_dir()` called once at startup and by the write paths, not on every read.

Above that, `App` memoizes month population as `HashMap<(i32, u8), Vec<Date>>`, invalidated by `invalidate_date`, editor exit, and explicit `r`. A date change within the same displayed month no longer triggers a month rescan.

**W18 — weekly aggregation extraction.** The `week_projects: HashMap<String, (u32, Vec<String>)>` fold in `show_weekly_summary` (`src/display/mod.rs:189`) is interleaved with `println!` and `formatter.display_*` calls, so the TUI cannot reach it. Extract:

```rust
pub struct WeeklyProject { pub name: String, pub total_minutes: u32, pub notes: Vec<String> }
pub struct WeeklySummary {
    pub total_minutes: u32,
    pub dead_time_minutes: u32,
    pub projects: Vec<WeeklyProject>,   // sorted by name for determinism
    pub warnings: Vec<String>,
    pub per_day: HashMap<Date, u32>,
}
pub async fn get_weekly_summary(&self, dates: &[Date]) -> Result<WeeklySummary>;
```

`show_weekly_summary` becomes a formatter call over that struct; `get_weekly_data` becomes `summary.per_day`. **The characterization test in Phase 1 Task 1 is written and green before any of this moves** (see Invariants).

---

## Phase 1 — Substrate

Behaviour-preserving except where a fix is named. Ordered by dependency.

| # | Item | Deliverable |
|---|---|---|
| 1.1 | W18 (pre) | **Characterization test** pinning `ttcli --week` and `ttcli` stdout byte-for-byte, across all three formatters, on a fixture data directory |
| 1.2 | W19 | `TuiContext` + `Theme` struct (dark preset only, hardcoded — W21 adds resolution); `App::new(ctx)`; widget constructors take `&TuiContext`; `TuiContext::for_test` |
| 1.3 | W12 | `week_start_day` / `week_dates` hoisted onto `App`; `WeeklyBarChart::new(active_date, &week_dates, &theme)`; no `Config::get()` below `tui()` |
| 1.4 | W20 | `Mode` + `Overlay`; dispatch overlay → mode → global; render mode then overlay |
| 1.5 | W25 | `BINDINGS` table; `handle_key_events` and `HelpPopup` both driven by it |
| 1.6 | W24 | `AppEventSender`; `AppEvent::DataLoaded(gen, ..)` / `LoadFailed(gen, ..)`; loads spawned with generation guard |
| 1.7 | W7 | `dirty` flag; conditional `terminal.draw`; `Event::Resize` handled; `TICK_FPS` 30 → 4 |
| 1.8 | W13 + W28 | List body built once per (data, width); note bullets wrapped with hanging indent; name column padded by display width via `unicode-width` |
| 1.9 | W23 | Parsed `TimeTrackingData` in `CacheEntry` |
| 1.10 | W27 | `read_dir`-based populated-date scan; `ensure_data_dir()`; month memo on `App` |
| 1.11 | W18 | `WeeklySummary` extraction; `show_weekly_summary` reduced to a formatter call; characterization test still green |

## Phase 2 — Features

| # | Item | Deliverable |
|---|---|---|
| 2.1 | W3 | `App`-owned clipboard context + `status: Option<(String, Instant)>` in the footer, expired on `tick()`; `ProjectListWidget` emits `Handled::Emit(AppEvent::CopyToClipboard(String))` instead of constructing a `ClipboardContext` itself; fed by the copy path ("Copied 4 notes for admin" / "Clipboard unavailable"), the two `tracing::warn!` swallows at `app.rs:75,91`, and an in-flight "Loading…" marker |
| 2.2 | W9 | Weekday-qualified active date (`Thu 2026-08-27`) as a persistent title on the project-list pane. The block built at `ui.rs:51` is currently attached only to the "No data found" paragraph and dropped unused when data exists — the date string never reaches the screen. Removes the `.unwrap()` on `format` |
| 2.3 | W2 | Dead time (via `formatted_dead_time_minutes()` / `formatted_dead_decimal()`) and parser warnings in the day header, styled by the same sub-90-minute warn / 90-minute-plus error threshold `format_day_summary_impl` uses |
| 2.4 | W1 | `H`/`L` week motion, `[`/`]` and `PageUp`/`PageDown` month motion via the existing `month_offset` helper |
| 2.5 | W15 | `v` toggles `Mode::RawFile` — a scrollable `Paragraph` over `DataService::read_day`, the same read the SPA exposes as `fileContentForDate` |
| 2.6 | W16 | `:` opens `Overlay::DatePrompt`; the buffer is fed to `interim::parse_date_string(&s, now, Dialect::Us)` — the exact call `config.rs:301` makes for `ttcli --date 'last friday'`. `Enter` commits, `Esc` cancels, unparseable input reports on the status line and keeps the prompt open |
| 2.7 | W10 | `y` yanks the day summary via `ctx.formatter`'s non-printing `day_summary` variant; `Y` yanks the week summary over W18's `WeeklySummary` |
| 2.8 | W11 | Empty state names the day, distinguishes "no file yet" from "file exists but parses to no entries", prompts `e` / `t`, and keeps the `?` hint (which today lives inside `ProjectListWidget::render` and so vanishes exactly on the empty screen — W3's App-level footer already fixes the disappearance) |
| 2.9 | W17 | `w` enters `Mode::Week`: per-project week rollup with week total and week dead time, `Enter` yanks that project's week notes and hours |
| 2.10 | W5 | 1 Hz mtime watch task posting `ReloadFromDisk` |

## Phase 3 — Polish

| # | Item | Deliverable |
|---|---|---|
| 3.1 | W21 | `theme` config key; `NO_COLOR` / `COLORTERM` resolution; light and none presets |
| 3.2 | W8 | Chart ceiling = `max(daily_target_hours, week max rounded up)` with a goal marker row; `daily_target_hours` config key. The hand-rolled total-hours `Rect` at `weekly_bar_chart.rs:166` — computed from `area` rather than the block's inner area, and using `total_text.len()` (bytes) as a column count — is replaced by a right-aligned `Block::title_top` |
| 3.3 | W26 | Breakpoints: below ~100 columns drop the calendar and give the chart full width; below ~22 rows collapse the chart band so the list keeps usable height; below 60×15 render a centred "terminal too small — need at least 60×15" notice. Help popup sized by `Constraint::Length` clamping instead of a flat 60% square. On very wide terminals cap chart width and centre |
| 3.4 | W6 | Help popup is modal (consumes `Esc`/`q`/`?`/any key to close instead of falling through to `Quit`) and renders in every mode — today `ui.rs:13-21` returns before the `show_help` check, so `?` does nothing visible while `f` is active. Popup rows and the README table are both generated from `BINDINGS` |
| 3.5 | W14 | The "Other jobs are running (webserver or tui), press ctrl-c to quit (webserver)" line at `cli/src/main.rs:91` prints only when the webserver task was actually spawned. A TUI-only run prints nothing — it is currently written to stdout while the TUI concurrently enters the alternate screen, so it either corrupts the first frame or lurks on the normal screen, and it teaches `ctrl-c` when `q` is the quit key |

---

## Canonical keymap

Generated from `BINDINGS`; the help popup and the README table are both rendered from this table.

```
DATE MOTION
  h  l   ← →        ± 1 day
  H  L              ± 1 week
  [  ]   PgUp/PgDn  ± 1 month
  t                 today
  :                 jump to date  ("last friday", "2026-08-14")

VIEW
  j  k   ↓ ↑        select project
  g  G              first / last project
  w                 week mode (per-project rollup)
  v                 raw file text
  f                 zoom weekly chart
  ?                 help
  Esc               close overlay  (quits only when none is open)

ACTIONS
  Enter             copy selected project's notes
  y                 copy day summary        Y  copy week summary
  e                 edit in $EDITOR
  r                 reload from disk
  q  Ctrl-C         quit
```

## Config additions

```toml
# Theme preset: "dark" | "light" | "none".
# "none" emits no colors so your terminal palette shows through.
# NO_COLOR in the environment forces "none".
theme = "dark"

# Target hours per day. Draws a goal marker on the weekly chart and
# sets the chart's minimum full-scale value.
daily_target_hours = 8
```

## Error handling

- Load failures become `AppEvent::LoadFailed(gen, msg)` → status line + `tracing::warn!`. Today they reach only the rolling log file the user cannot see, because the alternate screen owns the terminal, and a failed load renders identically to an empty day.
- Clipboard failures report "Clipboard unavailable" on the status line. On a headless box or over SSH with no clipboard backend, `Enter` currently does nothing at all with no indication.
- Unparseable date-prompt input keeps the overlay open with the error on the status line.
- A terminal below the hard minimum renders the notice rather than a mangled frame.
- No new `unwrap`/`expect` in `src/tui/`. The existing `.unwrap()` on `active_date.format` is removed by 2.2.

## Testing

`TuiContext::for_test` + `ratatui::backend::TestBackend`, no new dependency.

**Render assertions** — day view shows the weekday-qualified date; dead-time line and warnings appear when present; the empty state shows the CTA and the `?` hint; help renders over the zoomed chart; a 50×12 terminal shows the too-small notice; an 80×24 terminal drops the calendar; `Preset::None` writes no colour to the buffer.

**Pure-logic units** — bar max scaling against target and week max; breakpoint selection per (width, height); binding lookup per mode, and no duplicate key within a mode; month-memo hit and invalidation; load-generation guard drops stale payloads; theme resolution across `NO_COLOR` × `COLORTERM` × config; `unicode-width` padding for CJK and emoji names; note wrapping with hanging indent at several widths.

**Characterization** — `ttcli` and `ttcli --week` stdout, all three formatters, byte-for-byte (Phase 1 Task 1).

**Dispatch regression** — with `Overlay::Help` open, `j`/`k`/`h`/`l` change nothing; `Esc` closes the overlay and does **not** quit.

---

## Invariants this feature depends on

Recorded so a later change touching these funnels can grep for who relies on them.

1. **`ttcli` stdout is unchanged by the W18 extraction.** W18's own note claims "the CLI path keeps identical output because the formatter calls stay in `display/mod.rs`". That is an appeal to a current invariant, not a test — and it is exactly the load-bearing case. Task 1.1 writes the characterization test **first**, covering `ttcli` and `ttcli --week` across the default, plain, and markdown formatters against a fixture directory. No aggregation moves until it is green.
2. **`TimeTrackingData: Clone`** — W23's parse cache hands out cached copies. Provided by `time-tracking-parser` (`#[derive(Clone, ..)]` on `TimeTrackingData`). If that derive is ever dropped, the cache must move to `Arc<TimeTrackingData>`.
3. **`interim` is an unconditional dependency**, not feature-gated, so W16's prompt can call `parse_date_string` from `src/tui/` directly.
4. **`Config` is a process-lifetime `OnceLock`** — `TuiContext` snapshots it once at startup. Correct today; if config ever becomes reloadable, the context needs a refresh path (it is already `&mut`-reachable for that reason).
5. **`DisplayFormatter` has non-printing String-returning variants** (`day_summary`, `weekly_projects`, `weekly_totals`) next to every `display_*` method. W10's yank depends on them. If a formatter is added, it must implement both halves.
6. **`get_file_path`'s directory-creation side effect** is relied on by the current write paths. W27 lifts it into `ensure_data_dir()`; every caller that previously depended on the implicit creation (`create_day_file_if_not_exists`, the GraphQL mutation) must call it explicitly.

## Risks

- **W18 changing CLI output.** Mitigated by ordering: characterization test first, extraction second.
- **W27 changing which dates count as populated.** `read_dir` tells you a file *exists*; "populated" still means `!projects.is_empty() && total_minutes > 0`, so the listing only narrows the candidate set. A test asserts an existing-but-empty file is not reported as populated.
- **W7's dirty flag missing a redraw trigger** — a state change with no `dirty = true` renders as a frozen UI. Mitigated by setting it centrally in the event-dispatch path rather than at each call site, plus the `Event::Resize` arm.
- **Phase 1 is a large behaviour-preserving refactor with no user-visible output**, so regressions hide until Phase 2. Mitigated by building the render tests in 1.2 and extending them through the phase.
