# code-health execution batch — 2026-08-30

Source: `bughunt.md`, 14 findings marked `[x] execute` out of 17 active.
Branch: `codehealth/2026-08-30`. Worktree: `.worktrees/codehealth`.

Ranking is `impact = severity × blast-radius`, highest first. Effort is
reported but never folded into the rank.

## Verification gate (every task)

- `just gate` — check / test / clippy `-D warnings` / `fmt --check` across
  all three feature combos (default, `tui`-only, `webapp`-only), plus the
  `cargo tree -i` feature-isolation assertions. `site/build/` already exists.
- `cd site && yarn test --run && yarn lint` — for any task touching `site/`.

Baseline on the clean tree at the start of this batch: `just gate` green
(111 Rust tests), `yarn test` green (24 tests, 9 files), `yarn lint` clean.
There are **no** preexisting warnings to carry forward — any new warning is
this batch's fault.

**Never smoke-run the binary bare.** Always `--noedit --data-directory <tmp>`.

## Invariants this batch depends on

1. `DataService::get()` is a process-wide `OnceLock` shared by the CLI, the
   TUI, and the webapp; its `Config::get()`-derived prefix/suffix are the
   same values `AppState.config` holds, because `run_server` is always
   constructed from a `Config::get()` clone in `cli/src/main.rs:45`.
   B5 relies on this. If it ever stops holding, B5's shared-cache fix
   silently reads a different day file than the endpoint intended — pin it
   with a test, don't assert it in prose.
2. `read_day` maps `ErrorKind::NotFound` to `Ok(None)`, and
   `get_valid_entry` re-stats before returning a cache hit. B3 removes a
   redundant `Path::exists` that depends on both.
3. `tui()` always calls `ratatui::restore()` before returning, so the
   terminal is in cooked mode by the time `main.rs:58`'s `eprintln!` runs.
   B9 leaves that one call site alone for exactly this reason.
4. A `pub` item with zero repo-wide grep hits may still be linked from
   outside this repo (`time-tracking-nvim`). **No `pub` item may be removed
   or have its signature changed by this batch.** Additive `pub` is fine.

## Findings, in execution order

### B3 — `parse_day` blocking sync stat before the cache check
`src/data_svc.rs:327` · caching · impact 12 · effort S · risk low

Delete the early `if !file_path.exists() { return Ok(None); }`. It is a
blocking syscall on an async task, runs even on a full cache hit, and is
redundant with both `get_valid_entry`'s `tokio::fs::metadata` and
`read_day`'s `NotFound` → `Ok(None)` mapping. Called ~90-98× per TUI
navigation keystroke.

Covered by `test_read_nonexistent_file` and the FIFO/deleted-mid-read tests.

### B4 — day-data errors reduced to a bare 500 with no server-side log
`src/web.rs:213` · observability · impact 12 · effort S · **risk high**

`.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)` discards `read_day`'s
`.with_context(...)` detail. All four public entry points funnel through
`get_day_data_impl`. `aggregate_week_days`' `tracing::warn!` (web.rs:303)
logs a value that is already just the `StatusCode`.

Log before converting: `.map_err(|e| { tracing::error!(%date, error = %e,
"failed to read day data"); StatusCode::INTERNAL_SERVER_ERROR })?`. Drop
the now-redundant re-log in `aggregate_week_days`.

**risk high — no test coverage exists for `web.rs`/`graphql.rs` at all.**
Write the characterization test first (RED before GREEN).

### B5 — web/GraphQL endpoints bypass the memoized parse
`src/web.rs:227` · caching · impact 12 · effort S · risk low

Replace the direct `time_tracking_parser::parse_time_tracking_data` call
with `DataService::get().parse_day(&date)`, dropping the separate
`read_day` + manual parse. Every REST and GraphQL call currently reparses
from scratch; `aggregate_week_days` fans that out ×7 per week request, and
the SPA's 500ms autosave (`DateEditor.tsx:67` +
`useDateData.ts:26`'s `refetchQueries`) re-runs both queries per typing
pause — N pauses cost N×8 full reparses.

**Ordering: B5 lands after B4 and must preserve B4's error logging** — the
`map_err` B4 instruments is the one B5 rewrites. Do not regress it.
Depends on invariant 1 above; pin that with a test.

### B6 — `DateEditor` auto-resaves freshly-loaded content on every mount
`site/src/components/DateEditor.tsx:24` · frontend · impact 9 · effort S · risk medium

The init effect leaves `lastSentData.current` null, so 500ms later the
debounce effect re-writes the day file with content identical to what was
just read — on every page load. If the file is edited externally in that
window, the resave clobbers it with the stale in-browser copy.

Set `lastSentData.current = content` in the init effect alongside
`setLocalData(content)`/`setHasInitialized(true)`.

`site/src/components/__tests__/DateEditor.deps.test.tsx:16-20` currently
*works around* this as "a separate, pre-existing quirk". Per the one-way
rule: do not refactor that test to fit the new behavior beyond removing the
workaround it no longer needs — add a NEW test asserting no save fires on a
clean mount.

### B7 — mutation's `refetchQueries` pins the wrong date
`site/src/hooks/useDateData.ts:26` · frontend · impact 9 · effort S · **risk high**

`refetchQueries` pins `{date: <edited date>}`, a different Apollo cache
entry than the weekly-summary page holds (`{date: <week start>}`). With
the default cache-first policy, navigating back serves the stale pre-edit
result until a manual reload.

Replace the `{query, variables}` entries with operation-name strings
(`['FileContentForDate', 'DayDataForDate', 'WeekDataForDate']`) so every
active instance refetches regardless of its mounted date.

**Verify the operation names against the actual `gql` documents before
using them** — the strings must match the `query <Name>` in each document
or Apollo silently refetches nothing.

### B8 — Apollo `useQuery` error results are discarded
`site/src/hooks/useDateData.ts:13` · frontend · impact 8 · effort M · **risk high**

Both hooks destructure only `data`, and `App.tsx` configures no global
error link. A failed query leaves `data` permanently `undefined` with no
log and no toast; in `DateEditor` that means `hasInitialized` never fires,
so the debounced-save effect never fires either — the user types for a
whole session and nothing is ever sent. The write path is already
instrumented (`useDateData.ts:33-36` does `console.error` + toast); the
read path is not.

Surface `error` from each `useQuery`, return it from the hooks, and show a
toast/inline error in `DateEditor`/`WeeklySummaryPage`. Gate the textarea
while the initial load is in an error state so typed input isn't lost.
Distinguish a genuine empty week from a failed query in `WeeklySummary`'s
"No data available" fallback.

No test file exists for either hook — write one.

### B9 — `eprintln!`/`println!` on the join tasks corrupt the alternate screen
`cli/src/main.rs:107` · observability · impact 6 · effort S · risk medium

`--serve` and `--tui` are independent bools with no `conflicts_with`, so
both tasks run concurrently while the TUI owns the alternate screen. The
`println!` at line 122 fires unconditionally on every combined launch,
racing `ratatui::init()`.

Delete the `eprintln!` at :107 and the `println!` at :122 — both are
redundant with the adjacent `tracing::error!`/`info!`. **Leave
`main.rs:58`'s `eprintln!` alone** (invariant 3).

### B10 — `open_in_editor` cannot spawn a multi-word `$EDITOR`/`$VISUAL`
`src/editor.rs:28` · correctness · impact 6 · effort S · **risk high**

`Command::new(&editor)` does no word splitting, so `EDITOR="code --wait"`,
`"emacsclient -c"`, `"subl -n -w"` all fail with `NotFound`. In the CLI
path this aborts the run; in the TUI it is caught and logged, silently
leaving the user unable to ever edit a day file.

Split the configured editor into program + args before building the
`Command`; first token is the program, remaining tokens become leading args
ahead of the file path. Single-token values must behave exactly as today.
Prefer `shlex::split` (handles quoted paths); `str::split_whitespace` is
the minimal fallback — pick one and say why in the commit.

**risk high — untested.** `cli/tests/common/mod.rs:35` points `EDITOR` at
the single-word `false` to neutralise the editor, so no test exercises a
multi-word value, and `editor.rs`'s unit tests never execute
`open_in_editor`. Write a failing test first.

### B11 — `get_valid_entry` clones both fields when each caller needs one
`src/data_svc.rs:646` · caching · impact 6 · effort M · risk low

The success path does `cache.get(date).cloned()` — a full `CacheEntry`
clone of both `data: Option<String>` and `parsed: Option<TimeTrackingData>`
— and each caller immediately discards the field it didn't ask for. Every
one of the ~90-98 cache-hit calls per keystroke clones a field it drops.

Change `get_valid_entry` to take a projecting closure
`select: impl Fn(&CacheEntry) -> Option<T>` and clone only `select(entry)`
under the lock; `get_cached_content` passes `|e| e.data.clone()`,
`get_cached_parsed` passes `|e| e.parsed.clone()`. Keep the single shared
validity check — the comment at line 609 documents a previous regression
here, so preserve its intent and update it.

### B12 — invalid `--date` silently falls back to today, exit 0, no log
`src/config.rs:479` · api-surface · impact 4 · effort S · **risk high**

`resolve_requested_date` `eprintln!`s and returns `today_date()`; the
process exits 0 with a full report for the wrong day. Under `--tui` the
message is hidden by the alternate screen for the whole session, and it is
never written to the live `tracing` log.

`resolve_requested_date` is private (`config.rs:479`, no `pub`), so
changing it to return `Result<Date>` is not an API break.

**Constraint: do not let this become a panic.** `Config::get()` →
`init()` → `.expect("Could not load configuration")`, so propagating the
error out of `Config::load` alone converts a typo'd date into a panic —
strictly worse than today. The fix must pair with a fallible accessor:
add `pub fn try_get() -> anyhow::Result<&'static Config> {
Self::try_init(true) }` (additive, mirrors the existing
`try_get_no_args`) and switch `cli/src/main.rs:21` to `Config::try_get()?`
so a bad date exits non-zero through `anyhow` with a clear message.
Also emit `tracing::warn!`/the error at the failure point so it is durably
logged regardless of screen state.

No test in `config.rs`'s suite exercises this branch — write one.

### B13 — cache map has no sweep; stale entries live forever
`src/data_svc.rs:163` · caching · impact 4 · effort S · risk low

The 30s TTL is enforced only by `get_valid_entry` refusing to *return* a
stale entry; nothing removes it. `clear_cache` has zero prod call sites and
`invalidate_date` only touches the one edited date. A `--serve` daemon
accumulates one entry (raw content + parsed struct) per distinct date ever
requested, unbounded, for the process lifetime.

Sweep on insert in `cache_content`/`cache_parsed`:
`cache.retain(|_, e| now.duration_since(e.cached_at).is_ok_and(|d| d.as_secs() < self.cache_timeout))`.
O(n) over a map that this change keeps bounded.

### B15 — SPA catch-all returns 200 for unmatched `/api` and `/graphql`
`src/web.rs:142` · api-surface · impact 4 · effort S · risk low

`FallbackBehavior::Ok` forces HTTP 200 with index.html for any unresolved
path, so `GET /api/dayz`, `/api/day/2026-01-01/extra`, and
`/graphql/nonexistent` all look like successes to a probing monitor or a
typo'd client.

Scope the SPA fallback to non-API paths: return `StatusCode::NOT_FOUND`
for unmatched `/api/*` and `/graphql/*` before the request reaches
`fallback_service`. Registered routes must be unaffected — assert both
halves.

### B16 — failed watch retarget breaks the warn+status-line convention
`src/tui/app.rs:1745` · observability · impact 2 · effort S · **risk high**

Every other fallible path in `App` pairs `tracing::warn!`/`error!` with
`self.set_status(...)` — the clipboard fix's own comment (app.rs:1259-1262)
records *why*: a log-only failure "went to a log file the alternate screen
hides". `retarget_watch`'s `Err` arm is log-only, so the mtime watch stops
silently and external edits stop being detected for the rest of the session.

Add `self.set_status(format!("Could not watch for external changes: {e}"))`
beside the existing `tracing::warn!`.

No test exercises this `Err` branch — write one.

### B17 — `--stdin` silently drops `--serve`/`--week`/`--tui`/`--noedit`
`cli/src/main.rs:23` · api-surface · impact 1 · effort S · risk low

`main_impl` returns immediately after `show_single_day_stdin`, before the
other flags are consulted. `--stdin --serve --port 3000` starts no server
and says nothing.

Warn naming the ignored flags (via `tracing::warn!` — stdin mode writes the
report to stdout, so keep stdout clean for the report). Honouring the
combinations is out of scope for this batch.

## Out of scope

Unmarked in `bughunt.md`, not part of this batch: B1 (Critical), B2, B14,
and the one unmarked Low. Leave them in the file untouched.

Anything whose only correct fix is a big rewrite, a `pub` signature break,
or an architectural change becomes a `decision-needed` marker in
`bughunt.md` and is skipped — it is never auto-applied.

## Per-finding contract

1. Read the finding. For **risk high** (B4, B7, B8, B10, B12, B16), write a
   regression/characterization test that reproduces the bug and confirm it
   FAILS on unchanged code; commit as
   `test: characterize <unit> before fix [B<n>]`.
2. Apply the fix; the test goes GREEN.
3. Run the full gate for every recorded config (Rust: `just gate`;
   frontend tasks also `yarn test --run` + `yarn lint`). Fix new warnings
   this change introduced; leave unrelated preexisting ones alone.
4. `todo-parser bughunt.md --strip B<n>`.
5. `git add -A && git commit -m 'fix(<category>): <summary> [B<n>]'` — the
   fix and its strip land in **one** commit, so each fix stays
   independently revertable.

Milestone full-suite runs after every 5 findings and at each bucket end.

Existing tests are read as evidence and never refactored to fit a fix; new
regression tests are added instead.
