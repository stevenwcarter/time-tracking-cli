# TIDY.md — code cleanup findings

Last triage: 2026-08-29 against `tidy/2026-08-29` @ 816020d. Toolchain: cargo build / cargo clippy --all-targets --all-features -- -D warnings / cargo test --workspace.

> **For future sessions reading this file:** when you fix an item listed
> here, strip it from this file in the same commit that fixes it. The list
> is intended to reflect open issues only; resolved items shouldn't linger.
> This keeps the file's signal-to-noise high for the next tidy pass.

## How to use this file
- Check `[x] execute` on items to run this batch.
- Check `[x] skip` on items to file them into this file's own Skip archive and never re-flag them.
- Items left unchecked stay in TIDY.md for the next run.
- When ready, run `/tidy --execute`.

## High severity

### T2. Format the date picker's value from local components instead of UTC: `formatDate` (site/src/components/DateSelector.tsx:23)
- Lenses: opportunistic
- Risk: high — needs characterization tests first
- Proposed fix: Blocked by T42 — land the shared helper first and call it here rather than hand-rolling a second formatter, because T42's helper must itself format from local components or this bug gets enshrined in it. `formatDate` builds its string with `d.toISOString().split('T')[0]`, so the "Today" button and the current-date display show the UTC calendar day rather than the user's; format from `getFullYear()` / `getMonth() + 1` / `getDate()`, zero-padded, so "today" matches the user's local calendar day at any hour.
- [x] execute   [ ] skip

### T3. Split WeeklySummary into two hooks and two row subcomponents: `WeeklySummary` (site/src/components/WeeklySummary.tsx:37-255, 219 lines)
- Lenses: long-methods
- Risk: high — needs characterization tests first
- Proposed fix: Extract a `useWeeklyTableData(weekData)` hook for the `tableData` useMemo (lines 41-93), a `useNotesLookup(weekData)` hook for `daysByDate` / `notesByDate` / `getNotesForProjectDate` / `formatNotesTooltip` / `copyNotesToClipboard` (lines 96-145), and subcomponents `ProjectRow` (lines 202-234) and `DailyTotalsRow` (lines 236-249) taking the derived data as props, leaving WeeklySummary to compose the hooks plus the `<table>` shell; coordinate with T20, which lifts the clipboard/toast body out of lines 119-145 into a shared `site/src/utils/clipboard.ts` — run T20 first and have `useNotesLookup` call that helper — and note the doc-fixer queue also rewrites the comment at line 85, so those lines may already have moved.
- [x] execute   [ ] skip

### T4. Default the editor route's date from local components, not UTC: `DateEditorPage` fallback date (site/src/page/DateEditorPage.tsx:10)
- Lenses: opportunistic
- Risk: high — needs characterization tests first
- Proposed fix: Blocked by T42 — use the shared local-components helper T42 introduces instead of `new Date().toISOString().split('T')[0]`, so landing on /editor with no `:date` param during the evening at a negative UTC offset no longer opens tomorrow's file; this matches the local-time handling already applied in WeeklySummary.tsx's day-of-week calculation, and line 20 of the same file repeats the identical expression and must change with it.
- [x] execute   [ ] skip

### T5. Default the weekly route's week from a local date, not UTC: `WeeklySummaryPage` fallback date (site/src/page/WeeklySummaryPage.tsx:8)
- Lenses: opportunistic
- Risk: high — needs characterization tests first
- Proposed fix: Blocked by T42 — compute the default day with the shared local-date helper T42 introduces and pass a local `YYYY-MM-DD` string into `useWeekData`, instead of handing it a raw `new Date()` that `useWeekData` / `useDateData` then format with `toISOString().split('T')[0]`, which shows next week's data to evening users behind UTC.
- [x] execute   [ ] skip

### T6. Split Config::load into arg synthesis, file load-or-create, overrides and date resolution: `Config::load` (src/config.rs:248-358, 111 lines)
- Lenses: long-methods
- Risk: high — needs characterization tests first
- Proposed fix: Extract `fn synthetic_args() -> Args` for the else-branch default Args (lines 251-269), `fn load_or_create_config_file(config_path: &Path) -> Result<Config>` for the read-or-write-default block (lines 271-290), `fn apply_arg_overrides(config: &mut Config, args: &Args)` for the run of `if let Some(...) = args.X` assignments (lines 292-355 minus the date block), and `fn resolve_requested_date(date_str: Option<String>) -> Date` for the interim-based date parsing (lines 316-341), so `Config::load` reads as four calls; interacts with T43, whose TOCTOU fix lands squarely inside lines 271-290 — run this split first and apply T43 to the extracted `load_or_create_config_file` — and with T22, which deletes `get_no_args` / `try_get_no_args` immediately above at lines 235-241.
- [x] execute   [ ] skip

### T7. Share the 90-minute dead-time error threshold instead of hardcoding it twice: dead-time error threshold (src/display/mod.rs:139)
- Lenses: idioms
- Risk: low
- Proposed fix: Add `pub(crate) const DEAD_TIME_ERROR_THRESHOLD_MINUTES: u32 = 90;` to src/display/mod.rs and use it at line 139 (`if data.dead_time_minutes < DEAD_TIME_ERROR_THRESHOLD_MINUTES`), then have src/tui/project_list.rs:30 reference `crate::display::DEAD_TIME_ERROR_THRESHOLD_MINUTES` instead of redefining the same literal — that constant's own doc comment at project_list.rs:26-29 already warns the two must never drift apart; line 139 sits inside the dead-time block T28 extracts as `push_dead_time` (lines 134-156), so if T28 runs first apply this inside the extracted function.
- [ ] execute   [ ] skip

### T8. Fix the plain formatter's 40-line dash rule and share banner rendering with the default formatter: `PlainDisplayFormatter::weekly_totals` (src/display/plain.rs:60)
- Lenses: duplication
- Risk: low
- Proposed fix: src/display/plain.rs:60 uses `"-\n".repeat(40)` where src/display/default.rs:60 uses `"-".repeat(40)` plus a single newline, so plain output emits 40 lines each containing one dash instead of one 40-column rule; cli/tests/golden/weekly_plain.txt pins this broken output and must be regenerated in the same commit. Then remove the drift's cause: default.rs:45-52 (weekly_header) is byte-identical to plain.rs:45-52, default.rs:124-130 and plain.rs:123-130 (daily_breakdowns_header) produce identical output via different code, and default.rs:135-140 vs plain.rs:135-141 (day_header) differ only by the emoji prefix — extend the `DaySummaryStyle` / `format_day_summary_impl` mechanism already used for `day_summary` (src/display/mod.rs:78-199) with a `title_prefix`/emoji field plus a shared `render_rule(width)` helper covering these banner/totals methods for both `DefaultDisplayFormatter` and `PlainDisplayFormatter`; markdown.rs differs meaningfully (Markdown headings) and should stay separate. Overlaps T29, which rewrites the `push_str(&format!(...))` sites in both files — do this one first, then re-derive T29's site list.
- [x] execute   [ ] skip

## Medium severity

### T11. Split main_impl and de-duplicate its report dispatch: `main_impl` (cli/src/main.rs:14-116, 103 lines)
- Lenses: long-methods
- Risk: medium
- Proposed fix: Extract `async fn spawn_webserver_if_configured(config: &Config, set: &mut JoinSet<()>, rx: ...) -> bool` for the `#[cfg(feature = "webapp")]` block (lines 43-62), `async fn show_report(config: &Config, week_start_weekday: Weekday) -> Result<()>` to replace both verbatim copies of the weekly/single-day dispatch (lines 82-88 and the `#[cfg(not(feature = "tui"))]` copy at 94-100), and `async fn wait_for_background_tasks(set: JoinSet<()>, webserver_running: bool) -> Result<()>` for lines 103-113; the doc-fixer queue independently deletes redundant comments at lines 83, 86, 95 and 98 and relocates the misplaced "Load configuration…" comment at lines 15-16, so those exact lines may already have shifted when this runs, and T40 fixes a typo at line 19 inside the same function.
- [x] execute   [ ] skip

### T12. Drop the unused @react-hook/debounce dependency: `@react-hook/debounce` (site/package.json:19)
- Lenses: dead-code
- Risk: high
- Proposed fix: Remove `"@react-hook/debounce": "^4.0.0"` (site/package.json:19) and run `yarn install` to update yarn.lock; confirmed via `grep -rn '@react-hook/debounce' site/src` (zero hits) — site/src/hooks/useDebounce.ts implements its own `useState`/`useEffect`/`setTimeout` debounce instead of importing the package.
- [x] execute   [ ] skip

### T13. Drop the unused @uidotdev/usehooks dependency: `@uidotdev/usehooks` (site/package.json:22)
- Lenses: dead-code
- Risk: high
- Proposed fix: Remove `"@uidotdev/usehooks": "^2.4.1"` (site/package.json:22) and run `yarn install`; confirmed via `grep -rln uidotdev site/src` (zero hits) and it is not a peer dependency of any other declared package.
- [x] execute   [ ] skip

### T14. Drop the unused uuid dependency: `uuid` (site/package.json:32)
- Lenses: dead-code
- Risk: high
- Proposed fix: Remove `"uuid": "^11.1.0"` (site/package.json:32) and run `yarn install`; confirmed via `grep -rn uuid site/src` (zero hits outside package.json) and it is not a peer or transitive requirement of any other declared dependency checked.
- [x] execute   [ ] skip

### T15. Drop the unused webfontloader dependency: `webfontloader` (site/package.json:34)
- Lenses: dead-code
- Risk: high
- Proposed fix: Remove `"webfontloader": "^1.6.28"` (site/package.json:34) and run `yarn install`; confirmed via `grep -rin webfont site/src site/index.html site/vite.config.ts` (zero hits).
- [x] execute   [ ] skip

### T16. Delete the never-imported BorderedTableCell component: `BorderedTableCell` (site/src/components/BorderedTableCell.tsx:1)
- Lenses: dead-code
- Risk: low
- Proposed fix: Delete site/src/components/BorderedTableCell.tsx outright; confirmed via `git grep -n BorderedTableCell` scoped to site/ — only the file's own definition matches, with no imports in App.tsx, WeeklySummary.tsx, or any other page or component.
- [x] execute   [ ] skip

### T17. Delete the now-unused getVariant export: `getVariant` (site/src/components/Button/ButtonTypes.ts:10-21)
- Lenses: dead-code
- Risk: low
- Proposed fix: Delete `getVariant` (ButtonTypes.ts:10-21); confirmed via `grep -rn getVariant site/src` — the only non-definition hit is the commented-out call at site/src/components/Button/index.tsx:40 that T41 removes. Pair this with T41: either both land (delete the commented call and the helper) or neither, since the alternative resolution is to restore the call and actually wire the variant up.
- [x] execute   [ ] skip

### T18. Remove the unread `content` dependency from the debounced-save effect: DateEditor save effect (site/src/components/DateEditor.tsx:55)
- Lenses: idioms
- Risk: low
- Proposed fix: The debounced-save `useEffect` lists `content` in its dependency array but never reads `content` in the effect body, so it re-runs on every server refetch for no reason; change the array to `}, [debouncedData, updater, date, hasInitialized]);`.
- [x] execute   [ ] skip

### T19. Drop the inline style that duplicates the textarea's Tailwind classes: DateEditor textarea (site/src/components/DateEditor.tsx:77)
- Lenses: idioms
- Risk: low
- Proposed fix: The textarea carries `style={{ width: '50%', height: '100%' }}` even though `className` already sets `w-1/2` (width 50%), and `h-full` is the codebase's established way to say height 100% (see PageTemplate.tsx); drop the `style` prop entirely and use `className="w-1/2 p-2 border rounded mr-4 bg-gray-900 text-white h-full"`.
- [x] execute   [ ] skip

### T20. Extract a shared clipboard-copy-with-toast helper: `copyProjectNotesToClipboard` / `copyNotesToClipboard` (site/src/components/DateSummary.tsx:22, +1 site)
- Lenses: duplication
- Risk: medium
- Proposed fix: site/src/components/DateSummary.tsx:22-43 (`copyProjectNotesToClipboard`) and site/src/components/WeeklySummary.tsx:119-145 (`formatNotesTooltip` at 119 plus `copyNotesToClipboard` at 125) both join notes as `- ${note}` lines, `await navigator.clipboard.writeText(...)`, then fire `toast.success(msg, { position: 'top-right', autoClose: 2000, ... })` on success or `toast.error('Failed to copy to clipboard', { ... })` on failure; extract `copyNotesToClipboard(notes: string[], successMessage: string): Promise<void>` into site/src/utils/clipboard.ts with each caller supplying its own notes array and success message, leaving DateSummary's empty-notes early return and WeeklySummary's 'No notes for this day' tooltip fallback as caller-side concerns. Run this before T3, which restructures the WeeklySummary.tsx site into a `useNotesLookup` hook.
- [x] execute   [ ] skip

### T21. Stop panicking in Config::default() when the home directory can't be resolved: `Config::default` (src/config.rs:188)
- Lenses: idioms, opportunistic
- Risk: medium
- Proposed fix: `Config::default()` eagerly resolves the home directory with `Some(get_time_tracking_dir_with_override(None).unwrap().display().to_string())`, which panics wherever `dirs::home_dir()` returns None (a container with no `$HOME`, for example); set `data_directory: None` instead, since `get_data_directory()` / `get_time_tracking_dir_with_override(None)` already re-resolve lazily on demand and surface a `Result` error rather than panicking — that removes the panic and the eager work in one change, so no separate fallible-default plumbing is needed.
- [x] execute   [ ] skip

### T23. Make DataService::clear_cache test-only or remove it: `DataService::clear_cache` (src/data_svc.rs:232)
- Lenses: dead-code
- Risk: medium
- Proposed fix: `clear_cache` is `pub` in non-test code but its only caller is a `#[cfg(test)]` test (data_svc.rs:1139, inside `mod tests` at line 723); either delete it and rebuild that test to clear the cache via `invalidate_date` per key, or move it under `#[cfg(test)]` alongside the analogous test-only helpers already in this file (e.g. `parse_count`). Confirmed via `git grep -n 'clear_cache'` — only the definition and that one in-test call exist repo-wide. Public-API removal — needs an explicit decision before execution.
- [ ] execute   [ ] skip

### T24. Replace the exists()-then-write template race with an atomic create-only write: `create_day_file_if_not_exists` (src/data_svc.rs:361)
- Lenses: opportunistic
- Risk: medium
- Proposed fix: There is a TOCTOU window between `file_path.exists()` and the subsequent `fs::write` in which a concurrent process can create and populate the day file, after which this write clobbers real content with the empty template; replace the pair with an atomic create-only open — `tokio::fs::OpenOptions::new().write(true).create_new(true).open(&file_path)` — treating `ErrorKind::AlreadyExists` as "someone else already created it" and skipping the write instead of racing. Same shape as T43 in the config path; fix both the same way.
- [x] execute   [ ] skip

### T25. Split get_weekly_summary into load, fold and finalize phases: `DataService::get_weekly_summary` (src/data_svc.rs:505-591, 87 lines)
- Lenses: long-methods
- Risk: low
- Proposed fix: Extract `async fn load_days(&self, dates: &[Date]) -> Result<Vec<DayLoad>>` for the JoinSet spawn, collect and reorder (lines 506-528), `fn fold_day(summary: &mut WeeklySummary, week_projects: &mut HashMap<String, (u32, Vec<String>)>, day_date: Date, content: Option<String>, parsed: Option<TimeTrackingData>)` for the per-day accumulation (lines 531-563), and `fn finalize_projects(week_projects: HashMap<String, (u32, Vec<String>)>) -> Vec<WeeklyProject>` for the sort and collect (lines 566-577), so `get_weekly_summary` reads as three calls; T26 deletes the thin `get_weekly_data` wrapper just below at lines 597-599, and T39 extracts the same load/fold shape from `web.rs::aggregate_week_days`, so consider whether the two can share the fold once both are extracted.
- [x] execute   [ ] skip

### T26. Delete or gate the test-only DataService::get_weekly_data wrapper: `DataService::get_weekly_data` (src/data_svc.rs:597-599)
- Lenses: dead-code
- Risk: medium
- Proposed fix: Delete `get_weekly_data` (data_svc.rs:597-599) and update its one test caller (data_svc.rs:1284, inside `mod tests`) to call `get_weekly_summary(&week).await?.per_day` directly, or move the function under `#[cfg(test)]`; confirmed via `git grep -n 'get_weekly_data('` — only the definition and that test call exist, with all other hits being doc comments or prose in tui/event.rs, tui/app.rs and docs/. Those prose mentions need updating with the deletion. Public-API removal — needs an explicit decision before execution.
- [ ] execute   [ ] skip

### T27. Stop cloning the whole CacheEntry to check its Copy metadata: `get_valid_entry` (src/data_svc.rs:610)
- Lenses: opportunistic
- Risk: medium
- Proposed fix: `get_valid_entry` clones the entire `CacheEntry` — raw file text plus parsed data — merely to inspect `cached_at` and `file_mod_time`, on the documented ~97-calls-per-navigation hot path; copy only `cached_at` and `file_mod_time` (both `Copy`) while holding the lock to decide validity, then re-lock briefly to take just the field the caller needs (`data` or `parsed`), so an invalid or metadata-only check never clones the payload.
- [x] execute   [ ] skip

### T28. Split format_day_summary_impl into its five already-commented sections: `format_day_summary_impl` (src/display/mod.rs:96-199, 104 lines)
- Lenses: long-methods
- Risk: medium
- Proposed fix: Extract `fn push_overview(msg: &mut String, indent: &str, style: &DaySummaryStyle, data: &TimeTrackingData)` (lines 106-120), `push_working_time` (122-132), `push_dead_time` (134-156), `push_warnings` (158-167) and `push_projects` (169-196), reducing `format_day_summary_impl` to five calls and making each section independently testable; T7 changes line 139 inside the future `push_dead_time` and T29 rewrites the `push_str(&format!(...))` calls throughout this range, so sequence this split first and apply those two inside the extracted functions.
- [ ] execute   [ ] skip

### T29. Replace push_str(&format!(...)) with write! across the display formatters: `format_day_summary_impl` and the three formatter impls (src/display/mod.rs:107, +53 sites)
- Lenses: idioms
- Risk: low
- Proposed fix: 54 sites build output with `msg.push_str(&format!(...))`, allocating a temporary String only to copy it into another (clippy::format_push_string); add `use std::fmt::Write as _;` per file and replace each with `let _ = write!(msg, ...);`, clippy's own suggested rewrite. Sites: src/display/mod.rs:107, 108, 113, 123, 124, 135, 137, 144, 160, 162, 171, 173, 182, 186, 187, 191, 192; src/display/plain.rs:47, 49, 50, 62, 69, 88, 103, 112, 125, 137; src/display/markdown.rs:19, 26, 35, 37, 43, 45, 52, 71, 81, 86, 103, 117, 118, 126; src/display/default.rs:47, 49, 50, 60, 62, 69, 89, 104, 113, 126, 128, 137, 138. T8 and T28 both restructure code inside these ranges, so run them first and re-derive the line numbers rather than trusting this list verbatim.
- [ ] execute   [ ] skip

### T30. Delete the unused display::get_file_path / read_day / parse_day wrappers: `display::get_file_path` (src/display/mod.rs:274-284)
- Lenses: dead-code
- Risk: medium
- Proposed fix: Delete the three free functions at src/display/mod.rs:274-284 (`get_file_path`, `read_day`, `parse_day`), which duplicate `DataService` methods of the same name; confirmed via `git grep -n 'display::get_file_path|display::read_day'` (no hits) and by checking that every other call site of `read_day` / `get_file_path` / `parse_day` in the repo goes through `DataService::get()...` or a `DataService` instance directly, never these wrappers, and that they are not re-exported from src/lib.rs. These lines sit outside the 96-199 range T28 restructures, so the two don't collide. Public-API removal — needs an explicit decision before execution.
- [ ] execute   [ ] skip

### T31. Unify the four different ways date.format(&DATE_FORMAT) is handled: `create_day_file` date formatting (src/file_utils.rs:33, +2 sites)
- Lenses: idioms
- Risk: low
- Proposed fix: The same fallible `date.format(&DATE_FORMAT)` call is handled three different ways with no stated reason — `unwrap()`, `unwrap_or_default()` (which silently yields an empty date string), and `context()?`; at src/file_utils.rs:33 the function returns `Result<String>`, so use `let formatted_date = date.format(&DATE_FORMAT).context("could not format date")?;` and drop the redundant `.to_string()`; at src/web.rs:62 `DayData::empty` currently uses `.unwrap_or_default()` and would ship a blank date to the client on failure instead of surfacing it the way `get_day_data_impl` (web.rs:199) does, so use `.expect("DATE_FORMAT is a fixed valid format")` there; and src/time_utils.rs:51 returns `String` and cannot propagate, so keep it but switch the bare `.unwrap()` to the same documented `.expect(...)`. The doc-fixer queue adds a doc comment at time_utils.rs:50, immediately above that last site, so re-locate line 51 if the docs land first.
- [ ] execute   [ ] skip

### T32. Extract the duplicated wrap-around list navigation shared by the two TUI panes: `next_item` / `previous_item` / `go_to_first` / `go_to_last` (src/tui/project_list.rs:280, +1 file)
- Lenses: duplication
- Risk: low
- Proposed fix: Duplicate sites: src/tui/project_list.rs:280-289 (next_item), 291-301 (previous_item), 303-307 (go_to_first), 309-315 (go_to_last) versus src/tui/week_list.rs:251-260, 262-272, 274-278 and 280-284; both operate on a `ratatui::widgets::ListState` plus a length, with project_list.rs reading the length from `self.project_list.items` and week_list.rs taking it as a `len: usize` parameter. Extract free functions `select_next(state: &mut ListState, len: usize)`, `select_previous(...)`, `select_first(...)` and `select_last(...)` into src/tui/band.rs (already the shared home for cross-pane list-band logic) and have both widgets call them, project_list.rs passing `self.project_list.items.len()`.
- [ ] execute   [ ] skip

### T33. Delete the never-called ProjectListWidget::has_items: `ProjectListWidget::has_items` (src/tui/project_list.rs:321-323)
- Lenses: dead-code
- Risk: medium
- Proposed fix: Delete `has_items` (project_list.rs:321-323); confirmed via `git grep -n has_items` across the whole repo, where only its own definition matches — `selected_item` directly above it on the same struct is used from app.rs, mode.rs and week_list.rs, but `has_items` is not. Public-API removal — needs an explicit decision before execution.
- [ ] execute   [ ] skip

### T34. Share the popup centering math and SCREEN_MARGIN between the two overlay widgets: `popup_area` (src/tui/widgets/date_prompt.rs:79, +1 file)
- Lenses: duplication
- Risk: low
- Proposed fix: src/tui/widgets/help_popup.rs:71 and src/tui/widgets/date_prompt.rs:35 declare an identical `const SCREEN_MARGIN: u16 = 4` with an identical doc comment, and help_popup.rs:81-97 (popup_area) and date_prompt.rs:79-87 (popup_area) end with the same four lines (`Layout::vertical([Constraint::Length(height)]).flex(Flex::Center)`, the horizontal counterpart, and two `let [area] = ...areas(area)` destructurings), with each doc comment already cross-referencing the other; move `SCREEN_MARGIN` and a shared `pub(super) fn centered_box(area: Rect, width: u16, height: u16) -> Rect` carrying the common centering tail into src/tui/widgets/popup.rs, leaving each widget its own width/height sizing. The doc-fixer queue also adds a struct doc at popup.rs:11, so expect that file to have shifted slightly.
- [ ] execute   [ ] skip

### T35. Extract the total-hours label placement from the bar chart's render: `WeeklyBarChart::render` (src/tui/widgets/weekly_bar_chart.rs:246-324, 79 lines)
- Lenses: long-methods
- Risk: low
- Proposed fix: Most steps of this 79-line `Widget::render` already delegate to helpers, but the total-hours label's position math is inlined; extract `fn render_total_hours_label(&self, area: Rect, inner_area: Rect, total_text: String, buf: &mut Buffer)` covering the RIGHT_MARGIN / total_area / Paragraph block at lines 289-307, after which render reads as block, then label, then chart, then goal line. The doc-fixer queue deletes or rewrites comments at lines 256, 259, 263, 274 and 309 inside this same function, and T48 drops the unused `&self` from `calculate_bar_dimensions` called at line 253, so expect the line numbers to move.
- [ ] execute   [ ] skip

### T36. Extract shared request-date resolution for the four HTTP handlers: `get_day_data` / `get_week_data` date resolution (src/web.rs:174, +3 sites)
- Lenses: duplication
- Risk: high — needs characterization tests first
- Proposed fix: src/web.rs:174-181 (get_day_data) is byte-identical to src/web.rs:257-264 (get_week_data) — the `match params.date { Some(date_str) => Date::parse(...).map_err(|_| StatusCode::BAD_REQUEST)?, None => OffsetDateTime::now_local()...date() }` block — and src/web.rs:191 (get_day_data_by_date) and src/web.rs:277 (get_week_data_by_date) both repeat `Date::parse(&date_str, DATE_FORMAT).map_err(|_| StatusCode::BAD_REQUEST)?`; extract `fn resolve_date_or_today(date_str: Option<String>) -> Result<Date, StatusCode>` and `fn parse_date_param(date_str: &str) -> Result<Date, StatusCode>` and call them from all four handlers. The GraphQL layer has the same shape under a different error type (T45), so consider whether one core parser can back both.
- [ ] execute   [ ] skip

### T37. Extract the parsed-to-DTO mapping from get_day_data_impl: `get_day_data_impl` (src/web.rs:197-251, 55 lines)
- Lenses: long-methods
- Risk: high — needs characterization tests first
- Proposed fix: Extract `fn day_data_from_parsed(date_str: String, data: TimeTrackingData) -> DayData` covering the field mapping at lines 222-250, reducing `get_day_data_impl` to fetch, early return, and one call; this function is also the target of T38 (line 220 reparses instead of using the memoized parse) and T49 (line 208 rebuilds the `DayData::empty` literal by hand), both of which land inside the range this split rearranges — run this split first, or apply all three together in one edit of the function.
- [ ] execute   [ ] skip

### T38. Route the web/GraphQL day read through the memoized parse: `get_day_data_impl` parse call (src/web.rs:220)
- Lenses: opportunistic
- Risk: medium
- Proposed fix: `get_day_data_impl` re-invokes `time_tracking_parser::parse_time_tracking_data` on the raw content instead of calling `DataService::get().parse_day(&date)`, so every /api/day, /api/week and GraphQL request bypasses the 30-second memoized parse the TUI and CLI already share (`state.config` and the global `Config` agree on prefix/suffix, so the memoized parse is equivalent); switch to the DataService call. Sits inside the function T37 splits and two lines from where T49 applies, so coordinate the three.
- [ ] execute   [ ] skip

### T39. Extract the week fold out of aggregate_week_days: `aggregate_week_days` (src/web.rs:282-325, 44 lines)
- Lenses: long-methods
- Risk: high — needs characterization tests first
- Proposed fix: Extract `fn fold_week_results(week_dates: &[Date], results: Vec<(usize, DayData)>) -> (Vec<DayData>, Vec<ProjectSummary>, f64, f64)` covering lines 298-322 (order restore, day and project accumulation, project sort), leaving `aggregate_week_days` with only the spawn/collect and the call; this duplicates the load/fold shape already in `DataService::get_weekly_summary` that T25 extracts, so if both run, check whether the two folds can converge on one implementation rather than two parallel ones.
- [ ] execute   [ ] skip

## Low severity

### T40. Fix the "Coult not initialize tracing" typo: tracing init error context (cli/src/main.rs:19)
- Lenses: idioms
- Risk: low
- Proposed fix: Change the error context string at cli/src/main.rs:19 from "Coult not initialize tracing" to "Could not initialize tracing"; this line sits inside `main_impl`, which T11 splits and the doc-fixer queue also edits at lines 15-16, so re-locate it if either lands first.
- [ ] execute   [ ] skip

### T41. Delete the eleven-month-old commented-out clsx entries: `Button` clsx call (site/src/components/Button/index.tsx:24, +2 lines)
- Lenses: dead-code
- Risk: low
- Proposed fix: Delete the three commented lines at site/src/components/Button/index.tsx:24, 40 and 41 (`// 'text-black',`, `// getVariant(type, disabled),`, `// block && 'w-full',`); `git blame -L 20,45 -- site/src/components/Button/index.tsx` dates all three to commit b3857caa on 2025-10-04, roughly eleven months old and well past the 30-day bar. Once line 40 is gone, `type` becomes an unused destructured field of `props` at line 20 — drop it from the destructure too. Pair with T17, which deletes the `getVariant` helper that call was the last reference to.
- [x] execute   [ ] skip

### T42. Add a shared local-date string helper for the five toISOString call sites: `toDateString` (site/src/hooks/useDateData.ts:11, +4 sites)
- Lenses: duplication
- Risk: low
- Proposed fix: `date.toISOString().split('T')[0]` is repeated at site/src/hooks/useDateData.ts:11, site/src/hooks/useWeekData.ts:6, site/src/components/DateSelector.tsx:23 (`formatDate`), and site/src/page/DateEditorPage.tsx:10 and :20; add a single `toDateString(date: Date): string` helper in a new site/src/utils/date.ts (no utils module exists yet) and use it at all five call sites. Critically, the helper MUST format from LOCAL components — `` `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}` `` — not `toISOString()`, because T2, T4 and T5 are the same UTC-versus-local bug seen at three of these call sites and a helper that merely centralises `toISOString()` would enshrine it; land this first, then T2, T4 and T5 become calls to it.
- [x] execute   [ ] skip

### T43. Replace the exists()-then-write config race with an atomic create-if-absent open: default config write (src/config.rs:273)
- Lenses: opportunistic
- Risk: medium
- Proposed fix: Blocked by T6 — the TOCTOU window between `config_path.exists()` and the subsequent write, in which a concurrent process's freshly written config is silently clobbered, sits in lines 271-290, exactly the block T6 extracts as `load_or_create_config_file`, so run T6 first and apply this inside the extracted function (or skip T6 and patch in place). Use an atomic create-if-absent open (`create_new`) for the default-config write and treat `AlreadyExists` as "read what's there" instead of a separate `exists()` check followed by `fs::write`. Same shape as T24 in the day-file path; fix both the same way.
- [ ] execute   [ ] skip

### T44. Build the empty string the short way: empty-template return (src/file_utils.rs:38)
- Lenses: idioms
- Risk: low
- Proposed fix: Replace `Ok("".to_string())` at src/file_utils.rs:38 with `Ok(String::new())` (clippy::manual_string_new).
- [ ] execute   [ ] skip

### T45. Extract a shared GraphQL date parser and drop the redundant conversion closure: `data_for_date` date handling (src/graphql.rs:26, +3 sites)
- Lenses: duplication, idioms
- Risk: high — needs characterization tests first
- Proposed fix: `Date::parse(&date, DATE_FORMAT).map_err(|_| INVALID_DATE_MSG)?` is repeated verbatim in all four Query and Mutation resolvers — src/graphql.rs:26 (data_for_date), 38 (file_content_for_date), 61 (week_data_for_date) and 114 (update_file_content) — so extract `fn parse_date_field(date: &str) -> Result<Date, &'static str>` next to `INVALID_DATE_MSG` (src/graphql.rs:12) and call it from all four; while in `data_for_date`, also replace the closure two lines below at line 28, `.map_err(|e| e.into())`, with `.map_err(Into::into)` (clippy::redundant_closure). The HTTP handlers carry the same parse under `StatusCode` (T36), so consider whether one core parser can back both.
- [ ] execute   [ ] skip

### T46. Mark the three branches of the wrap_note state machine with section comments: `wrap_note` (src/tui/project_list.rs:584-648, 65 lines)
- Lenses: long-methods
- Risk: low
- Proposed fix: `wrap_note` is a 65-line greedy word-wrap state machine with three to four levels of nesting, but extracting it would fragment the shared mutable state (`line`, `has_word`, `lines`); instead add section comments `// word fits on the current line`, `// start a new line`, and `// doesn't fit even alone: hard-break it` (the last already exists) around the three branches of the for-loop body, making the phases explicit without splitting the state.
- [ ] execute   [ ] skip

### T47. Collapse the doubled Preset fallback into one map_or: theme preset resolution (src/tui/theme.rs:174)
- Lenses: idioms
- Risk: low
- Proposed fix: `configured.map(|name| name.parse().unwrap_or(Preset::Dark)).unwrap_or(Preset::Dark)` states the same fallback twice; use `let preset = configured.map_or(Preset::Dark, |name| name.parse().unwrap_or(Preset::Dark));`.
- [ ] execute   [ ] skip

### T48. Drop the unused &self from calculate_bar_dimensions: `WeeklyBarChart::calculate_bar_dimensions` (src/tui/widgets/weekly_bar_chart.rs:163)
- Lenses: idioms
- Risk: low
- Proposed fix: The private `calculate_bar_dimensions` takes `&self` but never reads it (clippy::unused_self); drop the parameter and call it as `Self::calculate_bar_dimensions(area)` from its single call site at line 253, or make it a free function. T35 restructures the surrounding `render`, so re-locate the call site if that lands first.
- [ ] execute   [ ] skip

### T49. Reuse DayData::empty for the no-file early return: `get_day_data_impl` empty literal (src/web.rs:208-218)
- Lenses: duplication
- Risk: medium
- Proposed fix: src/web.rs:60-70 (`DayData::empty(date)`) duplicates the struct literal at src/web.rs:208-218 — the early return inside `get_day_data_impl` when `content` is None — field for field, differing only in how `date` / `date_str` is supplied; replace the inline literal with `DayData { date: date_str, ..DayData::empty(date) }`, or add a `DayData::empty_with_date_str(date_str: String)` constructor, so there is one source of the "no data" shape. Note T31 also changes `DayData::empty`'s date formatting at web.rs:62, and T37 and T38 restructure the same function this literal lives in.
- [ ] execute   [ ] skip

## Skip (do not re-flag in future runs)

### T22. Delete the never-called Config::get_no_args / try_get_no_args: `Config::get_no_args` (src/config.rs:235)
- Lenses: dead-code
- Risk: medium
- Proposed fix: Delete `get_no_args` (config.rs:235) and `try_get_no_args` (config.rs:239); confirmed via `git grep -n 'get_no_args'` and `git grep -n 'try_get_no_args'` across the whole repo, where only their own definitions matched. Deleting `try_get_no_args` also strands the private `try_init` (config.rs:211) — delete it too, or fold its body into `try_get_no_args` before removing. Note that two entries in the doc-fixer queue (the `Config` struct doc at config.rs:133 and the `Config::get` doc at config.rs:243) name `get_no_args()` in their intent text, so those docs must not reference it once this lands, and T6 splits `Config::load` starting a few lines below. Public-API removal — needs an explicit decision before execution.
- User note: this is called by a vim plugin (time-tracking-nvim)
- [ ] execute   [x] skip
