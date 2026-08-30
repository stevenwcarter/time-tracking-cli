# TUI mouse support and Ctrl-Z suspend

Date: 2026-08-29
Source: `WHATS-NEXT.md` items W29 and W30, bundled by `/whats-next --execute`.

## Problem

Two gaps sit on the same seam — the code that owns which terminal modes the
TUI has turned on.

**W29.** The TUI enables raw mode, so the terminal never generates `SIGTSTP`
and Ctrl-Z is swallowed. There is no way to drop to the shell and `fg` back;
the user must quit and relaunch, losing the selected date. Job control is
reflexive for anyone running this from a shell all day.

**W30.** Mouse capture is never enabled and `App::run` (`src/tui/app.rs:491`)
matches only `Event::Key`, discarding everything else. Clicks and wheel
scrolls do nothing. The calendar is the most click-inviting thing on screen
and does not respond.

The two are one change because mouse capture is a terminal mode that must be
turned off and back on in exactly the same places the alternate screen is —
otherwise an `$EDITOR` session inherits capture. Today those places are
scattered across three functions.

## Current state

Terminal-mode handling lives in four places:

| Site | What it does |
|------|--------------|
| `src/tui/mod.rs:23` | `ratatui::init()` — raw mode, alternate screen, panic hook |
| `src/tui/mod.rs:28` | `ratatui::restore()` |
| `src/tui/app.rs:1545` `edit_date` | `LeaveAlternateScreen`, `disable_raw_mode` |
| `src/tui/app.rs:1561` `restore_terminal` | `EnterAlternateScreen`, `enable_raw_mode`, `terminal.clear()` |

Plus `PausedPoller` (`src/tui/app.rs:1522`), a `Drop` guard that pauses the
event poller across the editor session. That guard's rationale — a missed
resume hangs rather than errors, so the pairing must be structural — applies
verbatim to suspension, which is why the new helper absorbs it rather than
sitting beside it.

## Decisions

These were settled during brainstorming and are not open questions.

1. **Mouse on by default, with a config off-switch.** A new `mouse` key
   (default `true`) joins `theme` and `daily_target_hours`. Mouse capture
   costs the terminal's native click-drag text selection (Shift overrides it
   in most emulators), so an escape hatch is required; defaulting it off
   would make the feature invisible and defeat W30's rationale.
2. **Calendar hit-testing replicates `Monthly`'s geometry, pinned by a
   characterization test.** Rather than rewriting the calendar widget.
3. **Ctrl-Z appears once in `BINDINGS`, labelled "(Unix only)".** Only the
   handler is `cfg`-gated. This keeps `BINDINGS`, the help popup, the
   generated README table, and the test that compares it against the file on
   disk identical on every platform.
4. **Included beyond the core set:** double-click to open `$EDITOR`,
   click-outside-an-overlay to dismiss, clickable footer help hint.
   **Excluded:** drag-to-select a date range — nothing consumes a date range
   today, so it would build a gesture with no destination.

## Design

### 1. `src/tui/terminal.rs` — new module

The single owner of which terminal modes the TUI has on.

```rust
/// Every terminal mode the TUI turns on beyond the defaults.
#[derive(Clone, Copy, Debug)]
pub struct TerminalModes {
    pub mouse: bool,
}

impl TerminalModes {
    /// Alternate screen, raw mode, and mouse capture when enabled.
    fn enter(self) -> Result<()>;
    /// The exact inverse, innermost mode first.
    fn leave(self) -> Result<()>;
}

/// Hand the terminal back to the shell in cooked mode for the duration of
/// `body`, then take it back.
pub async fn with_suspended_terminal<B, T, F, Fut>(
    events: &mut EventHandler,
    terminal: &mut Terminal<B>,
    modes: TerminalModes,
    body: F,
) -> Result<T>
where
    B: Backend,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T>>;
```

Sequence: pause poller (via the moved `PausedPoller` guard) → `modes.leave()`
→ `body().await` → `modes.enter()` + `terminal.clear()` → guard drops,
poller resumes.

**The restore must run even when `body` returns `Err`.** Today's
`edited.and(restored)` shape (`app.rs:922`) already does this and is
preserved: the body's error is reported, but the terminal is put back either
way. A body error that skipped restore would leave the user in a cooked
terminal with a running TUI.

`PausedPoller` moves here from `app.rs` unchanged, including its doc comment.

Callers, both of which pass `TerminalModes` from `TuiContext`:

- `App::run_editor` — body is the existing `edit_date`.
- `App::suspend` (new) — body raises `SIGTSTP`.

### 2. Ctrl-Z

```rust
#[cfg(unix)]
async fn suspend<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()>
```

The body is `unsafe { libc::raise(libc::SIGTSTP) }`. Rust installs no
`SIGTSTP` handler, so the default action stops the process; `raise` returns
only once `SIGCONT` arrives, so the code after it *is* the resume path. No
`SIGCONT` handler is needed and none is installed.

On resume, after `modes.enter()` and `terminal.clear()`, the handler sends
`AppEvent::ReloadFromDisk(Reload::Rescan)` — the shell may have changed day
files while suspended. This mirrors what the editor path already does at
`app.rs:563`.

On Windows the handler is replaced by one that sets the status line to say
suspend is unavailable there. The binding itself is unconditional.

`libc` moves from a `cfg(unix)` **dev**-dependency to a `cfg(unix)`
dependency in the workspace `Cargo.toml`. It is declared unconditionally for
Unix rather than as an optional feature dependency, because a `dep:libc`
reference inside the `tui` feature would name a dependency that does not
exist on Windows. `just gate`'s isolation assertions check for `axum` and
`ratatui` specifically and are unaffected.

### 3. Startup, teardown, and the panic hook

`ratatui::init()` installs a panic hook that disables raw mode and leaves the
alternate screen. **It knows nothing about mouse capture.** Left alone, a
panic would return the user to a shell that emits escape sequences on every
drag.

`tui()` therefore:

1. calls `ratatui::init()` as today, keeping its hook and its terminal;
2. if `ctx.mouse`, executes `EnableMouseCapture` and installs a chained panic
   hook that executes `DisableMouseCapture` and then delegates to the hook
   ratatui installed;
3. on the way out, executes `DisableMouseCapture` before `ratatui::restore()`.

Ordering matters: our hook is installed *after* ratatui's, so it runs
*first*, disabling capture while still on the alternate screen.

### 4. Config and context

`Config` gains `pub mouse: Option<bool>`, defaulting to `Some(true)` in
`Config::default()`, with a commented line in the generated config file
alongside `theme` and `daily_target_hours` (`src/config.rs:429-434`).

`TuiContext` gains `pub mouse: bool`, resolved once in `from_config` via
`config.mouse.unwrap_or(true)`. Per the standing rule, no TUI code reads
`Config::get()` directly — only `tui()` does.

### 5. Layout capture

`App` gains one field:

```rust
/// Where each clickable region was drawn on the most recent frame.
///
/// Populated during render and read by mouse hit-testing, which can only
/// run after a frame has been drawn.
pub struct LayoutRects {
    pub calendar: Option<Rect>,
    pub bar_chart: Option<Rect>,
    pub project_list: Option<Rect>,
    pub week_list: Option<Rect>,
    pub raw_file: Option<Rect>,
    pub help_hint: Option<Rect>,
    pub overlay: Option<Rect>,
}
```

`impl Widget for &mut App` already takes `&mut self`, so the render methods
in `ui.rs` can record their rects with no new plumbing. Every field is reset
to `None` at the top of `render`, so a region not drawn this frame cannot be
hit — this is what makes clicks on a `TooSmall` terminal, or on a band that
was dropped because it was unaffordable, do nothing.

### 6. Hit-testing

Each widget owns the geometry it draws with.

**`Calendar::date_at(area, x, y) -> Option<Date>`.** Replicates
`Monthly`'s layout: the block (`Borders::RIGHT`, padding left 1 / top 1),
then one month-header row, one weekday-header row, then week rows where
weekday `i` occupies `x + 1 + 3i .. x + 3 + 3i`.

**`Monthly` always starts weeks on Sunday** (it uses
`number_days_from_sunday` internally) regardless of the app's
`week_start_day` config. The hit-test must use Sunday. Using
`ctx.week_start_day` here would be wrong on every non-Sunday configuration,
which is the single most likely bug in this feature.

**`WeeklyBarChart::date_at(area, x, y) -> Option<Date>`.** Reuses the
existing `calculate_bar_dimensions` (`weekly_bar_chart.rs:163`) so render and
hit-test read from one formula rather than two copies that can drift.

**`ProjectListWidget::index_at(area, y) -> Option<usize>`.** Rows are
**variable-height**: `render_list` builds each item with
`clamp_item_rows(project_item.body(body_width), viewport_rows)`
(`project_list.rs:504`). So a click maps to an index by walking heights from
`ListState::offset()`, applying the same clamp. `body_rows(width)` already
exists (`project_list.rs:150`) and is the height source for both paths.

**`week_list`** rows are single-height, so its index is
`offset + (y - inner.y)`.

### 7. Event handling

`App::run` gains one arm:

```rust
Event::Crossterm(CrosstermEvent::Mouse(mouse_event)) => {
    self.handle_mouse_event(mouse_event)?;
}
```

placed before the existing catch-all `Event::Crossterm(_) => {}`.

Only three kinds are acted on: `Down(Left)`, `ScrollUp`, `ScrollDown`.
Everything else — including `Moved` and `Drag` — is ignored. crossterm's
`EnableMouseCapture` uses mode 1002 (button-event tracking), so an idle
mouse generates no events at all and the loop is not woken by cursor motion.

Dispatch is **overlay-first**, matching the existing rule in `mode.rs` about
who gets first refusal on input: while an overlay is open, a click inside it
is ignored and a click outside dismisses it. Nothing falls through to a
widget behind a modal.

Then by mode:

| Mode | Target | Action |
|------|--------|--------|
| Day | calendar day | `go_to_date` |
| Day | weekly bar | `go_to_date` |
| Day | project row | select it |
| Week | rollup row | select it |
| Any | footer help hint | `ToggleHelp` |
| Day/Week/RawFile | wheel | move selection / scroll raw file |

Handlers call `go_to_date` directly rather than adding a date-carrying
`AppEvent`. This follows the date prompt's precedent (`app.rs:884`), and
avoids widening the deliberately `_`-arm-free matches in `handle_app_event`
and `apply_sync_event`.

**Wheel scrolling moves the selection**, reusing `NextProject` /
`PreviousProject` and their week-view counterparts, rather than scrolling the
viewport independently — `ListState` has no detached scroll offset, so an
independent scroll would desynchronise the selection from what is visible.

**Double-click** opens `$EDITOR` on the clicked day, reusing `AppEvent::Edit`
after a `go_to_date`. Detection is a `Option<(Instant, u16, u16)>` on `App`:
a second `Down(Left)` within 400 ms at the same cell counts as a double.
`Instant` is already imported for the status-line TTL.

## Testing

The seam is testable without a tty: render into a `TestBackend`, which
populates `LayoutRects`, then feed synthetic `MouseEvent`s to
`handle_mouse_event`.

1. **Calendar characterization test.** Render a real `Monthly` into a
   `TestBackend`, then for every cell in the calendar area assert that when
   `date_at` returns a date, the digits drawn at that cell are that date's
   day number. This is what turns "we replicated ratatui's geometry" into a
   falsifiable claim; if ratatui changes `Monthly`'s layout the test fails
   loudly instead of the app silently jumping to the wrong day.
2. **Sunday-start test.** The same hit-test under a non-Sunday
   `week_start_day` context still resolves the column the digits are drawn
   in — pinning decision 2's trap.
3. **Variable-height row test.** A day whose projects have differing note
   counts; assert `index_at` agrees with the highlighted row after a click,
   including at a non-zero `ListState::offset()`.
4. **Bar chart test.** A click in each of the seven bars resolves to that
   day, at more than one terminal width so the shared
   `calculate_bar_dimensions` is exercised rather than a hard-coded width.
5. **Suspend pause/resume test.** Via `drain_pause_signals`, assert the
   suspend path emits `[true, false]` — mirroring the existing editor
   regression test at `app.rs:3227`, which exists because a paused poller
   hangs rather than errors.
6. **Mode symmetry test.** `TerminalModes::leave` undoes exactly what
   `enter` does, for both `mouse: true` and `mouse: false`.
7. **Overlay-first test.** With the help popup open, a click on the calendar
   dismisses the popup and does *not* change `active_date`.
8. **Config round-trip tests** for `mouse`, matching the existing
   `daily_target_hours` tests at `config.rs:491-506`.
9. **Keymap/README test** — the existing generated-table test must stay
   green with the new Ctrl-Z row, on every platform.

## Invariants this feature depends on

Recorded so a later change touching any of them can find who relies on it.

- **`Monthly` lays weeks out Sunday-first, one month-header row, one
  weekday-header row, 3 columns per day.** Pinned by test 1.
- **`ListState::offset()` reflects the most recent render.** Hit-testing runs
  after a draw, so the offset is current. Pinned by test 3.
- **`ratatui::init()`'s panic hook does not know about mouse capture.** If a
  future ratatui adds mouse handling to its hook, the chained hook here
  becomes redundant but not harmful.
- **crossterm's `EnableMouseCapture` uses mode 1002, not 1003**, so idle
  motion generates no events. If this changed, the event loop would be woken
  on every cursor move.
- **`App::run`'s `Event::Crossterm(_) => {}` catch-all** silently discards
  unmatched events; the new `Mouse` arm must precede it.

## Out of scope

- Drag-to-select a date range (no consumer today).
- Mouse support in the CLI or web surfaces.
- A runtime toggle for mouse capture — the config key is the escape hatch.
- Any change to `DataService` or the write path (that is W4/W22, skipped).
