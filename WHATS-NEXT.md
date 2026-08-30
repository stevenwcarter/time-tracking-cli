# WHATS-NEXT.md — forward-looking work suggestions

Last triage: 2026-08-28 against `main` @ fe7f2b2.

> **For future sessions reading this file:** when an item listed here ships,
> strip it from this file in the same change that ships it. The list is
> intended to reflect open opportunities only; shipped items shouldn't linger.
> This keeps the file's signal-to-noise high for the next whats-next pass.

## How to use this file
- Check `[x] execute` on items to hand them to `/ship-it --ask` for implementation.
- Check `[x] skip` on items to file them into this file's own Skip archive and never re-flag them.
- Items left unchecked persist in WHATS-NEXT.md for the next run.
- Ranking is value-to-effort (bang-for-buck): effort IS folded into the score.
- When ready, run `/whats-next --execute`.

## Critical

_(none)_

## High

### W29. Add Ctrl-Z suspend/resume via a shared terminal-suspension helper (TUI/lifecycle — src/tui/app.rs:140)
- Lens: terminal-robustness
- Score: 1.50 (value 3 / effort M)
- What: `run_editor` hand-rolls the suspend dance — pause events, `LeaveAlternateScreen`, `disable_raw_mode`, run the editor, `EnterAlternateScreen`, `enable_raw_mode`, `terminal.clear()`. Extract it into a `with_suspended_terminal(|| ...)` helper and reuse it for a new Ctrl-Z binding that raises `SIGTSTP` after restoring the cooked terminal and re-enters the TUI on `SIGCONT`. The key-handling match at app.rs:229 already special-cases Ctrl-C, so Ctrl-Z is a natural sibling.
- Why: Raw mode means the terminal never generates SIGTSTP, so Ctrl-Z inside the TUI is swallowed and there is currently no way to drop back to the shell and `fg` again — the user has to quit and relaunch, losing the selected date. Job control is reflexive for a developer running this from a shell all day. Factoring the suspension into one helper also gives a single place that later has to know about mouse capture, bracketed paste and any other terminal mode the TUI adopts, instead of duplicating the sequence per feature.
- Blocked by: —
- Notes: W30 needs exactly this helper so an editor session does not inherit mouse capture; do this one first.
- [x] execute   [ ] skip
> in-flight (handed to /ship-it on 2026-08-29)

### W30. Add mouse support: click a calendar day, a bar, or a project row (TUI/input — src/tui/event.rs:81)
- Lens: terminal-robustness
- Score: 1.50 (value 3 / effort M)
- What: The crossterm `EventStream` is never put into mouse-capture mode and `App::run` matches only `Event::Key` (app.rs:87 discards everything else), so clicks and wheel scrolls do nothing. Enable mouse capture alongside the alternate-screen setup, disable it in the same suspension helper used for `$EDITOR`, add an `Event::Mouse` arm, and store the calendar, chart and list `Rect`s on `App` during render so a click can be hit-tested: click a calendar day or a weekly bar to jump to that date, click a project row to select it, wheel-scroll the list.
- Why: The user is often not in a keyboard-driven flow when they open this — they glance at the month and want a specific day. Every modern terminal emulator forwards mouse events, and clicking the calendar day you are already looking at is the most obvious interaction the calendar suggests but does not offer. Wheel-scrolling the project list is likewise reflexive on a day with many entries.
- Blocked by: —
- Notes: Mouse capture must be torn down and re-established in the same place the alternate screen is, or the editor session inherits capture — so do this after or together with W29's suspension helper.
- [x] execute   [ ] skip
> in-flight (handed to /ship-it on 2026-08-29)

## Medium

_(none)_

## Low

_(none)_

## Skip (do not re-flag in future runs)

### W4. Add a write path to DataService for in-TUI entry (TUI/editing — src/data_svc.rs:115)
- Lens: unblock-debt
- Score: 4.00 (value 4 / effort S)
- What: Add `DataService::write_day(&self, date: &Date, content: &str) -> Result<PathBuf>` and `append_line(&self, date: &Date, line: &str)` that create the file from the template if missing, write via `tokio::fs`, honour the configured prefix/suffix region, and invalidate the cache — the exact sequence currently inlined in `Mutation::update_file_content` at src/graphql.rs:112-138. Repoint the GraphQL resolver at it.
- Why: `DataService` exposes `create_day_file_if_not_exists`, `read_day`, `parse_day` and cache invalidation but nothing that writes, so `run_editor` is the TUI's only mutation path and the only in-process write in the codebase is inlined in a resolver that builds its own path from `get_time_tracking_dir()` and thereby bypasses the config data-directory override. Lifting it into the service removes that duplicated, override-ignoring path construction and is the enabling half of W22.
- Blocked by: —
- Notes: Non-TUI seam justified by a named TUI feature: in-TUI quick entry / punch in-out without shelling to `$EDITOR`. Ships together with W22.
- [ ] execute   [x] skip

### W22. Append a time entry from inside the TUI with `a` (TUI/editing — src/tui/app.rs:140)
- Lens: feature-gap
- Score: 2.00 (value 4 / effort M)
- What: `run_editor` is the TUI's only mutation path — it pauses the event loop, leaves the alternate screen, shells out and reloads. Add an `a` prompt that reads one line such as "2-3:30 client-bd" plus an optional note and appends it inside the active date's configured prefix/suffix region, or writes a stub entry for the current clock time and opens `$EDITOR` positioned on that line (which needs a line-position argument on `open_in_editor` at src/editor.rs:21, today passing only the path), then invalidates that date's cache.
- Why: Recording a block of time you just finished is the highest-frequency mutation a time tracker has, and the web surface already edits a day in place via DateEditor and `updateFileContent` while the TUI cannot. Today `e` opens the raw markdown at line 1 and the user must scroll to the end of the tracking region, read a clock, type the range by hand, save and quit — five steps for one entry, heavy enough that users defer logging, which is exactly how the dead-time gaps get created.
- Blocked by: —
- Notes: W4 is the data-layer half (`DataService::append_line` honouring `Config::get_prefix()`/`get_suffix()` at src/config.rs:381,386) and should ship with it; prefix/suffix placement is the fiddly part. Shares the input-prompt component with W16, so whichever lands second is cheap, and W20 is what makes key capture clean.
- [ ] execute   [x] skip
