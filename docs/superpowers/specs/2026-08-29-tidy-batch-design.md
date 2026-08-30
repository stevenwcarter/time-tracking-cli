# Tidy execution batch — 2026-08-29

25 findings selected from `TIDY.md` (triage @ 816020d, branch `tidy/2026-08-29`).
This document is the execution scope. It changes no behavior beyond what each
finding states.

## Working environment

All work happens in the worktree `/home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy`
on branch `tidy/2026-08-29`. It already contains `site/build/` (copied) and a
`site/node_modules` symlink, both required to compile the `webapp` feature.

Commands:

```
export SKIP_YARN=1
cargo check --workspace --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
cargo fmt --all
cd site && ./node_modules/.bin/eslint src --report-unused-disable-directives --max-warnings 0
cd site && yarn test --run
```

`just gate` is the full verification gate (three feature combinations). Run it at
the final milestone, not per task — it is slow.

**Never run the binary bare.** A plain `ttcli` or `cargo run -p cli --` defaults to
the real `~/.time-tracking/` and opens `$EDITOR` on today's file. Always pass
`--noedit --data-directory <tmp>`.

## Per-task contract

1. Read the finding below (file:line, proposed fix, risk).
2. If `risk: high — needs characterization tests first`: write characterization
   tests for the affected unit, confirm they pass on the **unchanged** code,
   commit as `test: characterize <unit> before tidy [T<n>]`.
3. Apply the change.
4. Run lint / typecheck / format. Fix warnings the change introduced; leave
   preexisting unrelated warnings alone.
5. At each milestone boundary (marked below): run the full test suite.
6. Strip the finding: `todo-parser TIDY.md --strip T<n>`.
7. Commit the code change and the strip **together**:
   `git add -A && git commit -m 'tidy(<lens>): <summary> [T<n>]'`.
   This is what keeps each fix independently revertable. Non-negotiable.

Never use `--no-verify`. Husky + commitlint enforce Conventional Commits.
Every commit body ends with a blank line then:
`Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP`

## Invariants this batch depends on

- **The library has an out-of-repo consumer** (`time-tracking-nvim` links the crate
  as a Rust library). "No callers per `git grep`" therefore does not prove a `pub`
  item is unused. No task in this batch deletes a `pub` Rust item; T22 was skipped
  for exactly this reason. If a task's fix would remove one, stop and surface it.
- `cli/tests/golden/*.txt` pin CLI output byte-for-byte. T8 deliberately changes
  `weekly_plain.txt`; no other task may change a golden file. If one does, that is
  an unintended behavior change — stop and diagnose.
- The three Cargo feature combinations (default, `tui`-only, `webapp`-only) must
  stay isolated. T9/T10 remove dependencies from the `webapp` feature list; the
  `cargo tree -i` assertions in `just gate` are what catch a regression there.
- **Do not refactor existing tests.** New characterization tests are welcome; edits
  to existing test files are limited to what a deliberate behavior change requires
  (T8's golden file).

## Execution order

The order below is load-bearing — several findings would bake in a bug or conflict
if run out of sequence.

### Wave 0 — unblock the toolchain (1 item, do first)

**T1.** Fixes `yarn lint` taking 12+ minutes. Every later TS task runs lint; landing
this first turns a 12-minute step into a 1-second one. Verify after: `eslint .`
(the bare form `yarn lint` uses) must now finish in seconds.

### Wave 1 — independent deletions and one-liners (11 items)

T9, T10 (Cargo deps) · T12, T13, T14, T15 (npm deps) · T41 then T17 (order matters:
T41 deletes the commented-out `getVariant` call, which is what leaves T17's export
unused) · T16 · T18 · T19.

For T12–T15, run `yarn install` so `yarn.lock` updates, and confirm `yarn build`
still succeeds — these are `risk: high` precisely because a removal can break an
adjacent transitive requirement.

*Milestone after Wave 1: full Rust test suite + `yarn build` + `yarn test`.*

### Wave 2 — shared helpers that unblock Wave 3 (2 items)

**T42** — the shared date-string helper. **It must format from local components**
(`getFullYear()` / `getMonth() + 1` / `getDate()`, zero-padded), NOT `toISOString()`.
Writing it as a straight extraction of the existing `toISOString().split('T')[0]`
would enshrine the very bug T2/T4/T5 exist to fix. Migrate all five call sites.

**T20** — the shared clipboard-with-toast helper, extracted before T3 splits the
component that holds one of its two call sites.

### Wave 3 — the UTC-vs-local date bugs (3 items, all risk: high)

T2, T4, T5 — each blocked by T42; each calls the helper rather than hand-rolling a
second formatter. All three need characterization tests first. Test at a fixed
instant that differs between UTC and a negative-offset local zone (e.g. 23:30 local
at UTC−05:00, where UTC has already rolled to the next day) — that is the case
that currently produces the wrong date.

*Milestone after Wave 3: full Rust test suite + `yarn build` + `yarn test`.*

### Wave 4 — the plain-formatter output bug (1 item)

**T8** — fix `"-\n".repeat(40)` to one 40-column rule, then regenerate
`cli/tests/golden/weekly_plain.txt` in the **same commit** (it currently pins the
broken output). Land the bug fix before the banner-sharing refactor so the golden
diff shows exactly one intended change. Confirm `weekly_default.txt` and
`weekly_markdown.txt` are byte-unchanged.

### Wave 5 — Rust refactors (6 items)

T6 (risk: high — characterization tests first; also run before anything else in
`Config::load`) · T21 · T24 · T25 · T27 · T11.

T21 and T6 both touch `src/config.rs`; T6's split moves the code T21 changes, so run
T6 first. T24 and T27 are both in `src/data_svc.rs` but in different functions.

*Milestone after Wave 5: full Rust test suite.*

### Wave 6 — the React component split (1 item, risk: high)

**T3** — after T20, so `useNotesLookup` calls the shared clipboard helper rather
than carrying a copy. Characterization tests first.

*Final: `just gate` (all three feature combinations) + `yarn build` + `yarn test`.*

## Findings

Verbatim from `TIDY.md`, in ID order (not execution order — follow the waves above).

### T1. Point eslint at the real build output so `yarn lint` finishes in seconds: `ignores: ["dist"]` (site/eslint.config.js:10)
- Lenses: idioms
- Risk: low
- Proposed fix: Change `{ ignores: ["dist"] }` at site/eslint.config.js:10 to `{ ignores: ["dist", "build", "coverage"] }` — Vite's `outDir` is `build` (site/vite.config.ts:14), so today `eslint .` lints the entire 1.4MB production bundle; verified in this worktree that a bare `eslint .` pegged one core for 12+ minutes and had to be killed while `eslint . --ignore-pattern 'build/**'` completed in 1.07s with zero errors, and site/vite.config.ts already excludes `'**/build/**'` (line 24) and `['coverage', 'build']` (line 36) elsewhere, so the eslint config is the one place that still says `dist`; `site/build/` exists on any machine that has run `yarn build`, which `just gate` requires, so this hits every developer.

### T2. Format the date picker's value from local components instead of UTC: `formatDate` (site/src/components/DateSelector.tsx:23)
- Lenses: opportunistic
- Risk: high — needs characterization tests first
- Proposed fix: Blocked by T42 — land the shared helper first and call it here rather than hand-rolling a second formatter, because T42's helper must itself format from local components or this bug gets enshrined in it. `formatDate` builds its string with `d.toISOString().split('T')[0]`, so the "Today" button and the current-date display show the UTC calendar day rather than the user's; format from `getFullYear()` / `getMonth() + 1` / `getDate()`, zero-padded, so "today" matches the user's local calendar day at any hour.

### T3. Split WeeklySummary into two hooks and two row subcomponents: `WeeklySummary` (site/src/components/WeeklySummary.tsx:37-255, 219 lines)
- Lenses: long-methods
- Risk: high — needs characterization tests first
- Proposed fix: Extract a `useWeeklyTableData(weekData)` hook for the `tableData` useMemo (lines 41-93), a `useNotesLookup(weekData)` hook for `daysByDate` / `notesByDate` / `getNotesForProjectDate` / `formatNotesTooltip` / `copyNotesToClipboard` (lines 96-145), and subcomponents `ProjectRow` (lines 202-234) and `DailyTotalsRow` (lines 236-249) taking the derived data as props, leaving WeeklySummary to compose the hooks plus the `<table>` shell; coordinate with T20, which lifts the clipboard/toast body out of lines 119-145 into a shared `site/src/utils/clipboard.ts` — run T20 first and have `useNotesLookup` call that helper — and note the doc-fixer queue also rewrites the comment at line 85, so those lines may already have moved.

### T4. Default the editor route's date from local components, not UTC: `DateEditorPage` fallback date (site/src/page/DateEditorPage.tsx:10)
- Lenses: opportunistic
- Risk: high — needs characterization tests first
- Proposed fix: Blocked by T42 — use the shared local-components helper T42 introduces instead of `new Date().toISOString().split('T')[0]`, so landing on /editor with no `:date` param during the evening at a negative UTC offset no longer opens tomorrow's file; this matches the local-time handling already applied in WeeklySummary.tsx's day-of-week calculation, and line 20 of the same file repeats the identical expression and must change with it.

### T5. Default the weekly route's week from a local date, not UTC: `WeeklySummaryPage` fallback date (site/src/page/WeeklySummaryPage.tsx:8)
- Lenses: opportunistic
- Risk: high — needs characterization tests first
- Proposed fix: Blocked by T42 — compute the default day with the shared local-date helper T42 introduces and pass a local `YYYY-MM-DD` string into `useWeekData`, instead of handing it a raw `new Date()` that `useWeekData` / `useDateData` then format with `toISOString().split('T')[0]`, which shows next week's data to evening users behind UTC.

### T6. Split Config::load into arg synthesis, file load-or-create, overrides and date resolution: `Config::load` (src/config.rs:248-358, 111 lines)
- Lenses: long-methods
- Risk: high — needs characterization tests first
- Proposed fix: Extract `fn synthetic_args() -> Args` for the else-branch default Args (lines 251-269), `fn load_or_create_config_file(config_path: &Path) -> Result<Config>` for the read-or-write-default block (lines 271-290), `fn apply_arg_overrides(config: &mut Config, args: &Args)` for the run of `if let Some(...) = args.X` assignments (lines 292-355 minus the date block), and `fn resolve_requested_date(date_str: Option<String>) -> Date` for the interim-based date parsing (lines 316-341), so `Config::load` reads as four calls; interacts with T43, whose TOCTOU fix lands squarely inside lines 271-290 — run this split first and apply T43 to the extracted `load_or_create_config_file` — and with T22, which deletes `get_no_args` / `try_get_no_args` immediately above at lines 235-241.

### T8. Fix the plain formatter's 40-line dash rule and share banner rendering with the default formatter: `PlainDisplayFormatter::weekly_totals` (src/display/plain.rs:60)
- Lenses: duplication
- Risk: low
- Proposed fix: src/display/plain.rs:60 uses `"-\n".repeat(40)` where src/display/default.rs:60 uses `"-".repeat(40)` plus a single newline, so plain output emits 40 lines each containing one dash instead of one 40-column rule; cli/tests/golden/weekly_plain.txt pins this broken output and must be regenerated in the same commit. Then remove the drift's cause: default.rs:45-52 (weekly_header) is byte-identical to plain.rs:45-52, default.rs:124-130 and plain.rs:123-130 (daily_breakdowns_header) produce identical output via different code, and default.rs:135-140 vs plain.rs:135-141 (day_header) differ only by the emoji prefix — extend the `DaySummaryStyle` / `format_day_summary_impl` mechanism already used for `day_summary` (src/display/mod.rs:78-199) with a `title_prefix`/emoji field plus a shared `render_rule(width)` helper covering these banner/totals methods for both `DefaultDisplayFormatter` and `PlainDisplayFormatter`; markdown.rs differs meaningfully (Markdown headings) and should stay separate. Overlaps T29, which rewrites the `push_str(&format!(...))` sites in both files — do this one first, then re-derive T29's site list.

### T9. Drop the unused serde_json dependency: `serde_json` (Cargo.toml:71)
- Lenses: dead-code
- Risk: high
- Proposed fix: Remove the `serde_json` dependency (Cargo.toml:71) and its entry in the `webapp` feature list (Cargo.toml:21); confirmed via `cargo machete` (flags it) and `git grep -n serde_json -- src cli/src build.rs` (zero hits) — web.rs and graphql.rs serialize through axum's `Json<T>` with `serde::{Deserialize, Serialize}`, never `serde_json` directly. Verify with `just gate`, since the `webapp`-only feature combination is the one at risk.

### T10. Drop the unused juniper_graphql_ws dependency: `juniper_graphql_ws` (Cargo.toml:72)
- Lenses: dead-code
- Risk: high
- Proposed fix: Remove the `juniper_graphql_ws` dependency (Cargo.toml:72-75) and its entry in the `webapp` feature list (Cargo.toml:24); confirmed via `cargo machete` (flags it) and `git grep -n juniper_graphql_ws -- src cli/src build.rs` (zero hits) — src/graphql.rs:138 defines `Schema = RootNode<Query, Mutation, EmptySubscription<GraphQLContext>>` and src/web.rs only mentions the literal string `"/graphql/subscriptions"` as a URL passed to `graphiql`/`playground`, with no subscription websocket route ever mounted. Verify with `just gate`.

### T11. Split main_impl and de-duplicate its report dispatch: `main_impl` (cli/src/main.rs:14-116, 103 lines)
- Lenses: long-methods
- Risk: medium
- Proposed fix: Extract `async fn spawn_webserver_if_configured(config: &Config, set: &mut JoinSet<()>, rx: ...) -> bool` for the `#[cfg(feature = "webapp")]` block (lines 43-62), `async fn show_report(config: &Config, week_start_weekday: Weekday) -> Result<()>` to replace both verbatim copies of the weekly/single-day dispatch (lines 82-88 and the `#[cfg(not(feature = "tui"))]` copy at 94-100), and `async fn wait_for_background_tasks(set: JoinSet<()>, webserver_running: bool) -> Result<()>` for lines 103-113; the doc-fixer queue independently deletes redundant comments at lines 83, 86, 95 and 98 and relocates the misplaced "Load configuration…" comment at lines 15-16, so those exact lines may already have shifted when this runs, and T40 fixes a typo at line 19 inside the same function.

### T12. Drop the unused @react-hook/debounce dependency: `@react-hook/debounce` (site/package.json:19)
- Lenses: dead-code
- Risk: high
- Proposed fix: Remove `"@react-hook/debounce": "^4.0.0"` (site/package.json:19) and run `yarn install` to update yarn.lock; confirmed via `grep -rn '@react-hook/debounce' site/src` (zero hits) — site/src/hooks/useDebounce.ts implements its own `useState`/`useEffect`/`setTimeout` debounce instead of importing the package.

### T13. Drop the unused @uidotdev/usehooks dependency: `@uidotdev/usehooks` (site/package.json:22)
- Lenses: dead-code
- Risk: high
- Proposed fix: Remove `"@uidotdev/usehooks": "^2.4.1"` (site/package.json:22) and run `yarn install`; confirmed via `grep -rln uidotdev site/src` (zero hits) and it is not a peer dependency of any other declared package.

### T14. Drop the unused uuid dependency: `uuid` (site/package.json:32)
- Lenses: dead-code
- Risk: high
- Proposed fix: Remove `"uuid": "^11.1.0"` (site/package.json:32) and run `yarn install`; confirmed via `grep -rn uuid site/src` (zero hits outside package.json) and it is not a peer or transitive requirement of any other declared dependency checked.

### T15. Drop the unused webfontloader dependency: `webfontloader` (site/package.json:34)
- Lenses: dead-code
- Risk: high
- Proposed fix: Remove `"webfontloader": "^1.6.28"` (site/package.json:34) and run `yarn install`; confirmed via `grep -rin webfont site/src site/index.html site/vite.config.ts` (zero hits).

### T16. Delete the never-imported BorderedTableCell component: `BorderedTableCell` (site/src/components/BorderedTableCell.tsx:1)
- Lenses: dead-code
- Risk: low
- Proposed fix: Delete site/src/components/BorderedTableCell.tsx outright; confirmed via `git grep -n BorderedTableCell` scoped to site/ — only the file's own definition matches, with no imports in App.tsx, WeeklySummary.tsx, or any other page or component.

### T17. Delete the now-unused getVariant export: `getVariant` (site/src/components/Button/ButtonTypes.ts:10-21)
- Lenses: dead-code
- Risk: low
- Proposed fix: Delete `getVariant` (ButtonTypes.ts:10-21); confirmed via `grep -rn getVariant site/src` — the only non-definition hit is the commented-out call at site/src/components/Button/index.tsx:40 that T41 removes. Pair this with T41: either both land (delete the commented call and the helper) or neither, since the alternative resolution is to restore the call and actually wire the variant up.

### T18. Remove the unread `content` dependency from the debounced-save effect: DateEditor save effect (site/src/components/DateEditor.tsx:55)
- Lenses: idioms
- Risk: low
- Proposed fix: The debounced-save `useEffect` lists `content` in its dependency array but never reads `content` in the effect body, so it re-runs on every server refetch for no reason; change the array to `}, [debouncedData, updater, date, hasInitialized]);`.

### T19. Drop the inline style that duplicates the textarea's Tailwind classes: DateEditor textarea (site/src/components/DateEditor.tsx:77)
- Lenses: idioms
- Risk: low
- Proposed fix: The textarea carries `style={{ width: '50%', height: '100%' }}` even though `className` already sets `w-1/2` (width 50%), and `h-full` is the codebase's established way to say height 100% (see PageTemplate.tsx); drop the `style` prop entirely and use `className="w-1/2 p-2 border rounded mr-4 bg-gray-900 text-white h-full"`.

### T20. Extract a shared clipboard-copy-with-toast helper: `copyProjectNotesToClipboard` / `copyNotesToClipboard` (site/src/components/DateSummary.tsx:22, +1 site)
- Lenses: duplication
- Risk: medium
- Proposed fix: site/src/components/DateSummary.tsx:22-43 (`copyProjectNotesToClipboard`) and site/src/components/WeeklySummary.tsx:119-145 (`formatNotesTooltip` at 119 plus `copyNotesToClipboard` at 125) both join notes as `- ${note}` lines, `await navigator.clipboard.writeText(...)`, then fire `toast.success(msg, { position: 'top-right', autoClose: 2000, ... })` on success or `toast.error('Failed to copy to clipboard', { ... })` on failure; extract `copyNotesToClipboard(notes: string[], successMessage: string): Promise<void>` into site/src/utils/clipboard.ts with each caller supplying its own notes array and success message, leaving DateSummary's empty-notes early return and WeeklySummary's 'No notes for this day' tooltip fallback as caller-side concerns. Run this before T3, which restructures the WeeklySummary.tsx site into a `useNotesLookup` hook.

### T21. Stop panicking in Config::default() when the home directory can't be resolved: `Config::default` (src/config.rs:188)
- Lenses: idioms, opportunistic
- Risk: medium
- Proposed fix: `Config::default()` eagerly resolves the home directory with `Some(get_time_tracking_dir_with_override(None).unwrap().display().to_string())`, which panics wherever `dirs::home_dir()` returns None (a container with no `$HOME`, for example); set `data_directory: None` instead, since `get_data_directory()` / `get_time_tracking_dir_with_override(None)` already re-resolve lazily on demand and surface a `Result` error rather than panicking — that removes the panic and the eager work in one change, so no separate fallible-default plumbing is needed.

### T24. Replace the exists()-then-write template race with an atomic create-only write: `create_day_file_if_not_exists` (src/data_svc.rs:361)
- Lenses: opportunistic
- Risk: medium
- Proposed fix: There is a TOCTOU window between `file_path.exists()` and the subsequent `fs::write` in which a concurrent process can create and populate the day file, after which this write clobbers real content with the empty template; replace the pair with an atomic create-only open — `tokio::fs::OpenOptions::new().write(true).create_new(true).open(&file_path)` — treating `ErrorKind::AlreadyExists` as "someone else already created it" and skipping the write instead of racing. Same shape as T43 in the config path; fix both the same way.

### T25. Split get_weekly_summary into load, fold and finalize phases: `DataService::get_weekly_summary` (src/data_svc.rs:505-591, 87 lines)
- Lenses: long-methods
- Risk: low
- Proposed fix: Extract `async fn load_days(&self, dates: &[Date]) -> Result<Vec<DayLoad>>` for the JoinSet spawn, collect and reorder (lines 506-528), `fn fold_day(summary: &mut WeeklySummary, week_projects: &mut HashMap<String, (u32, Vec<String>)>, day_date: Date, content: Option<String>, parsed: Option<TimeTrackingData>)` for the per-day accumulation (lines 531-563), and `fn finalize_projects(week_projects: HashMap<String, (u32, Vec<String>)>) -> Vec<WeeklyProject>` for the sort and collect (lines 566-577), so `get_weekly_summary` reads as three calls; T26 deletes the thin `get_weekly_data` wrapper just below at lines 597-599, and T39 extracts the same load/fold shape from `web.rs::aggregate_week_days`, so consider whether the two can share the fold once both are extracted.

### T27. Stop cloning the whole CacheEntry to check its Copy metadata: `get_valid_entry` (src/data_svc.rs:610)
- Lenses: opportunistic
- Risk: medium
- Proposed fix: `get_valid_entry` clones the entire `CacheEntry` — raw file text plus parsed data — merely to inspect `cached_at` and `file_mod_time`, on the documented ~97-calls-per-navigation hot path; copy only `cached_at` and `file_mod_time` (both `Copy`) while holding the lock to decide validity, then re-lock briefly to take just the field the caller needs (`data` or `parsed`), so an invalid or metadata-only check never clones the payload.

### T41. Delete the eleven-month-old commented-out clsx entries: `Button` clsx call (site/src/components/Button/index.tsx:24, +2 lines)
- Lenses: dead-code
- Risk: low
- Proposed fix: Delete the three commented lines at site/src/components/Button/index.tsx:24, 40 and 41 (`// 'text-black',`, `// getVariant(type, disabled),`, `// block && 'w-full',`); `git blame -L 20,45 -- site/src/components/Button/index.tsx` dates all three to commit b3857caa on 2025-10-04, roughly eleven months old and well past the 30-day bar. Once line 40 is gone, `type` becomes an unused destructured field of `props` at line 20 — drop it from the destructure too. Pair with T17, which deletes the `getVariant` helper that call was the last reference to.

### T42. Add a shared local-date string helper for the five toISOString call sites: `toDateString` (site/src/hooks/useDateData.ts:11, +4 sites)
- Lenses: duplication
- Risk: low
- Proposed fix: `date.toISOString().split('T')[0]` is repeated at site/src/hooks/useDateData.ts:11, site/src/hooks/useWeekData.ts:6, site/src/components/DateSelector.tsx:23 (`formatDate`), and site/src/page/DateEditorPage.tsx:10 and :20; add a single `toDateString(date: Date): string` helper in a new site/src/utils/date.ts (no utils module exists yet) and use it at all five call sites. Critically, the helper MUST format from LOCAL components — `` `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}` `` — not `toISOString()`, because T2, T4 and T5 are the same UTC-versus-local bug seen at three of these call sites and a helper that merely centralises `toISOString()` would enshrine it; land this first, then T2, T4 and T5 become calls to it.
