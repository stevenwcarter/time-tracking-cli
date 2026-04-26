# Code Health: 14 Correctness, Observability, and Quality Fixes

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply 14 surgical fixes to eliminate correctness bugs, observability gaps, and code quality issues identified in the code health audit.

**Architecture:** All changes are in-place edits to existing files — no new files, no new abstractions. Backend fixes touch `src/graphql.rs`, `src/display/mod.rs`, `src/web.rs`, `src/logging.rs`, and `Cargo.toml`. Frontend fixes touch five files in `site/src/`.

**Tech Stack:** Rust (Axum, Juniper, Tokio, tracing), React 18, TypeScript, Apollo Client, Vitest.

---

## Task 1: Fix GraphQL cache bugs (C1 + C2) — `src/graphql.rs`

**Files:**
- Modify: `src/graphql.rs`

**What to know:**
- `DataService` is the caching layer (`src/data_svc.rs`). It's a process-wide singleton accessed via `DataService::get()`.
- `create_day_file_if_not_exists(&date)` creates the file with a template if absent and returns the `PathBuf`.
- `read_day(&date)` returns `Result<Option<String>>` — `Ok(None)` means the file is absent or empty.
- The fix also removes the `create_template_content` import (no longer needed after C2).
- `get_time_tracking_dir` is still used by `update_file_content`, so keep it.

- [ ] **Step 1: Apply both fixes to `src/graphql.rs`**

Replace the entire file with the following:

```rust
use juniper::{EmptySubscription, FieldResult, RootNode};
use time::Date;
use tokio::fs;

use crate::{
    DATE_FORMAT, DataService,
    context::GraphQLContext,
    get_time_tracking_dir, get_week_dates, parse_weekday,
    web::{DayData, WeekData, aggregate_week_days, get_day_data_impl},
};

const INVALID_DATE_MSG: &str = "Invalid date format, expected YYYY-MM-DD";

pub struct Query;

#[juniper::graphql_object(Context = GraphQLContext)]
impl Query {
    #[graphql(name = "test")]
    pub async fn test(_context: &GraphQLContext) -> FieldResult<String> {
        Ok("Hello, GraphQL!".to_string())
    }

    #[graphql(name = "dataForDate")]
    pub async fn data_for_date(context: &GraphQLContext, date: String) -> FieldResult<DayData> {
        let state = &context.app_state;
        let date = Date::parse(&date, DATE_FORMAT)
            .map_err(|_| INVALID_DATE_MSG)?;

        get_day_data_impl(date, state).await.map_err(|e| e.into())
    }

    // C2: Use DataService so reads go through the shared 30-second cache and
    // template creation is handled in one place (DataService::create_day_file_if_not_exists).
    #[graphql(name = "fileContentForDate")]
    pub async fn file_content_for_date(
        _context: &GraphQLContext,
        date: String,
    ) -> FieldResult<String> {
        let date = Date::parse(&date, DATE_FORMAT)
            .map_err(|_| INVALID_DATE_MSG)?;

        DataService::get()
            .create_day_file_if_not_exists(&date)
            .await
            .map_err(|e| format!("Failed to create file: {}", e))?;

        let content = DataService::get()
            .read_day(&date)
            .await
            .map_err(|e| format!("Failed to read file: {}", e))?
            .unwrap_or_default();

        Ok(content)
    }

    #[graphql(name = "weekDataForDate")]
    pub async fn week_data_for_date(
        context: &GraphQLContext,
        date: String,
        week_start_day: Option<String>,
    ) -> FieldResult<WeekData> {
        let state = &context.app_state;
        let date = Date::parse(&date, DATE_FORMAT)
            .map_err(|_| INVALID_DATE_MSG)?;

        let week_start_day = week_start_day
            .or_else(|| state.config.week_start_day.clone())
            .unwrap_or_else(|| "Saturday".to_string());

        let week_start_weekday =
            parse_weekday(&week_start_day).map_err(|e| format!("Invalid week start day: {}", e))?;

        let week_dates = get_week_dates(&date, week_start_weekday);

        let (days, project_summaries, total_week_hours, total_dead_hours) =
            aggregate_week_days(&week_dates, state).await;

        let start_date = week_dates
            .first()
            .ok_or("week_dates is empty")?
            .format(DATE_FORMAT)
            .map_err(|e| format!("Failed to format start date: {e}"))?;
        let end_date = week_dates
            .last()
            .ok_or("week_dates is empty")?
            .format(DATE_FORMAT)
            .map_err(|e| format!("Failed to format end date: {e}"))?;

        Ok(WeekData {
            start_date,
            end_date,
            total_hours: total_week_hours,
            dead_time_hours: total_dead_hours,
            days,
            project_summaries,
        })
    }
}

pub struct Mutation;

#[juniper::graphql_object(Context = GraphQLContext)]
impl Mutation {
    #[graphql(name = "testMutation")]
    pub async fn test_mutation(_context: &GraphQLContext) -> FieldResult<String> {
        Ok("Hello from Mutation!".to_string())
    }

    // C1: Invalidate the DataService cache after writing so subsequent reads
    // return fresh content instead of stale data for up to 30 seconds.
    #[graphql(name = "updateFileContent")]
    pub async fn update_file_content(
        _context: &GraphQLContext,
        date: String,
        content: String,
    ) -> FieldResult<String> {
        let date = Date::parse(&date, DATE_FORMAT)
            .map_err(|_| INVALID_DATE_MSG)?;

        let time_tracking_dir = get_time_tracking_dir()
            .map_err(|e| format!("Failed to get time tracking directory: {}", e))?;

        fs::create_dir_all(&time_tracking_dir)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        let date_str = date
            .format(DATE_FORMAT)
            .map_err(|e| format!("Failed to format date: {e}"))?;
        let file_path = time_tracking_dir.join(format!("{}.md", date_str));

        fs::write(&file_path, &content)
            .await
            .map_err(|e| format!("Failed to write file: {}", e))?;

        DataService::get().invalidate_date(&date).await;

        Ok(format!("Successfully updated file for date {}", date_str))
    }
}

pub type Schema = RootNode<Query, Mutation, EmptySubscription<GraphQLContext>>;

pub fn create_schema() -> Schema {
    Schema::new(Query, Mutation, EmptySubscription::new())
}
```

- [ ] **Step 2: Build to verify**

```bash
cargo build --release -p cli
```

Expected: compiles with no errors.

- [ ] **Step 3: Run tests**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/graphql.rs
git commit -m "fix: route GraphQL fileContentForDate through DataService cache (C2) and invalidate cache after updateFileContent (C1)"
```

---

## Task 2: Remove dead `else` branch in `show_single_day` (C3) — `src/display/mod.rs`

**Files:**
- Modify: `src/display/mod.rs`

**What to know:**
- `create_day_file_if_not_exists` always ensures the file exists before returning. The `else` branch (printing "Created new time tracking file") is unreachable.
- `tracing::info!` already imported via `use tracing::info;` at the top of this file.

- [ ] **Step 1: Replace the dead if/else block in `show_single_day`**

Find this block in `show_single_day` (around line 264):

```rust
if !noedit {
    if file_path.exists() {
        info!(
            "Opening existing time tracking file: {}",
            file_path.display()
        );
    } else {
        println!("Created new time tracking file: {}", file_path.display());
    }

    // Open the file in the default editor
    open_in_editor(&file_path)?;
```

Replace with:

```rust
if !noedit {
    info!("Opening time tracking file: {}", file_path.display());

    // Open the file in the default editor
    open_in_editor(&file_path)?;
```

- [ ] **Step 2: Build and test**

```bash
cargo build --release -p cli && cargo test
```

Expected: compiles cleanly, all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/display/mod.rs
git commit -m "fix: remove unreachable else branch in show_single_day (C3)"
```

---

## Task 3: Fix `web.rs` — week start day + sort order + startup logging (C4, C5, O1)

**Files:**
- Modify: `src/web.rs`

**What to know:**
- `get_week_data_by_date` is the path-based handler (`/api/week/{date}`). It currently passes `"Saturday".to_string()` hardcoded instead of reading from config.
- `state.config.get_week_start_day()` returns `&str` (from `Config::get_week_start_day()`). Call `.to_string()` on it.
- `aggregate_week_days` builds `project_summaries` from a `HashMap` — iteration order is non-deterministic. Sort after collecting.
- The two `println!` startup messages should become `tracing::info!` (already imported as `use tracing::debug;` — add `info` to the import).

- [ ] **Step 1: Fix `get_week_data_by_date` (C4)**

Find:

```rust
async fn get_week_data_by_date(
    State(state): State<AppState>,
    Path(date_str): Path<String>,
) -> Result<Json<WeekData>, StatusCode> {
    let date = Date::parse(&date_str, DATE_FORMAT).map_err(|_| StatusCode::BAD_REQUEST)?;

    get_week_data_impl(date, "Saturday".to_string(), &state).await
}
```

Replace with:

```rust
async fn get_week_data_by_date(
    State(state): State<AppState>,
    Path(date_str): Path<String>,
) -> Result<Json<WeekData>, StatusCode> {
    let date = Date::parse(&date_str, DATE_FORMAT).map_err(|_| StatusCode::BAD_REQUEST)?;

    get_week_data_impl(date, state.config.get_week_start_day().to_string(), &state).await
}
```

- [ ] **Step 2: Sort `project_summaries` in `aggregate_week_days` (C5)**

Find this block near the end of `aggregate_week_days`:

```rust
    let project_summaries: Vec<ProjectSummary> = week_projects
        .into_iter()
        .map(|(name, total_hours)| ProjectSummary { name, total_hours })
        .collect();

    (days, project_summaries, total_week_hours, total_dead_hours)
```

Replace with:

```rust
    let mut project_summaries: Vec<ProjectSummary> = week_projects
        .into_iter()
        .map(|(name, total_hours)| ProjectSummary { name, total_hours })
        .collect();
    project_summaries.sort_unstable_by(|a, b| a.name.cmp(&b.name));

    (days, project_summaries, total_week_hours, total_dead_hours)
```

- [ ] **Step 3: Replace `println!` with `tracing::info!` in `run_server` (O1)**

First, update the tracing import at the top of `src/web.rs`. Find:

```rust
use tracing::debug;
```

Replace with:

```rust
use tracing::{debug, info};
```

Then find the two `println!` calls in `run_server`:

```rust
    println!(
        "🌐 Time Tracking Web Server running on http://localhost:{}",
        port
    );
    println!("📊 Access your time tracking data via the web interface");
```

Replace with:

```rust
    info!("Time Tracking Web Server running on http://localhost:{}", port);
    info!("Access your time tracking data via the web interface");
```

- [ ] **Step 4: Build and test**

```bash
cargo build --release -p cli && cargo test
```

Expected: compiles cleanly, all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/web.rs
git commit -m "fix: use config week_start_day in path handler (C4), sort project_summaries (C5), use tracing for startup log (O1)"
```

---

## Task 4: Fix logging — disable ANSI, enable RUST_LOG (O2 + O3)

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/logging.rs`

**What to know:**
- `tracing-subscriber` already exists as a dependency. Adding `features = ["env-filter"]` unlocks `EnvFilter`.
- `EnvFilter::from_default_env()` reads the `RUST_LOG` environment variable at startup (e.g. `RUST_LOG=debug`, `RUST_LOG=time_tracking_cli=trace`). If `RUST_LOG` is unset, the default level (ERROR for external crates, INFO for the app) applies.
- `.with_ansi(false)` stops writing ANSI escape codes into the log file.
- Order matters: `Cargo.toml` change must be saved before `logging.rs` references the new type.

- [ ] **Step 1: Add `env-filter` feature to `tracing-subscriber` in `Cargo.toml`**

Find:

```toml
tracing-subscriber = "0.3.20"
```

Replace with:

```toml
tracing-subscriber = { version = "0.3.20", features = ["env-filter"] }
```

- [ ] **Step 2: Update `src/logging.rs`**

Replace the entire file with:

```rust
use tokio::fs;

use anyhow::{Context, Result};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// Initialize tracing and return the worker guard.
/// The caller must hold the guard for the process lifetime to ensure logs are flushed at shutdown.
pub async fn init_tracing() -> Result<WorkerGuard> {
    dotenvy::dotenv().ok();

    let log_path = dirs::data_local_dir().context("Could not get local directory")?;

    let log_path = log_path.join("time-tracking-cli");

    fs::create_dir_all(&log_path)
        .await
        .context("Could not create log directory")?;

    let file_appender = tracing_appender::rolling::never(&log_path, "log.txt");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    Ok(guard)
}
```

- [ ] **Step 3: Build and test**

```bash
cargo build --release -p cli && cargo test
```

Expected: compiles cleanly, all tests pass.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml src/logging.rs
git commit -m "fix: disable ANSI codes in log file (O2) and enable RUST_LOG env var via EnvFilter (O3)"
```

---

## Task 5: Remove double `React.StrictMode` (C6) — `site/src/App.tsx`

**Files:**
- Modify: `site/src/App.tsx`

**What to know:**
- `main.tsx` already wraps the entire app in `<React.StrictMode>`. The extra wrapper in `App.tsx` causes double-wrapping, making effects fire an extra time in development.
- After removing the wrapper, `React` may no longer be needed as an import if no other JSX uses it explicitly — but `React.lazy` is still used so the import stays.

- [ ] **Step 1: Remove the `<React.StrictMode>` wrapper from `App.tsx`**

Find:

```tsx
export const App = () => {
  return (
    <React.StrictMode>
      <ApolloProvider client={apolloClient}>
        <Suspense fallback={<LoadingSpinner />}>
          <ToastContainer />
          <RouterProvider router={router} />
        </Suspense>
      </ApolloProvider>
    </React.StrictMode>
  );
};
```

Replace with:

```tsx
export const App = () => {
  return (
    <ApolloProvider client={apolloClient}>
      <Suspense fallback={<LoadingSpinner />}>
        <ToastContainer />
        <RouterProvider router={router} />
      </Suspense>
    </ApolloProvider>
  );
};
```

- [ ] **Step 2: Lint and test**

```bash
cd site && yarn lint && yarn test
```

Expected: no lint errors, all tests pass.

- [ ] **Step 3: Commit**

```bash
git add site/src/App.tsx
git commit -m "fix: remove duplicate React.StrictMode wrapper in App.tsx (C6)"
```

---

## Task 6: Fix timezone bug in date picker (C7) — `site/src/components/DateSelector.tsx`

**Files:**
- Modify: `site/src/components/DateSelector.tsx`

**What to know:**
- A `<input type="date">` returns a bare `YYYY-MM-DD` string in `e.target.value` (e.g. `"2025-04-25"`).
- `new Date("2025-04-25")` is parsed as **UTC midnight** by the JS spec. For users in UTC-5, this is 7pm on April 24th locally, so `.toISOString().split('T')[0]` returns `"2025-04-24"` — the wrong day.
- `new Date("2025-04-25T00:00:00")` (no `Z`) is parsed as **local midnight**, which is correct.

- [ ] **Step 1: Fix the `onChange` handler**

Find:

```tsx
      onChange={(e) => setDate(new Date(e.target.value))}
```

Replace with:

```tsx
      onChange={(e) => setDate(new Date(e.target.value + 'T00:00:00'))}
```

- [ ] **Step 2: Lint and test**

```bash
cd site && yarn lint && yarn test
```

Expected: no lint errors, all tests pass.

- [ ] **Step 3: Commit**

```bash
git add site/src/components/DateSelector.tsx
git commit -m "fix: parse date picker value as local midnight to avoid UTC timezone shift (C7)"
```

---

## Task 7: Fix always-false `skip` condition in `useDateData` (C8) — `site/src/hooks/useDateData.ts`

**Files:**
- Modify: `site/src/hooks/useDateData.ts`

**What to know:**
- `date` is typed as `Date` (non-optional). `!date` is always `false` — a `Date` object is always truthy.
- `dateString` is a `string` derived from `date.toISOString().split('T')[0]`. It could theoretically be empty if date is an invalid Date object. Using `skip: !dateString` is the correct guard.

- [ ] **Step 1: Change both `skip` conditions**

Find (appears twice in the file):

```typescript
    skip: !date,
```

Replace both occurrences with:

```typescript
    skip: !dateString,
```

- [ ] **Step 2: Lint and test**

```bash
cd site && yarn lint && yarn test
```

Expected: no lint errors, all tests pass.

- [ ] **Step 3: Commit**

```bash
git add site/src/hooks/useDateData.ts
git commit -m "fix: guard Apollo queries with !dateString instead of !date (C8)"
```

---

## Task 8: Type `parsedData` and format hours in `DateSummary` (C9 + Q2) — `site/src/components/DateSummary.tsx`

**Files:**
- Modify: `site/src/components/DateSummary.tsx`

**What to know:**
- The component currently uses `parsedData: any`, silencing all TypeScript checks on property accesses.
- `totalHours` and `deadTimeHours` are raw floats from the API (e.g., `7.083333...`). `.toFixed(2)` formats them to 2 decimal places.
- The `?.toFixed(2)` optional chain handles the case where the field is `null` or `undefined`.
- The interface shape mirrors `DayData` in `src/web.rs` — `date`, `totalHours`, `deadTimeHours`, `startTime`, `endTime`, `warnings`, `projects`.

- [ ] **Step 1: Replace the entire file with a typed version**

```tsx
import { toast } from 'react-toastify';

interface ParsedProject {
  name: string;
  totalHours: number;
  notes: string[];
}

interface ParsedDayData {
  date: string;
  totalHours: number;
  deadTimeHours: number;
  startTime: string | null;
  endTime: string | null;
  warnings: string[];
  projects: ParsedProject[];
}

export const DateSummary = (props: { parsedData: ParsedDayData }) => {
  const { parsedData } = props;

  const copyProjectNotesToClipboard = async (projectName: string, notes: string[]) => {
    if (notes.length === 0) return;

    const formattedNotes = notes.map((note) => `- ${note}`).join('\n');

    try {
      await navigator.clipboard.writeText(formattedNotes);
      toast.success(`${projectName} notes copied to clipboard!`, {
        position: 'top-right',
        autoClose: 2000,
        hideProgressBar: false,
        closeOnClick: true,
        pauseOnHover: true,
        draggable: true,
      });
    } catch (err) {
      toast.error('Failed to copy to clipboard', {
        position: 'top-right',
        autoClose: 2000,
      });
    }
  };

  return (
    <div className="px-2 overflow-y-auto">
      <h2 className="text-2xl font-bold mb-4">Summary for {parsedData.date}</h2>
      <p className="mb-2">Total Hours: {parsedData.totalHours?.toFixed(2)}</p>
      <p className="mb-2">Dead Time Hours: {parsedData.deadTimeHours?.toFixed(2)}</p>
      <p className="mb-4">
        Start Time: {parsedData.startTime} - End Time: {parsedData.endTime}
      </p>
      {parsedData.warnings.length > 0 && (
        <div className="mt-4 p-2 text-black bg-yellow-200 border border-yellow-400 rounded">
          <h3 className="font-semibold">Warnings:</h3>
          <ul className="list-disc list-inside">
            {parsedData.warnings.map((warning: string, index: number) => (
              <li key={index}>{warning}</li>
            ))}
          </ul>
        </div>
      )}
      <h3 className="text-xl font-semibold mt-8 mb-2">Projects:</h3>
      {parsedData.projects.map((project) => (
        <div key={project.name} className="mb-4">
          <div className="font-semibold flex text-lg">
            {project.name}
            <div className="text-sm ml-2 self-center">
              ({project.totalHours} {project.totalHours === 1 ? 'hour' : 'hours'})
            </div>
          </div>
          <ul
            className="list-disc list-inside cursor-pointer hover:bg-gray-800 p-2 rounded transition-colors"
            onClick={() => copyProjectNotesToClipboard(project.name, project.notes)}
            title="Click to copy notes to clipboard"
          >
            {project.notes.map((note: string, index: number) => (
              <li key={index}>{note}</li>
            ))}
          </ul>
        </div>
      ))}
    </div>
  );
};

export default DateSummary;
```

- [ ] **Step 2: Check the caller site in `DateEditor.tsx` still compiles**

`DateEditor.tsx` passes `parsedData || { date: 'N/A', projects: [], warnings: [] }` to `DateSummary`. The fallback object is missing `totalHours`, `deadTimeHours`, `startTime`, `endTime`. Update the fallback in `site/src/components/DateEditor.tsx`:

Find:

```tsx
        <DateSummary parsedData={parsedData || { date: 'N/A', projects: [], warnings: [] }} />
```

Replace with:

```tsx
        <DateSummary
          parsedData={parsedData || {
            date: 'N/A',
            totalHours: 0,
            deadTimeHours: 0,
            startTime: null,
            endTime: null,
            projects: [],
            warnings: [],
          }}
        />
```

- [ ] **Step 3: Lint and test**

```bash
cd site && yarn lint && yarn test
```

Expected: no TypeScript errors, no lint errors, all tests pass.

- [ ] **Step 4: Commit**

```bash
git add site/src/components/DateSummary.tsx site/src/components/DateEditor.tsx
git commit -m "fix: type parsedData with ParsedDayData interface (Q2) and format hours with .toFixed(2) (C9)"
```

---

## Task 9: Remove stale sample data comment (Q1) — `site/src/page/WeeklySummaryPage.tsx`

**Files:**
- Modify: `site/src/page/WeeklySummaryPage.tsx`

**What to know:**
- There is a large multi-line comment near the top of the file (before the `export const Homepage` declaration) containing a JSON sample payload. It is a development artifact with no runtime effect and should be deleted.

- [ ] **Step 1: Delete the stale comment**

The comment starts with:
```
// Sample data from useWeekData for 2025-09-30
// { "weekDataForDate": { "startDate": ...
```

Delete all lines of this comment block (everything from `// Sample data from useWeekData for 2025-09-30` through the closing `// ... } }` line).

- [ ] **Step 2: Lint and test**

```bash
cd site && yarn lint && yarn test
```

Expected: no errors, all tests pass.

- [ ] **Step 3: Commit**

```bash
git add site/src/page/WeeklySummaryPage.tsx
git commit -m "chore: remove stale sample data comment from WeeklySummaryPage (Q1)"
```

---

## Verification

After all tasks are complete, run the full test suite:

```bash
cargo test && cd site && yarn lint && yarn test
```

Expected: all Rust tests pass, no TypeScript/lint errors, all Vitest tests pass.
