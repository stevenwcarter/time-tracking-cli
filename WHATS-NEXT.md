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

### W1. Add week, month, and page-step date motions to the keymap (TUI/navigation — src/tui/app.rs:236)
- Lens: feature-gap
- Score: 5.00 (value 5 / effort S)
- What: Extend `handle_key_events`, which today maps only `h`/`l`/arrows to `PreviousDate`/`NextDate` and `t` to `Today`, with coarse motions: `[`/`]` or `PageUp`/`PageDown` for month stepping via the existing `month_offset` helper at app.rs:257, and `H`/`L` or `{`/`}` for week stepping via `get_week_dates`. Keep the new keys clear of `g`/`G`/`j`/`k`, which `ProjectListWidget::handle_key_event` intercepts first at project_list.rs:76-83.
- Why: The TUI renders a three-month populated-date calendar that invites the user to go look at a day last month, but the only route there is thirty to forty presses of `h`, each firing a full three-month populated-date rescan. The affordance the calendar promises (browse the month, spot the populated days, jump to one) is not reachable with the current keymap.
- Blocked by: —
- Notes: Month data is already prefetched — `load_data_for_active_date` scans prev/current/next month at app.rs:179-184 — so month stepping mostly renders data the app already holds. Split out of the same lens finding as W16; this half is keymap arms only and ships alone.
- [ ] execute   [ ] skip

### W2. Show dead time and parser warnings in the TUI day header (TUI/day-summary — src/tui/project_list.rs:177)
- Lens: feature-gap
- Score: 5.00 (value 5 / effort S)
- What: `ProjectListWidget::new` (project_list.rs:34) copies only `start_time`, `end_time` and `total_minutes` off the `TimeTrackingData` it is handed, and `render_header` prints just those three. Also capture `data.dead_time_minutes` and `data.warnings`, render a dead-time line next to Working Time using the existing `formatted_dead_time_minutes()`/`formatted_dead_decimal()` helpers on the parser type, and render a warnings block styled by the same sub-90-minute warn / 90-minute-plus error threshold used in `format_day_summary_impl`.
- Why: Dead-time detection is a headline README feature computed on every parse, yet the one surface the user keeps open all day is the only one that hides it, and a mistyped time range is the most common data-entry mistake. Someone reconciling a day before writing a timesheet has to quit the TUI and run `ttcli` on stdout to learn they have a two-hour unaccounted gap or an unparsed line silently skewing the number they are about to paste.
- Blocked by: —
- Notes: Data is already loaded into `App.data` at src/tui/app.rs:198-201, so no new fetch. Parity references: the dead-time and warnings sections of `format_day_summary_impl` in src/display/mod.rs, `dead_header`/`dead_warn`/`warnings_header` in src/display/plain.rs, and `DayData.dead_time_hours`/`DayData.warnings` at src/web.rs:52,54. Pairs with W15 — warnings say something is wrong, the raw view shows what.
- [ ] execute   [ ] skip

### W3. Add a status line and move clipboard IO out of the widget (TUI/feedback — src/tui/app.rs:42, src/tui/project_list.rs:134)
- Lens: unblock-debt
- Score: 4.00 (value 4 / effort S)
- What: Have `ProjectListWidget::handle_key_event` return an intent such as `Handled::Emit(AppEvent::CopyToClipboard(String))` instead of constructing a `copypasta::ClipboardContext` itself; let `App` own the single clipboard context plus a `status: Option<(String, Instant)>` field rendered in the existing footer at project_list.rs:188 and expired on `tick()`. Feed it from the clipboard path (success "Copied 4 notes for admin" / failure "Clipboard unavailable"), the two `tracing::warn!` swallows at app.rs:75 and app.rs:91, and a "Loading..." marker while `load_data_for_active_date` is in flight.
- Why: `Enter` yanking notes is the TUI's headline action and it produces zero feedback either way — on a headless box or over SSH with no clipboard backend it silently does nothing, the failure going only to the rolling log file the user cannot see because the alternate screen owns the terminal. A failed load renders identically to an empty day and `r` gives no sign it did anything. Burying the side effect in the widget also leaves nowhere to put a toast line and no way for other actions to reuse the copy path.
- Blocked by: —
- Notes: The footer already exists and currently shows only "? for help". Unblocks W10 (copy the whole day as markdown) and later copy targets driven by the `DisplayFormatter` impls, plus an action log once in-TUI writes exist.
- [ ] execute   [ ] skip

### W4. Add a write path to DataService for in-TUI entry (TUI/editing — src/data_svc.rs:115)
- Lens: unblock-debt
- Score: 4.00 (value 4 / effort S)
- What: Add `DataService::write_day(&self, date: &Date, content: &str) -> Result<PathBuf>` and `append_line(&self, date: &Date, line: &str)` that create the file from the template if missing, write via `tokio::fs`, honour the configured prefix/suffix region, and invalidate the cache — the exact sequence currently inlined in `Mutation::update_file_content` at src/graphql.rs:112-138. Repoint the GraphQL resolver at it.
- Why: `DataService` exposes `create_day_file_if_not_exists`, `read_day`, `parse_day` and cache invalidation but nothing that writes, so `run_editor` is the TUI's only mutation path and the only in-process write in the codebase is inlined in a resolver that builds its own path from `get_time_tracking_dir()` and thereby bypasses the config data-directory override. Lifting it into the service removes that duplicated, override-ignoring path construction and is the enabling half of W22.
- Blocked by: —
- Notes: Non-TUI seam justified by a named TUI feature: in-TUI quick entry / punch in-out without shelling to `$EDITOR`. Ships together with W22.
- [ ] execute   [ ] skip

### W5. Auto-refresh the TUI when the day file changes on disk (TUI/refresh — src/tui/app.rs:248)
- Lens: feature-gap
- Score: 4.00 (value 4 / effort S)
- What: `App::tick()` is an empty no-op even though `EventHandler` delivers `Event::Tick` at 30 FPS (src/tui/event.rs:139-163). Have `tick` count frames and roughly once per second stat the active date's file with `std::fs::metadata`, comparing mtime against a stored `SystemTime`, and on change send `AppEvent::ReloadFromDisk`. `DataService::get_cached_content` (src/data_svc.rs:202) already does the same mtime comparison, so unchanged reads stay served from cache.
- Why: This is the last unchecked TUI line in TODO.md ("Poll for changes in the file and update TUI for live preview"). The intended workflow is the TUI open in one pane while the user edits the same markdown file in Obsidian or neovim (the repo ships a neovim plugin), and today they must remember to press `r` while the bar chart and totals silently go stale. Polling avoids adding `notify` as a dependency.
- Blocked by: —
- Notes: Tension with W7, which proposes dropping the tick entirely — the two are compatible only if the tick survives at a low rate, so implement them together and land on a 1 Hz tick that drives this poll while leaving idle redraw cost at zero. W24 is the more robust form of this feature (a watcher task posting events) and supersedes the polling loop if it lands first.
- [ ] execute   [ ] skip

### W6. Document every implemented key in the help popup and README, and make the popup modal (TUI/discoverability — src/tui/widgets/help_popup.rs:12)
- Lens: feature-gap
- Score: 4.00 (value 4 / effort S)
- What: The hardcoded cheatsheet string lists j/k, g/G, r, e, f and Enter. Add the bindings it omits — `t`/`T` (today, app.rs:235), `h`/`l` and Left/Right (previous/next date, app.rs:236-237), `?` itself (app.rs:238) and `q`/`Esc`/`Ctrl-C` (quit, app.rs:228-231) — to both the popup and the identical six-row README table at README.md:192-199. Also make the popup modal: while `show_help` is true, consume `Esc`/`q`/`?`/any key to close it instead of falling through to `AppEvent::Quit`, and render it in zoomed mode, since ui.rs:13-21 returns before the `show_help` check so `?` does nothing visible while `f` is active.
- Why: Date navigation is the TUI's core browsing action and appears in neither the popup nor the README, so a user who reads both learns no way to look at yesterday and may conclude the TUI only shows today. Worse, the key users reflexively press to dismiss a modal, `Esc`, quits the whole application, so trying to close help drops them back to the shell. This is a near-zero-cost multiplier on features already shipped.
- Blocked by: —
- Notes: Three sources disagree — app.rs implements nine bindings, help_popup.rs documents six, README documents the same six. W25 is the structural fix that stops the drift recurring by generating both from one table; this item corrects the text now, W25 makes the correction durable.
- [ ] execute   [ ] skip

### W7. Redraw only on state change instead of unconditionally at 30 FPS (TUI — src/tui/app.rs:78)
- Lens: scale-perf
- Score: 4.00 (value 4 / effort S)
- What: Introduce a `dirty: bool` (or a `needs_redraw()` derived from a state generation counter) on `App`, set on startup, after every handled key or app event and after `load_data_for_active_date` applies results; call `terminal.draw(...)` only when it is set, then clear it. Handle `Event::Resize` explicitly as a redraw trigger. Once redraw is decoupled from the tick, drop `TICK_FPS` from 30 to roughly 1-4.
- Why: `run()` calls `terminal.draw` at the top of every loop iteration and the loop turns once per event, so the 30 FPS tick in src/tui/event.rs:9 forces thirty full re-renders per second forever with zero input and no animation. Each render rebuilds a `CalendarEventStore` from up to ninety populated dates, re-derives the week's seven bar labels, and reallocates a `String` per project plus one per note bullet. The target user leaves this open all day on a laptop; render-on-change takes idle cost to approximately zero and makes the per-frame cost of W12 and W13 irrelevant while idle.
- Blocked by: —
- Notes: Do not remove `Event::Tick` outright — W5 wants a roughly 1 Hz tick for mtime polling and W3 expires its status line on `tick()`. A low-rate tick serves all three; the dirty flag is what makes keeping the tick cheap.
- [ ] execute   [ ] skip

### W8. Scale the weekly bar chart to the week's data, not terminal height (TUI/chart — src/tui/widgets/weekly_bar_chart.rs:127)
- Lens: terminal-robustness
- Score: 4.00 (value 4 / effort S)
- What: `max_value` is currently `(content_height as u64 * 10).max(160)`, so the y-axis ceiling is derived from how many rows the widget happens to get, floored at sixteen hours. Replace it with a data-driven scale: take the maximum daily value in the loaded week, round up to a sensible increment, clamp to a configurable full-scale such as an eight-hour target, and optionally draw a goal marker row at that value so a full day is identifiable at a glance in both the inline and `f`-zoomed views.
- Why: An eight-hour day renders at half height in the inline chart and shrinks further the taller the terminal gets — zoom with `f` on a 44-row terminal and the ceiling lands near forty hours, turning a full working day into a stub about a fifth of the frame. The weekly chart is the tool's main at-a-glance signal about whether the week is on track, and today its meaning changes when you resize your window.
- Blocked by: —
- Notes: The total-hours overlay `Rect` at line 166 is computed from `area` rather than the block's inner area and uses `total_text.len()` (bytes) as a column count; folding it into a right-aligned `Block::title_top(...)` while touching this widget removes the hand-rolled geometry.
- [ ] execute   [ ] skip

### W9. Show the active date on the project-list pane (TUI/layout — src/tui/ui.rs:51)
- Lens: ux
- Score: 4.00 (value 4 / effort S)
- What: Render `active_date`, formatted with the weekday (for example `Thu 2026-08-27`), as a persistent title on the project-list pane. The bordered block built at ui.rs:51 with `self.active_date.format(DATE_FORMAT)` is attached only to the "No data found" paragraph in the `else` branch, so when `project_list_widget` is `Some` the block is dropped unused and the date string never reaches the screen.
- Why: After a few `h`/`l` presses the user has no textual confirmation of which day they are looking at and must decode the highlighted cell in the 24-column calendar or the highlighted bar's day-of-month label. When notes get copied into a timesheet against the wrong day the cost is a wrong billing entry. The date is already computed and formatted — it is simply discarded.
- Blocked by: —
- [ ] execute   [ ] skip

### W10. Yank the whole day summary to the clipboard with `y` (TUI/clipboard — src/tui/project_list.rs:134)
- Lens: feature-gap
- Score: 4.00 (value 4 / effort S)
- What: Add a `y` binding (and `Y` for the week once W17 lands) that copies the formatted day summary to the clipboard via `Config::get().get_formatter().day_summary(&content, "", prefix, suffix)` — the `DisplayFormatter` trait already declares non-printing `day_summary`/`weekly_projects`/`weekly_totals` String-returning variants next to every `display_*` method for exactly this purpose — reusing the clipboard block already in `copy_selected_notes_to_clipboard`.
- Why: The stated workflow is pasting per-project totals into a timesheet or standup note, but today `Enter` yanks one project's bullets without its hours, so producing a standup paste means N separate `Enter` presses or quitting the TUI to run `ttcli --formatter markdown`. A markdown day summary is one keypress away from data and plumbing that already exist.
- Blocked by: —
- Notes: `MarkdownDisplayFormatter` and `PlainDisplayFormatter` are already constructible via `Config::get_formatter()` (src/config.rs:389), and the TUI currently never touches the formatter layer at all. Cleaner on top of W3, which moves clipboard IO into `App` and gives the copy a visible confirmation.
- [ ] execute   [ ] skip

### W11. Give the empty-date screen a call to action and keep the help hint (TUI/empty-state — src/tui/ui.rs:58)
- Lens: ux
- Score: 3.00 (value 3 / effort S)
- What: Replace the bare "No data found for date" paragraph with a state that names the day, distinguishes "no file yet" from "file exists but has no parsed entries", and prompts the next action ("press e to create and edit this day", "press t for today"). Also render the footer hint in this branch — the "? for help" footer lives inside `ProjectListWidget::render` at project_list.rs:188, so it vanishes exactly on the empty screen.
- Why: This is the first screen a new user sees if they launch `--tui` before writing anything, and the screen they hit on every weekend or future date. It states a negative fact, offers no way forward, and simultaneously removes the only on-screen pointer to the help popup, at the moment the user most needs the keymap.
- Blocked by: —
- Notes: Distinguishing the two empty cases pairs with W15, which renders the raw text when a file exists but parses to nothing.
- [ ] execute   [ ] skip

### W12. Hoist week-start derivation out of the bar chart render path (TUI/widgets — src/tui/widgets/weekly_bar_chart.rs:46)
- Lens: scale-perf
- Score: 3.00 (value 3 / effort S)
- What: Store `week_start_day: Weekday` and `week_dates: [Date; 7]` on `App`, computed once in `load_data_for_active_date` (which at app.rs:188 already computes exactly these and then discards them), and pass them into `WeeklyBarChart::new`. The invalidation trigger is any change to `active_date`, the same place the data load already happens; `Config` is a process-lifetime `OnceLock`, so `week_start_day` never needs recomputing after startup.
- Why: `prepare_bars` reaches into the global `Config` singleton and re-runs `parse_weekday` (a chain of case-insensitive string comparisons) plus `get_week_dates` (a heap-allocated `Vec<Date>`) on every frame, for a value that can only change when the active date changes. Under the current unconditional 30 FPS redraw that is thirty times the necessary work per second, and even after W7 it is work re-derived per render rather than per state change, duplicating a computation the load path already performed.
- Blocked by: —
- Notes: Best done together with W7; on its own the win is modest. It also removes one of the in-render `Config::get()` calls that W19 eliminates wholesale.
- [ ] execute   [ ] skip

### W13. Precompute project-list line content once per data load (TUI/project-list — src/tui/project_list.rs:202)
- Lens: scale-perf
- Score: 3.00 (value 3 / effort S)
- What: Build the rendered `Text`/`ListItem` body for each project once in `ProjectListWidget::new`, storing it on `ProjectItem` alongside the raw fields, and have `render_list` reuse it while applying only the alternating row background at render time. No extra invalidation is needed — the widget is already reconstructed from scratch whenever the day's data loads.
- Why: `render_list` rebuilds `Vec<ListItem>` every frame, and `From<&ProjectItem> for ListItem` at project_list.rs:233 does a `format!` for the project header plus one `format!` and `push_str` per note bullet into a fresh `String` per project. Cost scales linearly with projects times notes per day — exactly the dimension that grows as the user logs longer, more detailed days — and is currently paid thirty times a second regardless of whether anything changed. The list content is a pure function of `TimeTrackingData`, which only changes on a reload.
- Blocked by: —
- Notes: The `ListState` (selection and scroll) must stay mutable and outside the cached body; only the item text is memoized. W28 changes how these lines are built and makes them width-dependent, so sequence the two deliberately to avoid rework.
- [ ] execute   [ ] skip

### W14. Stop printing the webserver ctrl-c message on a TUI-only launch (TUI/startup — cli/src/main.rs:91)
- Lens: ux
- Score: 3.00 (value 3 / effort S)
- What: Print the "Other jobs are running (webserver or tui), press ctrl-c to quit (webserver)" line only when the webserver task was actually spawned; for a TUI-only run print nothing (the TUI owns the screen and documents its own quit key) or a TUI-specific line emitted before the alternate screen is entered.
- Why: In `--tui` mode this line is written to stdout while the spawned TUI task is concurrently entering the alternate screen, so it either corrupts the first frame or lurks on the normal screen as the first thing the user sees after quitting. Either way it teaches ctrl-c as the quit key and blames a webserver that is not running, when `q` is the actual quit key — a confusing first and last impression of the only surface that ran.
- Blocked by: —
- [ ] execute   [ ] skip

### W15. View the raw day-file text without leaving the TUI (TUI/day-summary — src/tui/ui.rs:55)
- Lens: feature-gap
- Score: 3.00 (value 3 / effort S)
- What: Add a `v` toggle that renders the raw file text in a scrollable `Paragraph` fed by `DataService::get().read_day(&self.active_date)` — the same call the web surface exposes as the `fileContentForDate` query at src/graphql.rs:35 — instead of the bare "No data found for date" paragraph being the only response to an unparseable day.
- Why: The prefix/suffix fencing feature means a day file can be full of text yet parse to zero entries, and the TUI's response is an unhelpful "No data found" with no way to see why short of suspending to `$EDITOR`. Users on the Obsidian daily-note setup hit this whenever a fence marker gets moved or a time range is mistyped.
- Blocked by: —
- Notes: Pairs with W2 (warnings tell you something is wrong, the raw view shows you what) and with W11's "file exists but has no entries" state.
- [ ] execute   [ ] skip

### W16. Add a jump-to-date prompt that accepts natural-language dates (TUI/navigation — src/tui/app.rs:236)
- Lens: feature-gap
- Score: 2.50 (value 5 / effort M)
- What: Add a `:` or `d` prompt that routes keys to a text buffer instead of `handle_key_events` and feeds the typed string to `interim::parse_date_string(&s, now, Dialect::Us)` — the exact call the CLI already makes at src/config.rs:301 to accept `--date 'last friday'`. The new piece is a small input-mode state; the parsing and the date plumbing already exist.
- Why: The CLI user can type `ttcli 'last friday'`, but the TUI user, looking at a rendered calendar of that very month, cannot jump to a visible date. Coarse motions shorten the walk; only a prompt makes an arbitrary date one action away, and every intermediate step today fires a full three-month populated-date rescan.
- Blocked by: —
- Notes: Split out of the same lens finding as W1 because the effort differs materially — W1 is keymap arms, this needs a new input component. That component is shared with W22's append prompt, so whichever lands second is cheap, and W20's mode enum is what makes key capture clean rather than a fourth boolean flag.
- [ ] execute   [ ] skip

### W17. Add a weekly per-project rollup pane to the TUI (TUI/weekly — src/tui/app.rs:194)
- Lens: feature-gap
- Score: 2.50 (value 5 / effort M)
- What: The TUI's only weekly view is `WeeklyBarChart`, fed by `DataService::get_weekly_data`, which returns `HashMap<Date, u32>` — day totals and nothing else. Add a week mode (for example `w` toggling `App.week_mode`) that swaps the project list for a week-aggregated project list plus a week total and week dead-time header, with `Enter` yanking that project's week notes and hours. Reuse the aggregation already written twice: the `week_projects: HashMap<String, (u32, Vec<String>)>` loop in `show_weekly_summary` (src/display/mod.rs) and `aggregate_week_days` at src/web.rs:279.
- Why: The consultant's actual weekly billing question is "how many hours did client-bd get this week", and timesheets are filed weekly. That answer exists behind `ttcli --week` and behind the SPA's WeeklySummary page but not in the TUI, where the bar chart teases weekly data yet only answers "how long did I work Tuesday" — so the user quits the TUI, runs the CLI, then relaunches to keep browsing. This is the single largest capability the TUI is missing relative to its siblings, and the week's data is already loaded on every date change.
- Blocked by: —
- Notes: W18 is the data-layer half that lifts the aggregation out of the stdout printer into `DataService`; ship the two together, since this pane is the consumer that justifies that extraction. If W18 is skipped, the cheapest standalone path is a `DataService::get_weekly_projects(&[Date]) -> Vec<(String, u32, Vec<String>)>` alongside `get_weekly_data` (data_svc.rs:179), reusing the same per-day `parse_day` JoinSet but keeping `data.projects` instead of discarding them.
- [ ] execute   [ ] skip

### W18. Extract weekly per-project aggregation out of the stdout printer (TUI/weekly — src/display/mod.rs:189)
- Lens: unblock-debt
- Score: 2.50 (value 5 / effort M)
- What: Extract the collection loop in `show_weekly_summary` (the `week_projects: HashMap<String, (u32, Vec<String>)>` fold plus totals, dead minutes and warnings) into a pure `DataService::get_weekly_summary(&[Date]) -> Result<WeeklySummary>` returning total minutes, dead minutes, a `Vec<WeeklyProject>`, warnings and a per-day map. `show_weekly_summary` then becomes a formatter call over that struct and `DataService::get_weekly_data` becomes a thin projection of it.
- Why: The per-project weekly rollup already exists but is computed inline inside `show_weekly_summary`, interleaved with `println!` and `formatter.display_*` calls, so the TUI cannot reach it and is left with bare per-day minutes. That blocks the feature closest to the primary user's job: a weekly project-totals pane in the `f` zoom view, a copy-the-week's-totals-as-markdown action reusing the existing `MarkdownDisplayFormatter`, and surfacing week-level parse warnings in the TUI, which today are visible only on CLI stdout.
- Blocked by: —
- Notes: Non-TUI seam justified by named TUI features. The CLI path keeps identical output because the formatter calls stay in display/mod.rs. This is the enabling half of W17 — they ship together.
- [ ] execute   [ ] skip

### W19. Inject a TuiContext instead of reading the global Config singleton (TUI — src/tui/app.rs:65)
- Lens: unblock-debt
- Score: 2.50 (value 5 / effort M)
- What: Introduce an owned `TuiContext { week_start_day: Weekday, data_dir: PathBuf, formatter: Formatter, theme: Theme }` built once in `tui()` from `Config::get()`, stored on `App` and passed by reference into widget constructors. `App::new(ctx)` replaces the no-arg constructor, `WeeklyBarChart::new(active_date, week_start_day, theme)` replaces the in-render `Config::get()` call at weekly_bar_chart.rs:46, and `WidgetColors`' associated consts become fields on `Theme`. Make the context mutable behind `&mut self` so an in-TUI action can change it and trigger a reload.
- Why: `App::new()` takes zero arguments and reaches for a `OnceLock` initialised by `Config::init(true)`, which runs `Args::parse()` — so config is immutable for the process lifetime and constructing an `App` outside the real binary parses argv. That blocks a settings pane that flips `week_start_day` live (the bar chart and week query would keep the frozen value), a switch-data-directory command inside the TUI, and any rendering test of `ui.rs` against ratatui's `TestBackend`; there are currently zero tests under src/tui, so every TUI change is verified by hand.
- Blocked by: —
- Notes: This is the prerequisite plumbing for W21 — styles are hardcoded across colors.rs, project_list.rs:7-10 (a second private copy of the row backgrounds) and an inline `SLATE.c400` italic in calendar.rs, and with no `Theme` in the context there is no seam to hang a config-driven theme on. W12 removes one of the same in-render `Config::get()` calls cheaply if this lands later.
- [ ] execute   [ ] skip

### W20. Replace view booleans with a mode enum and focus-aware key dispatch (TUI/navigation — src/tui/ui.rs:13)
- Lens: unblock-debt
- Score: 2.50 (value 5 / effort M)
- What: Replace `zoom_bar: bool` and `show_help: bool` with `mode: Mode` (Day, ZoomedWeek, plus future DateJump, Settings, Search) and an `overlay: Option<Overlay>` for modal layers. Give each mode a `render(&mut self, area, buf)` and a `handle_key(&mut self, key) -> Handled`, so `impl Widget for &mut App` becomes a dispatch on `mode` and `handle_key_events` consults the topmost overlay first instead of unconditionally forwarding every key to `project_list_widget`.
- Why: `ui.rs::render` early-returns on `zoom_bar`, lays out the day view otherwise, and paints `HelpPopup` last if `show_help`, while `handle_key_events` (app.rs:212) forwards every keypress to the project list before matching app keys, with no awareness of what is on screen — so with the help popup open, `j`/`k` still move the hidden list and `h`/`l` still change the date behind it. Any view that needs to capture text is unbuildable, and a fourth and fifth boolean would make the render function's branch matrix combinatorial.
- Blocked by: —
- Notes: This is the clean substrate for W16's date-jump prompt, W22's append prompt and W6's modal help; each of those is a one-off hack without it.
- [ ] execute   [ ] skip

## High

### W21. Add a config-driven theme with light, dark, and no-color presets (TUI/theming — src/tui/widgets/colors.rs:9)
- Lens: terminal-robustness
- Score: 2.00 (value 4 / effort M)
- What: Replace the hardcoded `WidgetColors` consts (`BLUE.c300`, `Color::Red`, `SLATE.c400`) and the equally hardcoded styles in project_list.rs (`SLATE.c950`/`SLATE.c900` row backgrounds, `BLUE.c800` header, `BLUE.c950` selection) with a `Theme` struct resolved once at startup and threaded to the widgets. Add an optional theme table to the TOML config — `Config` already round-trips through `toml::to_string_pretty` plus `write_config_comments`, so a new optional field is additive — with named dark, light and none presets plus per-role overrides, have the none preset emit no fg/bg at all so the terminal palette shows through, honour `NO_COLOR` by forcing that preset, and fall back to the sixteen ANSI colors when `COLORTERM` does not advertise truecolor.
- Why: On a light-background terminal (Solarized Light, default macOS Terminal, most IDE-embedded shells) the near-black slate row stripes sit as dark blocks over a white page and the pale blue calendar days lose contrast; over SSH to an 8/16-color `TERM` the truecolor values are approximated unpredictably. A theme seam plus a palette-inheriting preset makes the tool legible everywhere the user works instead of only in a dark 24-bit terminal, and is the prerequisite for any later high-contrast or accessibility option.
- Blocked by: —
- Notes: Grep confirms no occurrence of `NO_COLOR`, `COLORTERM` or any theme concept anywhere in the repo. Styles are split across colors.rs, project_list.rs module-level consts and an inline italic in calendar.rs, so the seam should absorb all three. Depends on W19 for the context that carries the resolved `Theme`; doing it first means threading the theme by hand.
- [ ] execute   [ ] skip

### W22. Append a time entry from inside the TUI with `a` (TUI/editing — src/tui/app.rs:140)
- Lens: feature-gap
- Score: 2.00 (value 4 / effort M)
- What: `run_editor` is the TUI's only mutation path — it pauses the event loop, leaves the alternate screen, shells out and reloads. Add an `a` prompt that reads one line such as "2-3:30 client-bd" plus an optional note and appends it inside the active date's configured prefix/suffix region, or writes a stub entry for the current clock time and opens `$EDITOR` positioned on that line (which needs a line-position argument on `open_in_editor` at src/editor.rs:21, today passing only the path), then invalidates that date's cache.
- Why: Recording a block of time you just finished is the highest-frequency mutation a time tracker has, and the web surface already edits a day in place via DateEditor and `updateFileContent` while the TUI cannot. Today `e` opens the raw markdown at line 1 and the user must scroll to the end of the tracking region, read a clock, type the range by hand, save and quit — five steps for one entry, heavy enough that users defer logging, which is exactly how the dead-time gaps get created.
- Blocked by: —
- Notes: W4 is the data-layer half (`DataService::append_line` honouring `Config::get_prefix()`/`get_suffix()` at src/config.rs:381,386) and should ship with it; prefix/suffix placement is the fiddly part. Shares the input-prompt component with W16, so whichever lands second is cheap, and W20 is what makes key capture clean.
- [ ] execute   [ ] skip

### W23. Cache parsed day data in DataService, not just raw file content (TUI — src/data_svc.rs:100)
- Lens: scale-perf
- Score: 2.00 (value 4 / effort M)
- What: Extend `CacheEntry` to hold the parsed `TimeTrackingData` — or at minimum a small derived summary of `has_data: bool` and `total_minutes: u32` — keyed by date plus file mtime, so `parse_day` and `check_date_has_data` return memoized results instead of re-running `time_tracking_parser::parse_time_tracking_data` on every call. Invalidation is the existing mtime comparison plus `invalidate_date`, which the TUI already calls after an `$EDITOR` session.
- Why: A single arrow-key press triggers `find_populated_dates` over about ninety dates plus `get_weekly_data` over seven, so roughly ninety-seven `parse_day` calls. The cache stores only the file `String`, so all of those markdown parses re-run on every keypress even on a full cache hit, each one's cost scaling with the length of the day file. The CLI and GraphQL surfaces call `parse_day` a handful of times per process; only the long-lived TUI amortizes a parse cache, and only the TUI pays the multiplier on every navigation. This is what will make holding `l`/`h` sluggish as days get longer.
- Blocked by: —
- Notes: `TimeTrackingData` would need `Clone`, or storage behind an `Arc`, to hand out cached copies; caching just the has-data and total-minutes summary is a smaller change that still covers both hot TUI callers. Pairs with W27 — the directory listing tells you which files exist, this keeps you from re-parsing them.
- [ ] execute   [ ] skip

### W24. Expose a cloneable AppEvent sender and move loads off the event loop (TUI/event-loop — src/tui/event.rs:98)
- Lens: unblock-debt
- Score: 2.00 (value 4 / effort M)
- What: Expose a cloneable `AppEventSender` from `EventHandler` by wrapping the existing private `mpsc::UnboundedSender<Event>` and handing out clones, so spawned tasks can push `Event::App(..)`. Then move `load_data_for_active_date` off the event-loop thread: on a date change set a loading flag, spawn the three-way `tokio::join!` with the sender, and apply the result via a new `AppEvent::DataLoaded(..)`.
- Why: `EventHandler::send` takes `&mut self` and the sender is private, so only `App` can emit an app event while it holds the loop — a `notify` watcher or poll task is unimplementable without a sender clone, which matters precisely because the intended workflow is editing the day file in neovim while the TUI is open. The same seam is what makes loads blocking: `handle_app_event` awaits the load inline, so holding `l`/`h` to scrub dates stalls the UI on three concurrent file scans with no way to render a spinner or cancel a superseded load.
- Blocked by: —
- Notes: This is the robust form of W5's mtime polling (a background watcher posting events rather than a stat inside `tick`), and it is what lets W3 render a real loading state instead of a frozen frame.
- [ ] execute   [ ] skip

### W25. Generate the keymap, help popup, and README from one binding table (TUI/keymap — src/tui/app.rs:227)
- Lens: unblock-debt
- Score: 2.00 (value 4 / effort M)
- What: Define a single `const BINDINGS: &[Binding]` where `Binding { keys: &[(KeyCode, KeyModifiers)], event: AppEvent, group: &str, description: &str }`, drive `handle_key_events` by looking a key up in it, and have `HelpPopup` render its rows from the same table. Split it per-mode once W20's mode enum lands so the help shows only what is live.
- Why: The real keymap is a `match key_event.code` arm in app.rs plus a second match in `ProjectListWidget::handle_key_event`; the user-facing help is a hardcoded string literal at help_popup.rs:14-19; the README has a third copy. They have already drifted by four bindings. A binding table unblocks user-configurable keybindings from config.toml (a natural request for a vim-user-oriented tool that already ships a neovim plugin), a which-key style overlay showing what each key does in the current mode, and a help pane that cannot drift because it is generated.
- Blocked by: —
- Notes: Structurally distinct from W6, which corrects the drifted text now; this is what stops it recurring. Every keymap item on this list (W1, W10, W15, W16, W17, W22) adds rows, so landing this early keeps their help entries free.
- [ ] execute   [ ] skip

### W26. Make the layout responsive with breakpoints and a minimum-size notice (TUI/layout — src/tui/ui.rs:24)
- Lens: terminal-robustness
- Score: 2.00 (value 4 / effort M)
- What: Make the top-level layout size-aware instead of the fixed `Vertical[Length(12), Min(9)]` with `Horizontal[Length(24), Fill(1)]` inside. Below roughly 100 columns drop the calendar and give the chart full width (or the reverse on a toggle key); below roughly 22 rows collapse the chart band so the project list keeps usable height; below a hard minimum render a centered "terminal too small" notice naming the required size rather than a mangled frame. Size the help popup with `Constraint::Length`-based clamping instead of a flat 60 percent square, and on very wide terminals cap the chart width and center the app so bars do not stretch into unreadable slabs.
- Why: On a stock 80x24 terminal the fixed band leaves the project list twelve rows, of which the header consumes two and the footer one — nine rows for a day that may hold several projects with note bullets, which is the actual content the user came to read. Narrower than 24 columns the calendar consumes everything and the chart is squeezed to nothing. Adaptive layout turns the TUI into something usable in a tmux split or a small side pane, which is exactly where a time tracker gets glanced at during the day.
- Blocked by: —
- [ ] execute   [ ] skip

### W27. Rescan populated dates only when the visible month changes (TUI — src/tui/app.rs:171)
- Lens: scale-perf
- Score: 2.00 (value 4 / effort M)
- What: Split `load_data_for_active_date` into a day/week load that runs on every date change and a month-population load that runs only when the calendar's displayed month actually changes, memoized as a `HashMap<(year, month), Vec<Date>>` on `App` and invalidated by `invalidate_date`, editor exit and an explicit `r` reload. Underneath, replace the roughly ninety per-date existence probes in `DataService::find_populated_dates` with a single `read_dir` of the time-tracking directory, parsing filenames into dates and opening only the files that actually exist.
- Why: Every arrow-key press spawns a `JoinSet` with one task per date across prev, current and next month, each walking `parse_day` into `read_day` into `get_file_path`, where `get_file_path` itself re-resolves the home directory and stats the data directory before the file's own `exists()` and `metadata()` calls. That is several hundred syscalls plus ninety parses to recompute a month-population map that is identical twenty-nine days out of thirty, and it stays O(90) per keypress as years of files accumulate. A directory listing is one syscall regardless of history size and is the foundation for a year heatmap or jump-to-previous-populated-day later.
- Blocked by: —
- Notes: `get_file_path` creating the data directory as a side effect (data_svc.rs:63) is what makes it expensive to call ninety-seven times per load; a `read_dir`-based scan sidesteps that path entirely. Pairs with W23, and it is the change that makes W1's month stepping cheap rather than a full rescan per press.
- [ ] execute   [ ] skip

### W28. Wrap or ellipsize long project names and note bullets (TUI/project-list — src/tui/project_list.rs:237)
- Lens: terminal-robustness
- Score: 2.00 (value 4 / effort M)
- What: List items are built as raw strings — a left-padded 25-column name plus hours, followed by one indented bullet line per note — with no wrapping and no truncation. Give the list width-aware rendering: wrap note bullets to the available width with a hanging indent, or truncate with a trailing ellipsis plus an expand affordance on the selected row, and pad the name column by display width using `unicode-width` instead of the `{:<25}` formatter, which counts chars. Consider a horizontal-scroll or show-full-note key for the selected item.
- Why: Notes are the payload of this tool — the user selects a project and presses `Enter` to copy its bullets into a timesheet or invoice, so seeing them in full matters. Ratatui silently clips any line wider than the list area, so on a narrow or split terminal a long task description just disappears at the right edge with no indication anything was cut. A project name longer than 25 characters also pushes the hours column out of alignment, and a name containing CJK text or an emoji misaligns it even when shorter.
- Blocked by: —
- Notes: Touches the same construction path as W13, which memoizes these lines per data load; wrapping is width-dependent, so cache the wrapped body keyed on width or sequence the two deliberately.
- [ ] execute   [ ] skip

### W29. Add Ctrl-Z suspend/resume via a shared terminal-suspension helper (TUI/lifecycle — src/tui/app.rs:140)
- Lens: terminal-robustness
- Score: 1.50 (value 3 / effort M)
- What: `run_editor` hand-rolls the suspend dance — pause events, `LeaveAlternateScreen`, `disable_raw_mode`, run the editor, `EnterAlternateScreen`, `enable_raw_mode`, `terminal.clear()`. Extract it into a `with_suspended_terminal(|| ...)` helper and reuse it for a new Ctrl-Z binding that raises `SIGTSTP` after restoring the cooked terminal and re-enters the TUI on `SIGCONT`. The key-handling match at app.rs:229 already special-cases Ctrl-C, so Ctrl-Z is a natural sibling.
- Why: Raw mode means the terminal never generates SIGTSTP, so Ctrl-Z inside the TUI is swallowed and there is currently no way to drop back to the shell and `fg` again — the user has to quit and relaunch, losing the selected date. Job control is reflexive for a developer running this from a shell all day. Factoring the suspension into one helper also gives a single place that later has to know about mouse capture, bracketed paste and any other terminal mode the TUI adopts, instead of duplicating the sequence per feature.
- Blocked by: —
- Notes: W30 needs exactly this helper so an editor session does not inherit mouse capture; do this one first.
- [ ] execute   [ ] skip

### W30. Add mouse support: click a calendar day, a bar, or a project row (TUI/input — src/tui/event.rs:81)
- Lens: terminal-robustness
- Score: 1.50 (value 3 / effort M)
- What: The crossterm `EventStream` is never put into mouse-capture mode and `App::run` matches only `Event::Key` (app.rs:87 discards everything else), so clicks and wheel scrolls do nothing. Enable mouse capture alongside the alternate-screen setup, disable it in the same suspension helper used for `$EDITOR`, add an `Event::Mouse` arm, and store the calendar, chart and list `Rect`s on `App` during render so a click can be hit-tested: click a calendar day or a weekly bar to jump to that date, click a project row to select it, wheel-scroll the list.
- Why: The user is often not in a keyboard-driven flow when they open this — they glance at the month and want a specific day. Every modern terminal emulator forwards mouse events, and clicking the calendar day you are already looking at is the most obvious interaction the calendar suggests but does not offer. Wheel-scrolling the project list is likewise reflexive on a day with many entries.
- Blocked by: —
- Notes: Mouse capture must be torn down and re-established in the same place the alternate screen is, or the editor session inherits capture — so do this after or together with W29's suspension helper.
- [ ] execute   [ ] skip

## Medium

_(none)_

## Low

_(none)_

## Skip (do not re-flag in future runs)
