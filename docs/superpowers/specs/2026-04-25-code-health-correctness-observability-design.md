# Code Health: Correctness, Observability, and Code Quality Fixes

**Date:** 2026-04-25
**Scope:** 14 surgical fixes across Rust backend and React frontend. No rewrites. Security issues (S1–S3) are out of scope — this is a local-only tool.

---

## Backend Correctness (C1–C5)

### C1 — Cache not invalidated after GraphQL `updateFileContent` mutation
**File:** `src/graphql.rs` — `update_file_content` mutation  
**Fix:** After `fs::write(&file_path, &content).await`, call `DataService::get().invalidate_date(&date).await`. This ensures the next read after a save returns fresh data instead of serving stale cached content for up to 30 seconds.

### C2 — GraphQL `fileContentForDate` bypasses DataService cache
**File:** `src/graphql.rs` — `file_content_for_date` query  
**Fix:** Replace the direct `tokio::fs::read_to_string` call and the manual template-creation logic with:
1. `DataService::get().create_day_file_if_not_exists(&date).await` — creates file with template if missing
2. `DataService::get().read_day(&date).await` — reads through the shared cache; returns `Ok(None)` for an empty file, which should be mapped to `Ok(String::new())` to match the original behavior

This makes the GraphQL query use the same caching path as the REST endpoints and removes duplicated logic.

### C3 — Dead `else` branch in `show_single_day`
**File:** `src/display/mod.rs` — `show_single_day` function  
**Fix:** Remove the `if file_path.exists() { ... } else { ... }` block. Replace with a single `info!("Opening time tracking file: {}", file_path.display())` log statement. The `else` branch was unreachable because `create_day_file_if_not_exists` guarantees the file exists before the check.

### C4 — `get_week_data_by_date` hardcodes "Saturday" week start
**File:** `src/web.rs` — `get_week_data_by_date` handler  
**Fix:** Replace the hardcoded `"Saturday".to_string()` argument with `state.config.get_week_start_day().to_string()`, making the path-based handler consistent with the query-param handler.

### C5 — Non-deterministic `project_summaries` ordering
**File:** `src/web.rs` — `aggregate_week_days` function  
**Fix:** After converting the `HashMap<String, f64>` to `Vec<ProjectSummary>`, add `.sort_unstable_by(|a, b| a.name.cmp(&b.name))`. This gives stable, alphabetical ordering across requests and process restarts.

---

## Frontend Correctness (C6–C9)

### C6 — Double `React.StrictMode` wrapping
**File:** `site/src/App.tsx`  
**Fix:** Remove the `<React.StrictMode>` wrapper from `App.tsx`. The authoritative one lives in `main.tsx`. The double-wrap causes effects to triple-fire in development.

### C7 — Timezone bug in `DateSelector` date picker
**File:** `site/src/components/DateSelector.tsx`  
**Fix:** Change `new Date(e.target.value)` to `new Date(e.target.value + 'T00:00:00')` in the `onChange` handler. The date `<input>` returns a `YYYY-MM-DD` string; without the time suffix, `new Date()` parses it as UTC midnight, which renders as the previous day for users west of UTC.

### C8 — `skip: !date` never skips in `useDateData`
**File:** `site/src/hooks/useDateData.ts`  
**Fix:** Change both `skip: !date` conditions to `skip: !dateString`. `date` is a non-optional `Date` object and is always truthy; `dateString` is a string derived from it and is the correct guard.

### C9 — Raw unformatted float in `DateSummary`
**File:** `site/src/components/DateSummary.tsx`  
**Fix:** Replace `{parsedData.totalHours}` and `{parsedData.deadTimeHours}` with `{parsedData.totalHours?.toFixed(2)}` and `{parsedData.deadTimeHours?.toFixed(2)}`. Matches the `.toFixed(2)` formatting used consistently in `WeeklySummary.tsx`.

---

## Observability (O1–O3)

### O1 — `println!` in web server startup
**File:** `src/web.rs` — `run_server` function  
**Fix:** Replace the two `println!` startup messages with `tracing::info!` calls. Remove emoji from log messages (emoji is noise in structured log output).

### O2 — ANSI escape codes in log file
**File:** `src/logging.rs`  
**Fix:** Change `.with_ansi(true)` to `.with_ansi(false)`. ANSI color codes in log files break `cat`, `grep`, `less`, and log analysis tools.

### O3 — `RUST_LOG` env var has no effect
**File:** `src/logging.rs` + `Cargo.toml`  
**Fix:**
1. Add `env-filter` to the `tracing-subscriber` feature list in `Cargo.toml`: `tracing-subscriber = { version = "0.3.20", features = ["env-filter"] }`
2. Chain `.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())` onto the subscriber builder. This enables `RUST_LOG=debug`, `RUST_LOG=time_tracking_cli=trace`, etc.

---

## Code Quality (Q1–Q2)

### Q1 — Stale sample data comment in `WeeklySummaryPage.tsx`
**File:** `site/src/page/WeeklySummaryPage.tsx`  
**Fix:** Delete the large multi-line JSON comment at the top of the file (development artifact).

### Q2 — `parsedData: any` in `DateSummary`
**File:** `site/src/components/DateSummary.tsx`  
**Fix:** Define a local `ParsedDayData` interface with the fields the component uses (`date`, `totalHours`, `deadTimeHours`, `startTime`, `endTime`, `warnings`, `projects`) and replace `parsedData: any` with `parsedData: ParsedDayData`. Define a `ParsedProject` interface for the project items as well.

---

## What is NOT changing

- No authentication or CORS changes (local-only tool, S1–S3 out of scope)
- No architectural changes — all fixes are surgical edits to existing files
- No new abstractions introduced
- Cache timeout (30s) and cache strategy are unchanged

## Files touched

| File | Changes |
|------|---------|
| `src/graphql.rs` | C1, C2 |
| `src/display/mod.rs` | C3 |
| `src/web.rs` | C4, C5, O1 |
| `src/logging.rs` | O2, O3 |
| `Cargo.toml` | O3 (add env-filter feature) |
| `site/src/App.tsx` | C6 |
| `site/src/components/DateSelector.tsx` | C7 |
| `site/src/hooks/useDateData.ts` | C8 |
| `site/src/components/DateSummary.tsx` | C9, Q2 |
| `site/src/page/WeeklySummaryPage.tsx` | Q1 |
