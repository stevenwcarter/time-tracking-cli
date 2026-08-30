# TUI Mouse Support and Ctrl-Z Suspend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the TUI mouse support (click a calendar day, a weekly bar, or a list row; wheel-scroll) and a working Ctrl-Z suspend, both built on one module that owns which terminal modes are on.

**Architecture:** A new `src/tui/terminal.rs` becomes the single owner of the terminal mode set (alternate screen, raw mode, mouse capture) and exposes `with_suspended_terminal`, which both the `$EDITOR` handover and the new Ctrl-Z handler run their bodies inside. Mouse hit-testing lives on the widgets that draw the geometry, reading rects that `App` records during render into a `LayoutRects` struct.

**Tech Stack:** Rust (edition 2024), ratatui 0.29, crossterm (via `ratatui::crossterm`), tokio, `libc` for `SIGTSTP`. All work is behind the `tui` cargo feature.

**Spec:** `docs/superpowers/specs/2026-08-29-tui-mouse-and-suspend-design.md`

## Global Constraints

- **Edition 2024.** Every crate in this workspace is `edition = "2024"` and `rustfmt.toml` matches. Do not change either.
- **Verification gate is `just gate`, not `cargo test`.** It runs check/test/clippy `-D warnings`/`fmt --check` across the default, `tui`-only, and `webapp`-only feature combinations. It needs `site/build/` to exist first (`cd site && yarn install && yarn build`). Per-task steps may run a narrower `cargo test`, but the branch is not done until `just gate` is green.
- **Never smoke-run the binary bare.** A plain `cargo run -p cli --` opens `$EDITOR` on the real `~/.time-tracking/`. Always pass `--noedit --data-directory <tmp>`.
- **All new code is `tui`-feature-gated.** `src/tui/` is already behind `#[cfg(feature = "tui")]`; the one exception is the `mouse` field on `Config` in `src/config.rs`, which is unconditional like `theme` and `daily_target_hours`.
- **TUI code must never call `Config::get()`.** It reads config through `TuiContext`. Only `tui()` in `src/tui/mod.rs` touches `Config`.
- **`handle_app_event` and `apply_sync_event` have deliberately no `_` arm.** Adding an `AppEvent` variant means adding it to the correct arm in both. Do not add a catch-all.
- **Conventional commits** (`feat:`, `fix:`, `test:`, `docs:`, `chore:`) — enforced by Husky + commitlint.
- **`Monthly` always lays weeks out Sunday-first**, regardless of the app's `week_start_day`. Any calendar hit-test uses Sunday.

---

## File Structure

| File | Change | Responsibility |
|------|--------|----------------|
| `src/tui/terminal.rs` | **Create** | `TerminalModes`, `PausedPoller` (moved), `with_suspended_terminal` |
| `src/tui/layout_rects.rs` | **Create** | `LayoutRects` — where each clickable region was drawn |
| `src/tui/mod.rs` | Modify | Register both modules; mouse capture + chained panic hook in `tui()` |
| `src/config.rs` | Modify | `mouse: Option<bool>` field, default, generated-config line, tests |
| `src/tui/context.rs` | Modify | `mouse: bool` resolved from config |
| `src/tui/app.rs` | Modify | Remove moved items; `AppEvent::Suspend` handling; `handle_mouse_event`; `layout` field |
| `src/tui/event.rs` | Modify | `AppEvent::Suspend` variant |
| `src/tui/keymap.rs` | Modify | Ctrl-Z binding row |
| `src/tui/ui.rs` | Modify | Record rects during render |
| `src/tui/widgets/calendar.rs` | Modify | `Calendar::date_at` |
| `src/tui/widgets/weekly_bar_chart.rs` | Modify | `WeeklyBarChart::date_at` |
| `src/tui/widgets/help_popup.rs` | Modify | `pub` popup rect accessor |
| `src/tui/widgets/date_prompt.rs` | Modify | `pub` popup rect accessor |
| `src/tui/project_list.rs` | Modify | `index_at`, `select_index` |
| `src/tui/week_list.rs` | Modify | `index_at`, `select_index` |
| `Cargo.toml` | Modify | `libc` from dev-dependency to dependency under `cfg(unix)` |
| `README.md` | Modify | Regenerated keybind table; mouse section |
| `CLAUDE.md` | Modify | New `tui/` submodule rows |

---

## Task 1: The terminal-modes module

Extracts the scattered terminal-mode handling into one place and rewires the editor handover through it. No behaviour change yet — this is the seam both later features hang off.

**Files:**
- Create: `src/tui/terminal.rs`
- Modify: `src/tui/mod.rs` (register module)
- Modify: `src/tui/app.rs` — remove `PausedPoller` (1522-1534), `restore_terminal` (1561-1566), the `LeaveAlternateScreen`/`disable_raw_mode` lines from `edit_date` (1545-1547), and rewrite `run_editor` (914-923)

**Interfaces:**
- Consumes: `EventHandler::pause`/`resume` (`src/tui/event.rs`), `ratatui::Terminal`, `ratatui::backend::Backend`
- Produces:
  - `pub struct TerminalModes { pub mouse: bool }` — `Clone + Copy + Debug + PartialEq + Eq`
  - `TerminalModes::enter(self) -> anyhow::Result<()>`
  - `TerminalModes::leave(self) -> anyhow::Result<()>`
  - `pub async fn with_suspended_terminal<B, T, F, Fut>(events: &mut EventHandler, terminal: &mut Terminal<B>, modes: TerminalModes, body: F) -> anyhow::Result<T>` where `B: Backend`, `F: FnOnce() -> Fut`, `Fut: Future<Output = anyhow::Result<T>>`

- [ ] **Step 1: Write the failing test**

Create `src/tui/terminal.rs` with only the test module, so the test names the API before it exists:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::event::EventHandler;
    use ratatui::{Terminal, backend::TestBackend};

    /// The helper must pause the poller on the way in and resume it on the
    /// way out. A missed resume does not error — it hangs — so this is the
    /// only way the pairing is checked. Mirrors the existing editor
    /// regression test in `app.rs`.
    #[tokio::test]
    async fn suspending_pauses_and_resumes_the_poller() {
        let mut events = EventHandler::new();
        let mut terminal = Terminal::new(TestBackend::new(20, 10)).expect("test backend");
        let modes = TerminalModes { mouse: false };

        with_suspended_terminal(&mut events, &mut terminal, modes, || async { Ok(()) })
            .await
            .expect("suspend");

        assert_eq!(events.drain_pause_signals(), vec![true, false]);
    }

    /// A body that fails must still leave the terminal restored and the
    /// poller resumed: the error is reported, the terminal is put back.
    #[tokio::test]
    async fn a_failing_body_still_resumes_the_poller() {
        let mut events = EventHandler::new();
        let mut terminal = Terminal::new(TestBackend::new(20, 10)).expect("test backend");
        let modes = TerminalModes { mouse: false };

        let result: anyhow::Result<()> =
            with_suspended_terminal(&mut events, &mut terminal, modes, || async {
                Err(anyhow::anyhow!("body failed"))
            })
            .await;

        assert!(result.is_err(), "the body's error must be reported");
        assert_eq!(events.drain_pause_signals(), vec![true, false]);
    }

    #[test]
    fn modes_are_copy_and_compare_by_value() {
        let a = TerminalModes { mouse: true };
        let b = a;
        assert_eq!(a, b);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --no-default-features --features tui tui::terminal`
Expected: FAIL — `cannot find function with_suspended_terminal`, `cannot find struct TerminalModes`.

- [ ] **Step 3: Write the module**

Put this above the test module in `src/tui/terminal.rs`:

```rust
//! The one place that knows which terminal modes the TUI has turned on.
//!
//! Before this module the mode set lived in four places — `tui()`'s
//! `ratatui::init`/`restore` pair and the `$EDITOR` handover's own
//! leave/enter pair — and the two were only equal by inspection. Mouse
//! capture is the mode that made that untenable: a mode the editor handover
//! forgot to drop is one the editor session inherits.

use std::future::Future;
use std::io::stdout;

use anyhow::Result;
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::crossterm::ExecutableCommand;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use super::event::EventHandler;

/// Every terminal mode the TUI turns on beyond the terminal's defaults.
///
/// Held by value and passed to [`with_suspended_terminal`] rather than read
/// from a global, so the suspend path cannot drift from what start-up
/// actually enabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalModes {
    /// Is mouse capture on? Governed by the `mouse` config key.
    pub mouse: bool,
}

impl TerminalModes {
    /// Take the terminal: alternate screen, raw mode, and mouse capture when
    /// it is enabled.
    pub fn enter(self) -> Result<()> {
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        if self.mouse {
            stdout().execute(EnableMouseCapture)?;
        }
        Ok(())
    }

    /// Give the terminal back, innermost mode first — the exact inverse of
    /// [`TerminalModes::enter`].
    pub fn leave(self) -> Result<()> {
        if self.mouse {
            stdout().execute(DisableMouseCapture)?;
        }
        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;
        Ok(())
    }
}

/// Holds the event poller paused for as long as it lives, and resumes it on
/// the way out — including the way out an `?` takes.
///
/// A `Drop` guard rather than a matching `resume()` call at the bottom of
/// [`with_suspended_terminal`] because the failure mode is invisible: a
/// paused poller emits nothing at all, so a missed resume does not error, it
/// hangs. The pairing has to be structural, not remembered.
struct PausedPoller<'a>(&'a mut EventHandler);

impl<'a> PausedPoller<'a> {
    /// Pause `events` until the returned guard is dropped.
    fn new(events: &'a mut EventHandler) -> Self {
        events.pause();
        Self(events)
    }
}

impl Drop for PausedPoller<'_> {
    fn drop(&mut self) {
        self.0.resume();
    }
}

/// Hand the terminal back to the shell in cooked mode, run `body`, then take
/// the terminal back and redraw from scratch.
///
/// The poller is paused throughout: crossterm and whatever `body` hands the
/// terminal to must not both be reading stdin.
///
/// **`body`'s error does not skip the restore.** The result is combined the
/// way the editor handover always did it — the body's error is what the
/// caller sees, but the terminal is put back either way. Returning early on
/// a body error would leave a cooked terminal under a running TUI.
pub async fn with_suspended_terminal<B, T, F, Fut>(
    events: &mut EventHandler,
    terminal: &mut Terminal<B>,
    modes: TerminalModes,
    body: F,
) -> Result<T>
where
    B: Backend,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let _paused = PausedPoller::new(events);

    let left = modes.leave();
    let out = body().await;
    let restored = left.and_then(|()| {
        modes.enter()?;
        // Whatever ran while we were away wrote over the alternate screen.
        terminal.clear()?;
        Ok(())
    });

    match restored {
        Ok(()) => out,
        Err(e) => out.and(Err(e)),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --no-default-features --features tui tui::terminal`
Expected: PASS (3 tests).

- [ ] **Step 5: Register the module**

In `src/tui/mod.rs`, add to the module list, alphabetically after `pub mod project_list;`:

```rust
pub mod terminal;
```

- [ ] **Step 6: Rewire `run_editor` and delete the moved code**

In `src/tui/app.rs`:

1. Delete `struct PausedPoller` and its two impls (lines 1515-1534, including the doc comment above the struct).
2. Delete `fn restore_terminal` and its doc comment (lines 1558-1566).
3. In `edit_date` (line 1545), delete the first two statements — `stdout().execute(LeaveAlternateScreen)?;` and `disable_raw_mode()?;` — and shorten its doc comment's first line to `/// Open `$EDITOR` on `date`'s file — creating it if it does not exist — and`. The helper owns the terminal now; `edit_date` only edits.
4. Replace `run_editor`'s body:

```rust
    pub async fn run_editor<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        // Destructured rather than borrowed through `self` twice: the helper
        // takes `&mut self.events` and the body captures `&self.data_svc`,
        // which the borrow checker only sees as disjoint when the two are
        // named separately.
        let Self {
            events,
            data_svc,
            active_date,
            ctx,
            ..
        } = self;
        let date = *active_date;
        let modes = TerminalModes { mouse: ctx.mouse };
        with_suspended_terminal(events, terminal, modes, || edit_date(data_svc, date)).await
    }
```

5. Fix the imports at the top of `app.rs`: drop `EnterAlternateScreen`, `LeaveAlternateScreen`, `disable_raw_mode`, `enable_raw_mode` from the `terminal::{...}` import on line 29 (keep the import only if something else still uses it — if the braces empty out, delete the whole line), drop `stdout` if now unused, and add:

```rust
use super::terminal::{TerminalModes, with_suspended_terminal};
```

**Note:** `ctx.mouse` does not exist until Task 2. Until then, hard-code `TerminalModes { mouse: false }` here and change it in Task 2 — do **not** invent the field early.

- [ ] **Step 7: Run the full tui suite**

Run: `cargo test --no-default-features --features tui`
Expected: PASS. In particular `app::tests` around line 3227 (the editor pause/resume regression test) must still pass — it drives `PausedPoller` through the editor path.

- [ ] **Step 8: Clippy and fmt**

Run: `cargo clippy --no-default-features --features tui --all-targets -- -D warnings && cargo fmt --all`
Expected: clean.

- [ ] **Step 9: Commit**

```bash
git add src/tui/terminal.rs src/tui/mod.rs src/tui/app.rs
git commit -m "refactor(tui): own the terminal mode set in one module"
```

---

## Task 2: The `mouse` config key, and mouse capture at start-up

**Files:**
- Modify: `src/config.rs` — field near `daily_target_hours` (line 151), default (line 196), generated-config writer (lines 429-434), tests (near line 491)
- Modify: `src/tui/context.rs` — `mouse` field and resolution
- Modify: `src/tui/mod.rs` — `tui()` enables capture and chains a panic hook
- Modify: `src/tui/app.rs` — use `ctx.mouse` in `run_editor`

**Interfaces:**
- Consumes: `TerminalModes` (Task 1)
- Produces: `Config::mouse: Option<bool>`, `TuiContext::mouse: bool`

- [ ] **Step 1: Write the failing tests**

In `src/config.rs`'s test module, beside the `daily_target_hours` tests:

```rust
    #[test]
    fn test_mouse_defaults_to_true() {
        let config = Config::default();
        assert_eq!(config.mouse, Some(true));
    }

    #[test]
    fn test_mouse_roundtrip() {
        let config = Config {
            mouse: Some(false),
            ..Config::default()
        };
        let toml_str = toml::to_string(&config).expect("serialize");
        assert!(toml_str.contains("mouse = false"));
        let deserialized: Config = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(deserialized.mouse, Some(false));
    }

    #[test]
    fn test_mouse_missing_key_deserializes_to_none() {
        let config: Config = toml::from_str("").expect("deserialize");
        assert_eq!(config.mouse, None);
    }
```

In `src/tui/context.rs`'s test module:

```rust
    /// An unset key means capture is on: the feature has to be discoverable
    /// without reading the config file.
    #[test]
    fn mouse_defaults_on_when_the_key_is_absent() {
        let config = Config {
            mouse: None,
            ..Config::default()
        };
        let ctx = TuiContext::from_config(&config).expect("context");
        assert!(ctx.mouse);
    }

    #[test]
    fn mouse_can_be_turned_off_in_config() {
        let config = Config {
            mouse: Some(false),
            ..Config::default()
        };
        let ctx = TuiContext::from_config(&config).expect("context");
        assert!(!ctx.mouse);
    }
```

If `src/tui/context.rs` has no test module, add one:

```rust
#[cfg(test)]
mod tests {
    use super::*;
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --no-default-features --features tui mouse`
Expected: FAIL — `Config` has no field `mouse`, `TuiContext` has no field `mouse`.

- [ ] **Step 3: Add the config field**

In `src/config.rs`, after `daily_target_hours` (line 151):

```rust
    /// Whether the TUI captures the mouse, enabling clicks and wheel
    /// scrolling. Defaults to true. Turning it off restores the terminal's
    /// own click-drag text selection, which capture otherwise takes over
    /// (most emulators leave it on Shift-drag).
    pub mouse: Option<bool>,
```

In `Config::default()` (after line 196):

```rust
            mouse: Some(true),
```

In the generated-config writer, after the `daily_target_hours` line (line 434):

```rust
    file.write_all(b"\n# Let the TUI capture the mouse: click a day, a bar or a\n")?;
    file.write_all(b"# row, and scroll with the wheel. Turn this off to keep the\n")?;
    file.write_all(b"# terminal's own click-drag text selection.\n")?;
    file.write_all(b"#mouse = true\n")?;
```

- [ ] **Step 4: Add the context field**

In `src/tui/context.rs`, add to `TuiContext` after `theme`:

```rust
    /// Does the TUI capture the mouse? See [`Config::mouse`].
    pub mouse: bool,
```

In `from_config`, after the `theme:` line:

```rust
            mouse: config.mouse.unwrap_or(true),
```

Also add `mouse: true` to whatever `TuiContext` test constructor exists further down the file (search for `pub fn for_test` or similar and add the field so it still compiles).

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --no-default-features --features tui mouse`
Expected: PASS (5 tests).

- [ ] **Step 6: Enable capture and chain the panic hook**

Rewrite `tui()` in `src/tui/mod.rs`:

```rust
use anyhow::Result;
use ratatui::crossterm::ExecutableCommand;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use std::io::stdout;

use crate::Config;

pub async fn tui() -> Result<()> {
    let config = Config::get();
    let ctx = context::TuiContext::from_config(config)?;
    let terminal = ratatui::init();
    // `ratatui::init` installed a hook that restores raw mode and leaves the
    // alternate screen — but it knows nothing about mouse capture, so a
    // panic would drop the user into a shell that emits escape sequences on
    // every drag. Ours is installed *after* ratatui's, so it runs *first*,
    // dropping capture while the alternate screen is still up.
    if ctx.mouse {
        stdout().execute(EnableMouseCapture)?;
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = stdout().execute(DisableMouseCapture);
            previous(info);
        }));
    }
    let mouse = ctx.mouse;
    let result = app::App::new(ctx)
        .with_active_date(config.date)
        .run(terminal)
        .await;
    if mouse {
        stdout().execute(DisableMouseCapture)?;
    }
    ratatui::restore();
    result
}
```

- [ ] **Step 7: Use the real field in `run_editor`**

In `src/tui/app.rs`, change the placeholder from Task 1:

```rust
        let modes = TerminalModes { mouse: ctx.mouse };
```

- [ ] **Step 8: Run the suite, clippy, fmt**

Run: `cargo test --workspace --no-default-features --features tui && cargo clippy --no-default-features --features tui --all-targets -- -D warnings && cargo fmt --all`
Expected: PASS, clean.

- [ ] **Step 9: Commit**

```bash
git add src/config.rs src/tui/context.rs src/tui/mod.rs src/tui/app.rs
git commit -m "feat(tui): capture the mouse, governed by a mouse config key"
```

---

## Task 3: Ctrl-Z suspend

**Files:**
- Modify: `Cargo.toml` — `libc` from dev-dependency to dependency (lines 110-111)
- Modify: `src/tui/event.rs` — `AppEvent::Suspend`
- Modify: `src/tui/keymap.rs` — binding row
- Modify: `src/tui/app.rs` — `handle_app_event` arm and the `suspend` method
- Modify: `README.md` — regenerated keybind table

**Interfaces:**
- Consumes: `with_suspended_terminal`, `TerminalModes` (Task 1); `TuiContext::mouse` (Task 2)
- Produces: `AppEvent::Suspend`; `App::suspend<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()>`

- [ ] **Step 1: Write the failing test**

In `src/tui/app.rs`'s test module:

```rust
    /// Ctrl-Z must pause the poller and resume it, exactly as the editor
    /// handover does. Driven through the event rather than through a real
    /// `SIGTSTP`: raising it in a test would stop the test runner.
    #[test]
    fn ctrl_z_is_bound_to_suspend_in_every_mode() {
        use crate::tui::keymap::{BINDINGS, Key};
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};

        let key: Key = (KeyCode::Char('Z'), KeyModifiers::CONTROL);
        let binding = BINDINGS
            .iter()
            .find(|b| b.keys.contains(&key))
            .expect("Ctrl-Z is bound");
        assert_eq!(binding.event, AppEvent::Suspend);
        for mode in [Mode::Day, Mode::Week, Mode::ZoomedWeek, Mode::RawFile] {
            assert!(
                binding.modes.contains(mode),
                "Ctrl-Z must be live in {mode:?}"
            );
        }
    }

    /// The row is unconditional so BINDINGS, the help popup, the generated
    /// README table and the test that compares it stay identical on every
    /// platform — only the handler is cfg-gated. The description therefore
    /// has to say where it works.
    #[test]
    fn the_suspend_binding_says_it_is_unix_only() {
        use crate::tui::keymap::BINDINGS;

        let binding = BINDINGS
            .iter()
            .find(|b| b.event == AppEvent::Suspend)
            .expect("suspend binding");
        assert!(
            binding.description.contains("Unix only"),
            "description must name the platform limit, got {:?}",
            binding.description
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --no-default-features --features tui suspend`
Expected: FAIL — no variant `Suspend` on `AppEvent`.

- [ ] **Step 3: Add the event variant**

In `src/tui/event.rs`, in `AppEvent`, after the `Edit` variant:

```rust
    /// Drop to the shell, leaving the process stopped until it is resumed
    /// with `fg`. Unix only; on Windows the handler reports that instead.
    Suspend,
```

- [ ] **Step 4: Add the binding**

In `src/tui/keymap.rs`, add to `BINDINGS` in the `Group::General` run (beside the `Edit` and `Quit` rows):

```rust
    Binding {
        keys: &[(KeyCode::Char('Z'), KeyModifiers::CONTROL)],
        event: AppEvent::Suspend,
        modes: ModeMask::ALL,
        group: Group::General,
        description: "suspend to the shell, resume with fg (Unix only)",
    },
```

Note the **uppercase `'Z'`** — this matches the existing `CTRL_C` satellite's `(KeyCode::Char('C'), KeyModifiers::CONTROL)` spelling, and `normalize` only strips SHIFT for `Char`, so the code must be spelled the way crossterm delivers it.

- [ ] **Step 5: Add the handler**

In `src/tui/app.rs`, `handle_app_event` — this needs the terminal, so it goes beside the `AppEvent::Edit` arm (line 559), **not** in the `apply_sync_event` delegation list:

```rust
            AppEvent::Suspend => {
                self.suspend(terminal).await?;
                self.dirty = true;
                self.events.send(AppEvent::ReloadFromDisk(Reload::Rescan));
            }
```

Then the method, next to `run_editor`:

```rust
    /// Stop the process and hand the terminal back to the shell, resuming
    /// where it left off when the user runs `fg`.
    ///
    /// Rust installs no `SIGTSTP` handler, so the signal's default action
    /// stops the process and `raise` returns only once `SIGCONT` arrives —
    /// which makes the statement after it the resume path. No `SIGCONT`
    /// handler is needed, and none is installed.
    #[cfg(unix)]
    pub async fn suspend<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let Self { events, ctx, .. } = self;
        let modes = TerminalModes { mouse: ctx.mouse };
        with_suspended_terminal(events, terminal, modes, || async {
            // SAFETY: `raise` is async-signal-safe and takes no pointers.
            // SIGTSTP's default disposition stops the process; nothing here
            // has installed a handler that would change that.
            let rc = unsafe { libc::raise(libc::SIGTSTP) };
            if rc != 0 {
                anyhow::bail!("could not suspend: raise(SIGTSTP) returned {rc}");
            }
            Ok(())
        })
        .await
    }

    /// Windows has no `SIGTSTP`, and the binding is deliberately not
    /// `cfg`-gated — one `BINDINGS` table keeps the help popup and the
    /// generated README identical on every platform — so the key has to
    /// answer for itself here instead of doing nothing.
    #[cfg(not(unix))]
    pub async fn suspend<B: Backend>(&mut self, _terminal: &mut Terminal<B>) -> Result<()> {
        self.set_status("Suspend is not available on Windows");
        Ok(())
    }
```

- [ ] **Step 6: Promote the `libc` dependency**

In the workspace `Cargo.toml`, lines 110-111 currently read:

```toml
[target.'cfg(unix)'.dev-dependencies]
libc = "0.2.177"
```

Add a real dependency section above it (keep the dev-dependency block — tests still use it):

```toml
# A hard dependency rather than an optional one behind `tui`: a `dep:libc`
# reference inside the `tui` feature would name a dependency that does not
# exist on Windows, and Cargo rejects that. Unix-only and tiny.
[target.'cfg(unix)'.dependencies]
libc = "0.2.177"
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test --no-default-features --features tui`
Expected: the two new tests PASS; `readme_keybind_table_matches_the_binding_table` now FAILS (the table gained a row).

- [ ] **Step 8: Regenerate the README table**

Run: `cargo test --no-default-features --features tui -- --ignored print_readme_table --nocapture`

Copy the printed table over the existing one in `README.md` (the block containing the `| Ctrl-C | All | quit, except it cancels the date prompt while that's open |` row around line 232).

- [ ] **Step 9: Verify the whole suite, clippy, fmt**

Run: `cargo test --workspace --no-default-features --features tui && cargo clippy --no-default-features --features tui --all-targets -- -D warnings && cargo fmt --all`
Expected: PASS, clean — including `readme_keybind_table_matches_the_binding_table`.

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml Cargo.lock src/tui/event.rs src/tui/keymap.rs src/tui/app.rs README.md
git commit -m "feat(tui): suspend to the shell with Ctrl-Z"
```

---

## Task 4: Record where things were drawn

**Files:**
- Create: `src/tui/layout_rects.rs`
- Modify: `src/tui/mod.rs` (register)
- Modify: `src/tui/app.rs` (field + reset)
- Modify: `src/tui/ui.rs` (record during render)
- Modify: `src/tui/widgets/help_popup.rs`, `src/tui/widgets/date_prompt.rs` (expose popup rect)

**Interfaces:**
- Produces:
  - `pub struct LayoutRects` with `pub calendar/bar_chart/project_list/week_list/raw_file/help_hint/overlay: Option<Rect>` and `LayoutRects::clear(&mut self)`
  - `App::layout: LayoutRects` (public field)
  - `HelpPopup::popup_rect(&self, area: Rect) -> Rect`
  - `DatePrompt::popup_rect(area: Rect) -> Rect` (associated function — takes no `self`)

- [ ] **Step 1: Write the failing test**

In `src/tui/app.rs`'s test module (it already has `render_to_string` and fixtures in scope via `crate::tui::testing`):

```rust
    /// Hit-testing reads these, so a region that was not drawn this frame
    /// must not be hittable. A terminal too small for the band is the case
    /// that matters: the calendar is simply absent.
    #[test]
    fn rendering_records_the_regions_it_drew() {
        let mut app = day_app();
        let _ = render_to_string(&mut app, 120, 40);

        assert!(app.layout.calendar.is_some(), "calendar was drawn");
        assert!(app.layout.bar_chart.is_some(), "bar chart was drawn");
        assert!(app.layout.project_list.is_some(), "project list was drawn");
        assert!(app.layout.help_hint.is_some(), "status line was drawn");
        assert!(app.layout.overlay.is_none(), "no overlay is open");
    }

    #[test]
    fn a_terminal_too_small_for_the_band_records_no_calendar() {
        let mut app = day_app();
        let _ = render_to_string(&mut app, 80, 24);

        assert!(
            app.layout.calendar.is_none(),
            "the band was not drawn, so nothing there is clickable"
        );
    }

    #[test]
    fn an_open_overlay_records_its_rect() {
        let mut app = day_app();
        app.overlay = Some(Overlay::Help);
        let _ = render_to_string(&mut app, 120, 40);

        let overlay = app.layout.overlay.expect("help popup was drawn");
        assert!(overlay.width > 0 && overlay.height > 0);
    }
```

`day_app()` is the existing helper at `src/tui/app.rs:1979` — `App::new(TuiContext::for_test()).with_active_date(fixture_date()).with_data(fixture_day())`. Use it; do not add a duplicate. `selection(app)` (line 1985) is the existing helper for reading the project selection.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --no-default-features --features tui records`
Expected: FAIL — no field `layout` on `App`.

- [ ] **Step 3: Write `LayoutRects`**

Create `src/tui/layout_rects.rs`:

```rust
//! Where each clickable region was drawn on the most recent frame.
//!
//! Mouse hit-testing can only run after a draw, so this is filled in during
//! render and read by [`App::handle_mouse_event`](super::app::App). Every
//! field is cleared at the top of each frame: a region that was not drawn —
//! the calendar band on a short terminal, the week list while the day view
//! is up — must not be hittable, and `None` is what says so.

use ratatui::layout::Rect;

/// The regions the most recent frame drew, or `None` for regions it did not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutRects {
    /// The month calendar in the day view's header band.
    pub calendar: Option<Rect>,
    /// The weekly bar chart, in the band or full-screen when zoomed.
    pub bar_chart: Option<Rect>,
    /// The day view's project list, inside its border.
    pub project_list: Option<Rect>,
    /// The weekly rollup list, inside its border.
    pub week_list: Option<Rect>,
    /// The raw-file pane.
    pub raw_file: Option<Rect>,
    /// The footer, which carries the "press ? for help" hint.
    pub help_hint: Option<Rect>,
    /// The open overlay's box, if one is open. A click outside this while it
    /// is `Some` dismisses the overlay instead of reaching anything behind.
    pub overlay: Option<Rect>,
}

impl LayoutRects {
    /// Forget the previous frame. Called at the top of every render.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Does `rect` contain the cell at (`x`, `y`)?
///
/// `Rect::contains` exists in ratatui, but takes a `Position`; this keeps
/// the hit-test call sites reading in terminal coordinates.
pub fn hits(rect: Option<Rect>, x: u16, y: u16) -> bool {
    rect.is_some_and(|r| {
        x >= r.x && x < r.x.saturating_add(r.width) && y >= r.y && y < r.y.saturating_add(r.height)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_none_rect_is_never_hit() {
        assert!(!hits(None, 0, 0));
    }

    #[test]
    fn hits_are_inclusive_of_the_origin_and_exclusive_of_the_far_edge() {
        let r = Some(Rect::new(2, 3, 4, 5));
        assert!(hits(r, 2, 3), "origin");
        assert!(hits(r, 5, 7), "last cell");
        assert!(!hits(r, 6, 7), "one past the right edge");
        assert!(!hits(r, 5, 8), "one past the bottom edge");
        assert!(!hits(r, 1, 3), "one before the left edge");
    }

    #[test]
    fn clear_forgets_every_region() {
        let mut rects = LayoutRects {
            calendar: Some(Rect::new(0, 0, 1, 1)),
            ..LayoutRects::default()
        };
        rects.clear();
        assert_eq!(rects, LayoutRects::default());
    }
}
```

Register it in `src/tui/mod.rs`:

```rust
pub mod layout_rects;
```

- [ ] **Step 4: Add the field to `App`**

In `src/tui/app.rs`, add to the struct (near `mode`/`overlay`, around line 213):

```rust
    /// Where each clickable region was drawn on the most recent frame; see
    /// [`LayoutRects`].
    pub layout: LayoutRects,
```

and to `App::new`'s initializer:

```rust
            layout: LayoutRects::default(),
```

with `use super::layout_rects::LayoutRects;` at the top.

- [ ] **Step 5: Expose the popup rects**

In `src/tui/widgets/help_popup.rs`, add to `impl HelpPopup<'_>` (the inherent impl, not the `Widget` one):

```rust
    /// Where [`HelpPopup::render`] would draw its box in `area`.
    ///
    /// Recomputes the content rather than caching it: the popup is sized to
    /// fit its rows, so the rect cannot be known without them, and this runs
    /// once per frame on a table of about thirty entries.
    pub fn popup_rect(&self, area: Rect) -> Rect {
        popup_area(area, &self.content())
    }
```

In `src/tui/widgets/date_prompt.rs`, add to `impl DatePrompt<'_>`:

```rust
    /// Where [`DatePrompt::render`] would draw its box in `area`. Fixed
    /// size, so unlike the help popup this needs nothing from `self`.
    pub fn popup_rect(area: Rect) -> Rect {
        popup_area(area)
    }
```

If `self.content()` is a private method on `HelpPopup`, leave it private — `popup_rect` is in the same module.

- [ ] **Step 6: Record the rects during render**

In `src/tui/ui.rs`:

1. At the top of `impl Widget for &mut App`'s `render`, before the layout split:

```rust
        // A region not drawn this frame must not be hittable next frame.
        self.layout.clear();
```

2. In `render_day_header`, after each widget renders:

```rust
        if bp == Breakpoint::Narrow {
            self.weekly_bar_chart().render(area, buf);
            self.layout.bar_chart = Some(area);
            draw_header_border(area, buf);
            return;
        }
```

and in the wide branch, after the two renders:

```rust
        self.layout.calendar = Some(calendar_area);
        self.layout.bar_chart = Some(bar_chart_area);
```

3. In `render_day`'s `DayPane::Projects` arm, after `block.render(chunks[1], buf);`:

```rust
                self.layout.project_list = Some(inner);
```

4. In `render_zoomed_week`:

```rust
        self.layout.bar_chart = Some(area);
```

5. In `render_week`'s `WeekPane::Projects` arm, after the block renders, record `inner` into `self.layout.week_list`.

6. In `render_raw_file`, record the pane's inner area into `self.layout.raw_file`.

7. Change `fn render_status(&self, ...)` to `fn render_status(&mut self, ...)` and record:

```rust
        self.layout.help_hint = Some(area);
```

8. In `render_overlay`, record each popup's rect:

```rust
    fn render_overlay(&mut self, area: Rect, buf: &mut Buffer) {
        match &self.overlay {
            Some(Overlay::Help) => {
                let popup = HelpPopup::new(&self.ctx.theme, self.mode);
                self.layout.overlay = Some(popup.popup_rect(area));
                popup.render(area, buf);
            }
            Some(Overlay::DatePrompt(input)) => {
                self.layout.overlay = Some(DatePrompt::popup_rect(area));
                DatePrompt::new(&self.ctx.theme, input).render(area, buf);
            }
            None => {}
        }
    }
```

The `Some(Overlay::DatePrompt(input))` arm borrows `self.overlay` immutably while assigning to `self.layout`. If the borrow checker objects, clone the input first: `let input = input.clone();` before the assignment.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test --no-default-features --features tui`
Expected: PASS, including the three new ones and every existing render test.

- [ ] **Step 8: Clippy and fmt**

Run: `cargo clippy --no-default-features --features tui --all-targets -- -D warnings && cargo fmt --all`

- [ ] **Step 9: Commit**

```bash
git add src/tui/layout_rects.rs src/tui/mod.rs src/tui/app.rs src/tui/ui.rs src/tui/widgets/help_popup.rs src/tui/widgets/date_prompt.rs
git commit -m "feat(tui): record where each clickable region was drawn"
```

---

## Task 5: Calendar hit-testing

The riskiest piece: it replicates `ratatui::widgets::calendar::Monthly`'s internal geometry. The characterization test is what makes that safe.

**Files:**
- Modify: `src/tui/widgets/calendar.rs`

**Interfaces:**
- Produces: `Calendar::date_at(&self, area: Rect, x: u16, y: u16) -> Option<Date>`

- [ ] **Step 1: Write the failing characterization test**

In `src/tui/widgets/calendar.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use time::macros::date;

    const AREA: Rect = Rect { x: 0, y: 0, width: 24, height: 12 };

    fn render_calendar(active: Date) -> Buffer {
        let theme = Theme::none();
        let populated: Vec<Date> = Vec::new();
        let mut buf = Buffer::empty(AREA);
        Calendar::new(active, &populated, &theme).render(AREA, &mut buf);
        buf
    }

    /// The two-character day number drawn at (`x`, `y`), or `None` if that
    /// cell is not the start of one.
    fn day_number_at(buf: &Buffer, x: u16, y: u16) -> Option<u32> {
        let text: String = (x..x + 2).map(|cx| buf[(cx, y)].symbol()).collect();
        text.trim().parse().ok()
    }

    /// The load-bearing test for this whole feature.
    ///
    /// `Monthly` exposes no hit-test, so `date_at` replicates its geometry.
    /// This walks every cell of a real rendered calendar and asserts that
    /// wherever `date_at` claims a date, the digits actually on screen are
    /// that date's day number. If ratatui ever changes `Monthly`'s layout
    /// this fails loudly, instead of the app silently jumping to a day the
    /// user did not click.
    #[test]
    fn date_at_agrees_with_what_monthly_actually_drew() {
        let active = date!(2025 - 06 - 11);
        let buf = render_calendar(active);
        let theme = Theme::none();
        let populated: Vec<Date> = Vec::new();
        let calendar = Calendar::new(active, &populated, &theme);

        let mut matched = 0;
        for y in AREA.y..AREA.y + AREA.height {
            for x in AREA.x..AREA.x + AREA.width {
                let Some(hit) = calendar.date_at(AREA, x, y) else {
                    continue;
                };
                let drawn = day_number_at(&buf, x, y)
                    .unwrap_or_else(|| panic!("date_at claimed {hit} at ({x},{y}) but no day number is drawn there"));
                assert_eq!(
                    u32::from(hit.day()),
                    drawn,
                    "date_at said {hit} at ({x},{y}) but the screen shows {drawn}"
                );
                matched += 1;
            }
        }
        assert!(
            matched >= 28,
            "expected to hit at least a month of days, only matched {matched}"
        );
    }

    /// `Monthly` always starts its weeks on Sunday, whatever the app's
    /// `week_start_day` is set to. A hit-test that used the configured
    /// start would be wrong on every non-Sunday configuration — the single
    /// most likely bug in this feature.
    #[test]
    fn the_first_column_is_sunday() {
        let active = date!(2025 - 06 - 11);
        let theme = Theme::none();
        let populated: Vec<Date> = Vec::new();
        let calendar = Calendar::new(active, &populated, &theme);

        let first_hit = (AREA.y..AREA.y + AREA.height)
            .flat_map(|y| (AREA.x..AREA.x + AREA.width).map(move |x| (x, y)))
            .find_map(|(x, y)| calendar.date_at(AREA, x, y))
            .expect("some cell resolves to a date");

        assert_eq!(
            first_hit.weekday(),
            time::Weekday::Sunday,
            "the topmost-leftmost day cell must be a Sunday"
        );
    }

    #[test]
    fn clicks_outside_the_day_grid_resolve_to_nothing() {
        let active = date!(2025 - 06 - 11);
        let theme = Theme::none();
        let populated: Vec<Date> = Vec::new();
        let calendar = Calendar::new(active, &populated, &theme);

        // The month-header row.
        assert_eq!(calendar.date_at(AREA, 5, AREA.y), None);
        // Far outside the area.
        assert_eq!(calendar.date_at(AREA, 200, 200), None);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --no-default-features --features tui calendar`
Expected: FAIL — no method `date_at`.

- [ ] **Step 3: Implement `date_at`**

Add to `impl<'a> Calendar<'a>` in `src/tui/widgets/calendar.rs`:

```rust
    /// The date drawn at (`x`, `y`) when this calendar is rendered in
    /// `area`, or `None` for a cell that is not a day.
    ///
    /// # Why this duplicates ratatui
    ///
    /// [`Monthly`] exposes no hit-test, so this replicates its layout:
    /// the block (a right border and one column/row of padding), then one
    /// month-header row, one weekday-header row, then week rows in which
    /// weekday `i` occupies the two columns at `1 + 3 * i`.
    ///
    /// **Weeks start on Sunday**, because `Monthly` starts them on Sunday —
    /// it offsets from `number_days_from_sunday` internally and takes no
    /// first-day parameter. This deliberately ignores the app's
    /// `week_start_day`; using it here would be wrong on every
    /// non-Sunday configuration.
    ///
    /// `date_at_agrees_with_what_monthly_actually_drew` is what keeps this
    /// honest against a ratatui upgrade.
    pub fn date_at(&self, area: Rect, x: u16, y: u16) -> Option<Date> {
        // The block this widget renders with: `Borders::RIGHT` and
        // `Padding { left: 1, top: 1 }`. Kept in step with `render` below.
        let inner = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.checked_sub(2)?,
            height: area.height.checked_sub(1)?,
        };

        // One month-header row and one weekday-header row, both shown.
        let grid_y = inner.y + 2;
        if y < grid_y || y >= inner.y + inner.height {
            return None;
        }
        if x < inner.x || x >= inner.x + inner.width {
            return None;
        }

        // " Su Mo Tu ..." — a one-column gutter, then two columns per day.
        let column = x.checked_sub(inner.x)?;
        if column % 3 == 0 {
            // The gutter between day cells.
            return None;
        }
        let weekday_index = u16::from(column / 3);
        if weekday_index >= 7 {
            return None;
        }
        let week_index = y - grid_y;

        // `Monthly` starts the grid at the Sunday on or before the 1st.
        let first_of_month = self.active_date.replace_day(1).ok()?;
        let offset = i64::from(first_of_month.weekday().number_days_from_sunday());
        let grid_start = first_of_month.checked_sub(time::Duration::days(offset))?;

        let day_index = i64::from(week_index) * 7 + i64::from(weekday_index);
        let hit = grid_start.checked_add(time::Duration::days(day_index))?;

        // `show_surrounding` draws the neighbouring months' days greyed out.
        // They are real cells and clicking one navigates there, which is what
        // makes paging months by click work.
        Some(hit)
    }
```

Note: `Rect` must be in scope — it comes from `ratatui::prelude::*`, already imported at the top of the file.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --no-default-features --features tui calendar`
Expected: PASS (3 tests).

If `date_at_agrees_with_what_monthly_actually_drew` fails at a specific cell, the discrepancy is in the block inset or the column formula — fix `date_at`, never the test's expectation, since the test reads the truth off the rendered buffer.

- [ ] **Step 5: Clippy, fmt, commit**

```bash
cargo clippy --no-default-features --features tui --all-targets -- -D warnings && cargo fmt --all
git add src/tui/widgets/calendar.rs
git commit -m "feat(tui): resolve a calendar cell to its date"
```

---

## Task 6: Bar chart hit-testing

**Files:**
- Modify: `src/tui/widgets/weekly_bar_chart.rs`

**Interfaces:**
- Consumes: the existing private `calculate_bar_dimensions(&self, area: Rect) -> (u16, u16)` (line 163)
- Produces: `WeeklyBarChart::date_at(&self, area: Rect, x: u16, y: u16) -> Option<Date>`

- [ ] **Step 1: Write the failing test**

In `src/tui/widgets/weekly_bar_chart.rs`'s existing test module:

```rust
    /// Every bar resolves to its own day, at more than one width — the
    /// widths are what exercise the shared `calculate_bar_dimensions`
    /// rather than one hard-coded layout.
    #[test]
    fn each_bar_resolves_to_its_day() {
        let theme = Theme::none();
        let week = week();
        let chart = WeeklyBarChart::new(date!(2026 - 08 - 24), &week, &theme);

        for width in [40_u16, 80, 120] {
            let area = Rect::new(0, 0, width, 12);
            let mut seen: Vec<Date> = Vec::new();
            for x in area.x..area.x + area.width {
                if let Some(d) = chart.date_at(area, x, area.y + 3)
                    && !seen.contains(&d)
                {
                    seen.push(d);
                }
            }
            assert_eq!(
                seen.len(),
                7,
                "at width {width} every one of the seven days should be reachable, got {seen:?}"
            );
            for (i, d) in seen.iter().enumerate() {
                assert_eq!(*d, week[i], "bars must resolve left-to-right at width {width}");
            }
        }
    }

    #[test]
    fn clicks_outside_the_chart_resolve_to_nothing() {
        let theme = Theme::none();
        let week = week();
        let chart = WeeklyBarChart::new(date!(2026 - 08 - 24), &week, &theme);
        let area = Rect::new(0, 0, 80, 12);

        assert_eq!(chart.date_at(area, 500, 3), None);
        assert_eq!(chart.date_at(area, 5, 500), None);
    }
```

The `week()` helper already exists in that test module (it is used by the tests around line 352). If it returns `[Date; 7]`, index it directly as above.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --no-default-features --features tui weekly_bar_chart`
Expected: FAIL — no method `date_at`.

- [ ] **Step 3: Implement `date_at`**

Add to `impl<'a> WeeklyBarChart<'a>`:

```rust
    /// The day whose bar is drawn at (`x`, `y`) in `area`, or `None`.
    ///
    /// Reads its geometry from [`WeeklyBarChart::calculate_bar_dimensions`]
    /// — the same function `render` lays the bars out with — so the two
    /// cannot drift into disagreeing about where a bar is.
    pub fn date_at(&self, area: Rect, x: u16, y: u16) -> Option<Date> {
        if x < area.x || x >= area.x + area.width || y < area.y || y >= area.y + area.height {
            return None;
        }

        let (bar_width, bar_gap) = self.calculate_bar_dimensions(area);
        let stride = bar_width.checked_add(bar_gap)?;
        if stride == 0 {
            return None;
        }

        // The block's `Padding { left: 1, .. }` plus the chart's own left
        // border: the same two columns `content_width` subtracts for.
        let content_x = area.x + 2;
        let offset = x.checked_sub(content_x)?;

        let index = offset / stride;
        // The gap after a bar belongs to no day.
        if offset % stride >= bar_width {
            return None;
        }
        self.week_dates.get(usize::from(index)).copied()
    }
```

If `calculate_bar_dimensions` is private, this method is in the same `impl` block so it stays private — do not widen its visibility.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --no-default-features --features tui weekly_bar_chart`
Expected: PASS.

If `each_bar_resolves_to_its_day` finds fewer than seven days, `content_x` is off by a column — compare against `calculate_bar_dimensions`' `area.width.saturating_sub(4)` and adjust the constant, not the test.

- [ ] **Step 5: Clippy, fmt, commit**

```bash
cargo clippy --no-default-features --features tui --all-targets -- -D warnings && cargo fmt --all
git add src/tui/widgets/weekly_bar_chart.rs
git commit -m "feat(tui): resolve a bar chart column to its day"
```

---

## Task 7: List row hit-testing

**Files:**
- Modify: `src/tui/project_list.rs`
- Modify: `src/tui/week_list.rs`

**Interfaces:**
- Consumes: `ProjectItem::body_rows(&self, width: u16) -> usize` (line 150), `ListState::offset()`
- Produces:
  - `ProjectListWidget::index_at(&self, area: Rect, y: u16) -> Option<usize>`
  - `ProjectListWidget::select_index(&mut self, index: usize)`
  - `WeekListState::index_at(&self, area: Rect, y: u16, count: usize) -> Option<usize>`
  - `WeekListState::select_index(&mut self, index: usize)`

- [ ] **Step 1: Write the failing tests**

In `src/tui/project_list.rs`'s test module:

```rust
    /// Rows are variable-height — `render_list` builds each item from
    /// `clamp_item_rows(body(width), viewport_rows)` — so a click maps to an
    /// index by walking heights, not by dividing. A day whose projects have
    /// different note counts is what tells the two apart.
    #[test]
    fn index_at_walks_variable_row_heights() {
        let data = TimeTrackingData {
            total_minutes: 180,
            dead_time_minutes: 0,
            projects: vec![
                project("one-note", 60, ["a"]),
                project("three-notes", 60, ["a", "b", "c"]),
                project("two-notes", 60, ["a", "b"]),
            ],
            warnings: Vec::new(),
            start_time: None,
            end_time: None,
        };
        let theme = Theme::none();
        let widget = ProjectListWidget::new(&data, &theme);
        let area = Rect::new(0, 0, 60, 30);

        let mut hits: Vec<usize> = Vec::new();
        for y in area.y..area.y + area.height {
            if let Some(i) = widget.index_at(area, y)
                && !hits.contains(&i)
            {
                hits.push(i);
            }
        }
        assert_eq!(hits, vec![0, 1, 2], "each project reachable, in order");
    }

    #[test]
    fn index_at_ignores_clicks_above_the_first_row() {
        let theme = Theme::none();
        let widget = ProjectListWidget::new(&fixture_day(), &theme);
        let area = Rect::new(0, 5, 60, 30);
        // The list's title row.
        assert_eq!(widget.index_at(area, 5), None);
    }

    #[test]
    fn select_index_moves_the_selection() {
        let theme = Theme::none();
        let mut widget = ProjectListWidget::new(&fixture_day(), &theme);
        widget.select_index(2);
        assert_eq!(widget.selected_item(), Some(2));
    }

    #[test]
    fn select_index_past_the_end_is_ignored() {
        let theme = Theme::none();
        let mut widget = ProjectListWidget::new(&fixture_day(), &theme);
        let before = widget.selected_item();
        widget.select_index(999);
        assert_eq!(widget.selected_item(), before);
    }
```

Use the test module's existing `project(...)` and `fixture_day()` helpers — check which are already imported there and import from `crate::tui::testing` if not.

In `src/tui/week_list.rs`'s test module:

```rust
    #[test]
    fn index_at_maps_rows_one_for_one() {
        let state = WeekListState::default();
        let area = Rect::new(0, 4, 60, 10);
        assert_eq!(state.index_at(area, 4, 3), Some(0));
        assert_eq!(state.index_at(area, 5, 3), Some(1));
        assert_eq!(state.index_at(area, 6, 3), Some(2));
        assert_eq!(state.index_at(area, 7, 3), None, "past the last project");
        assert_eq!(state.index_at(area, 3, 3), None, "above the list");
    }

    #[test]
    fn select_index_moves_the_selection() {
        let mut state = WeekListState::default();
        state.select_index(1);
        assert_eq!(state.selected(), Some(1));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --no-default-features --features tui index_at`
Expected: FAIL — no method `index_at`.

- [ ] **Step 3: Implement on `ProjectListWidget`**

```rust
    /// The project index drawn at row `y` when this list is rendered in
    /// `area`, or `None` for the title row and for rows past the last item.
    ///
    /// Rows are **variable-height**: [`ProjectListWidget::render_list`]
    /// builds each item from `clamp_item_rows(body(width), viewport_rows)`,
    /// so an index cannot be divided out of `y`. This walks the same heights
    /// from the same `body_rows`, starting at [`ListState::offset`] — which
    /// is current because hit-testing only ever runs after a draw.
    pub fn index_at(&self, area: Rect, y: u16) -> Option<usize> {
        let inner_y = area.y.checked_add(LIST_TITLE_ROWS)?;
        if y < inner_y || y >= area.y.checked_add(area.height)? {
            return None;
        }

        let body_width = area.width.saturating_sub(4);
        let viewport_rows = area.height.saturating_sub(LIST_TITLE_ROWS);
        let target = y - inner_y;

        let mut row = 0_u16;
        for index in self.project_list.state.offset()..self.project_list.items.len() {
            let item = self.project_list.items.get(index)?;
            let height = u16::try_from(item.body_rows(body_width))
                .unwrap_or(u16::MAX)
                .min(viewport_rows);
            let next = row.checked_add(height)?;
            if target < next {
                return Some(index);
            }
            row = next;
        }
        None
    }

    /// Select `index`, ignoring one past the end.
    pub fn select_index(&mut self, index: usize) {
        if index < self.project_list.items.len() {
            self.project_list.state.select(Some(index));
        }
    }
```

`LIST_TITLE_ROWS` already exists in this file (it is used by `render_list`). If `body_rows` is a method on the item type rather than on the widget, call it as shown; if its name differs, use the real one.

- [ ] **Step 4: Implement on `WeekListState`**

```rust
    /// The project index drawn at row `y` when the rollup is rendered in
    /// `area`, given `count` projects, or `None` outside the rows.
    ///
    /// Rows here are single-height, unlike the day view's project list.
    pub fn index_at(&self, area: Rect, y: u16, count: usize) -> Option<usize> {
        if y < area.y || y >= area.y.checked_add(area.height)? {
            return None;
        }
        let index = self.list.offset() + usize::from(y - area.y);
        (index < count).then_some(index)
    }

    /// Select `index`, ignoring one past the end.
    pub fn select_index(&mut self, index: usize) {
        self.list.select(Some(index));
    }
```

`select_index` here takes no bound because `index_at` is the only caller and has already bounded it against `count`; note that in the doc comment.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --no-default-features --features tui index_at`
Expected: PASS (6 tests).

- [ ] **Step 6: Clippy, fmt, commit**

```bash
cargo clippy --no-default-features --features tui --all-targets -- -D warnings && cargo fmt --all
git add src/tui/project_list.rs src/tui/week_list.rs
git commit -m "feat(tui): resolve a list row to its index"
```

---

## Task 8: Handle mouse events

**Files:**
- Modify: `src/tui/app.rs` — the `run` loop arm (before line 496) and a new `handle_mouse_event`

**Interfaces:**
- Consumes: everything from Tasks 4-7
- Produces: `App::handle_mouse_event(&mut self, event: MouseEvent) -> Result<()>`

- [ ] **Step 1: Write the failing tests**

In `src/tui/app.rs`'s test module:

```rust
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    fn click(x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn wheel(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
        MouseEvent { kind, column: x, row: y, modifiers: KeyModifiers::NONE }
    }

    /// The first cell `date_at` resolves inside the recorded calendar rect.
    fn a_calendar_cell(app: &App) -> (u16, u16, Date) {
        let area = app.layout.calendar.expect("calendar drawn");
        let calendar = Calendar::new(app.active_date, &app.populated_dates, &app.ctx.theme);
        (area.y..area.y + area.height)
            .flat_map(|y| (area.x..area.x + area.width).map(move |x| (x, y)))
            .find_map(|(x, y)| calendar.date_at(area, x, y).map(|d| (x, y, d)))
            .expect("some calendar cell resolves")
    }

    #[test]
    fn clicking_a_calendar_day_goes_to_it() {
        let mut app = day_app();
        let _ = render_to_string(&mut app, 120, 40);
        let (x, y, expected) = a_calendar_cell(&app);

        app.handle_mouse_event(click(x, y)).expect("mouse");

        assert_eq!(app.active_date, expected);
    }

    #[test]
    fn clicking_a_project_row_selects_it() {
        let mut app = day_app();
        let _ = render_to_string(&mut app, 120, 40);
        let list = app.layout.project_list.expect("list drawn");

        // The second row of the list body.
        app.handle_mouse_event(click(list.x + 1, list.y + 1))
            .expect("mouse");

        assert!(
            app.project_list_widget
                .as_ref()
                .and_then(|w| w.selected_item())
                .is_some(),
            "a click in the list must select something"
        );
    }

    /// Overlay-first: nothing behind a modal is reachable, and a click
    /// outside it dismisses. This is the rule `mode.rs` already states for
    /// keys, applied to clicks.
    #[test]
    fn clicking_outside_an_open_overlay_dismisses_it_and_nothing_else() {
        let mut app = day_app();
        app.overlay = Some(Overlay::Help);
        let _ = render_to_string(&mut app, 120, 40);
        let before = app.active_date;
        let (x, y, _) = {
            let area = app.layout.calendar.expect("calendar drawn");
            let calendar = Calendar::new(app.active_date, &app.populated_dates, &app.ctx.theme);
            (area.y..area.y + area.height)
                .flat_map(|y| (area.x..area.x + area.width).map(move |x| (x, y)))
                .find_map(|(x, y)| calendar.date_at(area, x, y).map(|d| (x, y, d)))
                .expect("cell")
        };

        app.handle_mouse_event(click(x, y)).expect("mouse");

        assert!(app.overlay.is_none(), "the click dismissed the popup");
        assert_eq!(app.active_date, before, "and did not reach the calendar");
    }

    #[test]
    fn clicking_inside_an_open_overlay_leaves_it_open() {
        let mut app = day_app();
        app.overlay = Some(Overlay::Help);
        let _ = render_to_string(&mut app, 120, 40);
        let popup = app.layout.overlay.expect("popup drawn");

        app.handle_mouse_event(click(popup.x + 1, popup.y + 1))
            .expect("mouse");

        assert!(app.overlay.is_some(), "a click inside the popup is inert");
    }

    #[test]
    fn clicking_the_footer_opens_help() {
        let mut app = day_app();
        let _ = render_to_string(&mut app, 120, 40);
        let footer = app.layout.help_hint.expect("footer drawn");

        app.handle_mouse_event(click(footer.x, footer.y)).expect("mouse");

        assert!(matches!(app.overlay, Some(Overlay::Help)));
    }

    #[test]
    fn the_wheel_moves_the_project_selection() {
        let mut app = day_app();
        let _ = render_to_string(&mut app, 120, 40);
        let list = app.layout.project_list.expect("list drawn");
        let before = app.project_list_widget.as_ref().and_then(|w| w.selected_item());

        app.handle_mouse_event(wheel(MouseEventKind::ScrollDown, list.x + 1, list.y + 1))
            .expect("mouse");

        let after = app.project_list_widget.as_ref().and_then(|w| w.selected_item());
        assert_ne!(before, after, "the wheel moved the selection");
    }

    /// Nothing was drawn, so nothing is hittable — the guard that keeps a
    /// stale rect from a previous frame from being clicked.
    #[test]
    fn clicks_before_the_first_render_do_nothing() {
        let mut app = day_app();
        let before = app.active_date;

        app.handle_mouse_event(click(5, 5)).expect("mouse");

        assert_eq!(app.active_date, before);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --no-default-features --features tui mouse`
Expected: FAIL — no method `handle_mouse_event`.

- [ ] **Step 3: Add the event-loop arm**

In `App::run`, **before** the existing `Event::Crossterm(_) => {}` (line 496):

```rust
                Event::Crossterm(CrosstermEvent::Mouse(mouse_event)) => {
                    self.handle_mouse_event(mouse_event)?;
                }
```

The catch-all silently discards anything unmatched, so this arm must precede it.

- [ ] **Step 4: Implement `handle_mouse_event`**

```rust
    /// Route a mouse event to whatever was drawn under it.
    ///
    /// Only presses of the left button and the two wheel directions are
    /// acted on. Motion and drags are ignored: crossterm's
    /// `EnableMouseCapture` uses mode 1002 (button-event tracking), so an
    /// idle mouse generates nothing at all and the loop is never woken by
    /// the cursor moving.
    ///
    /// Dispatch is **overlay-first**, the same rule
    /// [`Overlay`](super::mode::Overlay) states for keys: while one is open,
    /// a click inside it is inert and a click outside dismisses it. Nothing
    /// falls through to a widget behind a modal.
    ///
    /// Hits are resolved against [`App::layout`], which the previous frame
    /// filled in — so a region that was not drawn cannot be clicked.
    pub fn handle_mouse_event(&mut self, event: MouseEvent) -> Result<()> {
        let (x, y) = (event.column, event.row);

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {}
            MouseEventKind::ScrollDown => return self.scroll_at(x, y, true),
            MouseEventKind::ScrollUp => return self.scroll_at(x, y, false),
            _ => return Ok(()),
        }

        // Overlay first: it owns every click while it is open.
        if self.overlay.is_some() {
            if !hits(self.layout.overlay, x, y) {
                self.overlay = None;
                self.dirty = true;
            }
            return Ok(());
        }

        if hits(self.layout.help_hint, x, y) {
            self.toggle_help();
            self.dirty = true;
            return Ok(());
        }

        if let Some(area) = self.layout.calendar
            && hits(Some(area), x, y)
            && let Some(date) =
                Calendar::new(self.active_date, &self.populated_dates, &self.ctx.theme)
                    .date_at(area, x, y)
        {
            self.go_to_date(date);
            return Ok(());
        }

        if let Some(area) = self.layout.bar_chart
            && hits(Some(area), x, y)
            && let Some(date) = self.weekly_bar_chart().date_at(area, x, y)
        {
            self.go_to_date(date);
            return Ok(());
        }

        if let Some(area) = self.layout.project_list
            && hits(Some(area), x, y)
            && let Some(widget) = &mut self.project_list_widget
            && let Some(index) = widget.index_at(area, y)
        {
            widget.select_index(index);
            self.dirty = true;
            return Ok(());
        }

        if let Some(area) = self.layout.week_list
            && hits(Some(area), x, y)
        {
            // `weekly_summary` is the rollup; `weekly_data` is the bar
            // chart's per-day minutes and has no projects. `week_projects`
            // is the existing accessor that also honours `week_is_stale`,
            // so a click cannot select into last week's list.
            let count = week_projects(self.weekly_summary.as_ref(), self.week_is_stale()).len();
            if let Some(index) = self.week_list.index_at(area, y, count) {
                self.week_list.select_index(index);
                self.dirty = true;
            }
            return Ok(());
        }

        Ok(())
    }

    /// Move the selection in whichever list the wheel is over.
    ///
    /// The selection moves rather than a detached viewport, because
    /// [`ListState`] has no scroll offset independent of the selection —
    /// scrolling one without the other would leave the highlight off-screen.
    fn scroll_at(&mut self, x: u16, y: u16, down: bool) -> Result<()> {
        if self.overlay.is_some() {
            return Ok(());
        }

        let event = if hits(self.layout.project_list, x, y) {
            if down { AppEvent::NextProject } else { AppEvent::PreviousProject }
        } else if hits(self.layout.week_list, x, y) {
            if down { AppEvent::NextWeekProject } else { AppEvent::PreviousWeekProject }
        } else if hits(self.layout.raw_file, x, y) {
            if down { AppEvent::ScrollRawFileDown } else { AppEvent::ScrollRawFileUp }
        } else {
            return Ok(());
        };

        self.apply_sync_event(event);
        Ok(())
    }
```

Add the imports at the top of `app.rs`:

```rust
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use super::layout_rects::hits;
use super::widgets::calendar::Calendar;
```

These names are verified against the current file: `App::week_list: WeekListState` (line 319), `App::weekly_summary: Option<WeeklySummary>` (line 314), the free function `week_projects(summary, stale) -> &[WeeklyProject]` (line 1767), `App::toggle_help(&mut self)` (line 753), and `App::apply_sync_event(&mut self, AppEvent)` (line 607). Note that `App::weekly_data` is a `HashMap<Date, u32>` of per-day minutes for the bar chart — it is *not* the rollup and has no project list.

The `let ... && let ...` chains use Rust 2024 let-chains, which this edition supports.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --no-default-features --features tui`
Expected: PASS, including all eight new tests and every existing test.

- [ ] **Step 6: Clippy, fmt, commit**

```bash
cargo clippy --no-default-features --features tui --all-targets -- -D warnings && cargo fmt --all
git add src/tui/app.rs
git commit -m "feat(tui): act on clicks and wheel scrolling"
```

---

## Task 9: Double-click to edit

**Files:**
- Modify: `src/tui/app.rs`

**Interfaces:**
- Consumes: `handle_mouse_event` (Task 8), `AppEvent::Edit`
- Produces: `App::last_click: Option<(Instant, u16, u16)>` (private field)

- [ ] **Step 1: Write the failing tests**

```rust
    /// A second click at the same cell inside the window opens the editor
    /// on that day — the natural "open" gesture, reusing `AppEvent::Edit`.
    #[test]
    fn double_clicking_a_calendar_day_queues_an_edit() {
        let mut app = day_app();
        let _ = render_to_string(&mut app, 120, 40);
        let (x, y, expected) = a_calendar_cell(&app);

        app.handle_mouse_event(click(x, y)).expect("first");
        app.handle_mouse_event(click(x, y)).expect("second");

        assert_eq!(app.active_date, expected);
        let queued: Vec<Event> = std::iter::from_fn(|| app.events.try_next()).collect();
        assert!(
            queued
                .iter()
                .any(|e| matches!(e, Event::App(AppEvent::Edit))),
            "a double click queues an Edit, got {queued:?}"
        );
    }

    #[test]
    fn two_clicks_at_different_cells_are_not_a_double_click() {
        let mut app = day_app();
        let _ = render_to_string(&mut app, 120, 40);
        let (x, y, _) = a_calendar_cell(&app);

        app.handle_mouse_event(click(x, y)).expect("first");
        app.handle_mouse_event(click(x, y + 1)).expect("second, elsewhere");

        let queued: Vec<Event> = std::iter::from_fn(|| app.events.try_next()).collect();
        assert!(
            !queued.iter().any(|e| matches!(e, Event::App(AppEvent::Edit))),
            "clicks at different cells must not open the editor"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --no-default-features --features tui double_click`
Expected: FAIL — no `Edit` is queued.

- [ ] **Step 3: Implement**

Add the constant near `STATUS_TTL`:

```rust
/// How close together two clicks at the same cell count as a double click.
///
/// Terminals do not report double clicks, so this is measured here. 400ms is
/// the common desktop default; shorter starts dropping deliberate doubles.
const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(400);
```

Add the field to `App`:

```rust
    /// When and where the last left-click landed, for detecting a double.
    ///
    /// `Instant` is already the status line's clock; this reuses it rather
    /// than reaching for a second time source.
    last_click: Option<(Instant, u16, u16)>,
```

with `last_click: None,` in `App::new`.

Add the helper:

```rust
    /// Is this click the second of a double at the same cell?
    ///
    /// Records the click either way, so three fast clicks read as one double
    /// and then a fresh single rather than two overlapping doubles.
    fn is_double_click(&mut self, x: u16, y: u16) -> bool {
        let now = Instant::now();
        let doubled = self
            .last_click
            .is_some_and(|(at, px, py)| px == x && py == y && now - at < DOUBLE_CLICK_WINDOW);
        self.last_click = if doubled { None } else { Some((now, x, y)) };
        doubled
    }
```

Then in `handle_mouse_event`, compute it once immediately after the `MouseEventKind::Down(MouseButton::Left)` arm falls through:

```rust
        let doubled = self.is_double_click(x, y);
```

and in the calendar branch and the project-list branch, queue an edit when doubled:

```rust
            self.go_to_date(date);
            if doubled {
                self.events.send(AppEvent::Edit);
            }
            return Ok(());
```

```rust
            widget.select_index(index);
            self.dirty = true;
            if doubled {
                self.events.send(AppEvent::Edit);
            }
            return Ok(());
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --no-default-features --features tui`
Expected: PASS.

- [ ] **Step 5: Clippy, fmt, commit**

```bash
cargo clippy --no-default-features --features tui --all-targets -- -D warnings && cargo fmt --all
git add src/tui/app.rs
git commit -m "feat(tui): open the editor on a double click"
```

---

## Task 10: Documentation and the full gate

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Document the mouse in the README**

Add a section after the keybinding table:

```markdown
### Mouse

The TUI captures the mouse by default:

| Gesture | Effect |
|---------|--------|
| Click a calendar day | Jump to that date |
| Click a bar in the weekly chart | Jump to that day |
| Click a project or rollup row | Select it |
| Double-click a day or a project | Open it in `$EDITOR` |
| Click the footer hint | Open the help popup |
| Wheel | Move the selection in whichever list is under the pointer |
| Click outside an open popup | Dismiss it |

Capture takes over the terminal's own click-drag text selection. Most
emulators still select on **Shift-drag**. To turn capture off entirely, set
`mouse = false` in the config file.
```

- [ ] **Step 2: Document the config key**

Find the README's configuration section (it lists `theme` and `daily_target_hours`) and add a `mouse` row/entry in the same style, defaulting to `true`.

- [ ] **Step 3: Update `CLAUDE.md`**

In the `tui/` submodules table, add rows in the table's existing order:

```markdown
| `terminal.rs` | `TerminalModes` — the one owner of which terminal modes are on (alternate screen, raw mode, mouse capture) — plus `with_suspended_terminal`, which the `$EDITOR` handover and Ctrl-Z both run their bodies inside |
| `layout_rects.rs` | `LayoutRects` — where each clickable region was drawn on the last frame, filled during render and read by mouse hit-testing |
```

In the `config.rs` row of the module table, add `mouse` to the list of TUI-only config keys read once into `TuiContext`.

- [ ] **Step 4: Run the real gate**

The site build must exist first:

```bash
test -d site/build || (cd site && yarn install && yarn build)
just gate
```

Expected: every command green across all three feature combinations. This is the first run of the `webapp`-only and default combinations against these changes; a failure there is most likely a `#[cfg(feature = "tui")]` missing from something new.

- [ ] **Step 5: Sanity-run the TUI against a scratch directory**

Never bare. Confirm it starts and exits cleanly:

```bash
TMP=$(mktemp -d) && cargo run -p cli -- --noedit --data-directory "$TMP" --help
```

- [ ] **Step 6: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "docs: document mouse support and Ctrl-Z suspend"
```

---

## Self-Review

**Spec coverage:**

| Spec section | Task |
|---|---|
| 1. `terminal.rs` module | Task 1 |
| 2. Ctrl-Z, `libc`, Windows fallback | Task 3 |
| 3. Start-up, teardown, panic hook | Task 2 |
| 4. Config and context | Task 2 |
| 5. Layout capture | Task 4 |
| 6. Hit-testing (calendar / bar / lists) | Tasks 5, 6, 7 |
| 7. Event handling, overlay-first, wheel | Task 8 |
| 7. Double-click | Task 9 |
| Testing items 1-2 (calendar characterization, Sunday) | Task 5 |
| Testing item 3 (variable-height rows) | Task 7 |
| Testing item 4 (bar chart at several widths) | Task 6 |
| Testing item 5 (suspend pause/resume) | Tasks 1, 3 |
| Testing item 6 (mode symmetry) | Task 1 |
| Testing item 7 (overlay-first) | Task 8 |
| Testing item 8 (config round-trip) | Task 2 |
| Testing item 9 (keymap/README) | Task 3 |
| Documentation | Task 10 |

**Names verified against the source** rather than inferred: `day_app()` / `selection()` test helpers (`app.rs:1979`, `1985`), `App::week_list` (319), `App::weekly_summary` (314), `week_projects()` (1767), `App::toggle_help` (753), `App::apply_sync_event` (607), `App::set_status(impl Into<String>)` (1252), `LIST_TITLE_ROWS` (`project_list.rs:39`), `TuiContext::for_test` (`context.rs:83`), and the ignored `print_readme_table` test (`keymap.rs:854`).

**Type consistency:** `TerminalModes` / `with_suspended_terminal` (Tasks 1, 3), `LayoutRects` / `hits` (Tasks 4, 8), `date_at` (Tasks 5, 6, 8), `index_at` / `select_index` (Tasks 7, 8) all match across the tasks that define and consume them.
