# code-health bughunt batch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the 14 findings the user marked `[x] execute` in `bughunt.md`, one commit per finding, each stripping its own entry from `bughunt.md`.

**Architecture:** Surgical fixes plus three small testability seams — a `_with(svc, …)` variant of `web::get_day_data_impl`, a `build_router` extraction in `web.rs`, and an `open_in_editor_with(editor, path)` core in `editor.rs`. Every seam is *additive*: no existing `pub` signature changes. Findings land in impact order (severity × blast-radius), which also happens to satisfy the one ordering dependency (B4 before B5).

**Tech Stack:** Rust 2024 (tokio, axum 0.8, juniper, ratatui, clap), React 19 + Apollo Client 3 + Vitest.

**Spec:** `docs/superpowers/specs/2026-08-30-codehealth-bughunt-batch-design.md`

## Global Constraints

- **Verification gate (Rust):** `just gate` — check / test / clippy `-D warnings` / `fmt --check` across default, `tui`-only, and `webapp`-only, plus the `cargo tree -i` feature-isolation assertions. Requires `site/build/` to exist; it already does.
- **Verification gate (frontend, any task touching `site/`):** `cd site && yarn test --run && yarn lint && npx tsc --noEmit`. `yarn test`/`yarn lint` do **not** typecheck — `tsc --noEmit` is what catches a mock whose shape no longer matches a hook's return type.
- **Baseline on the clean tree:** `just gate` green (111 Rust tests), `yarn test` green (24 tests / 9 files), `yarn lint` clean. There are **zero** preexisting warnings. Any new warning belongs to this batch.
- **Never smoke-run the binary bare.** Always `--noedit --data-directory <tmp>`. A bare `ttcli` opens `$EDITOR` on the user's real `~/.time-tracking/` data.
- **No `pub` item may be removed or have its signature changed.** A `pub` item with zero repo-wide grep hits may still be linked from outside this repo (`time-tracking-nvim`). Additive `pub` / `pub(crate)` is fine.
- **Edition 2024** everywhere; `rustfmt.toml` matches. Run `cargo fmt --all`, never bare `rustfmt`.
- **Conventional commits** — enforced by Husky + commitlint.
- **One commit per finding**, containing both the code change and its `todo-parser bughunt.md --strip B<n>`. Never bulk-commit.
- **Existing tests are read as evidence, never refactored to fit a fix.** The single authorized exception is `site/src/components/__tests__/DateEditor.deps.test.tsx`, which currently *works around* the B6 bug and cannot survive its fix unchanged (Task 4).
- **Milestone full-suite run** after Tasks 5, 10, and 14.

---

### Task 1 (B3): Drop the blocking `Path::exists` from `parse_day`

**Files:**
- Modify: `src/data_svc.rs:325-329`
- Test: `src/data_svc.rs` (`#[cfg(test)] mod tests`, near `test_read_nonexistent_file`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: nothing later tasks rely on. `parse_day`'s signature is unchanged: `pub async fn parse_day(&self, date: &Date) -> Result<Option<TimeTrackingData>>`.

This is a performance fix with **no behavior change**, so the test is a *characterization* test: it must pass BEFORE and AFTER. That is the point — it pins the behavior the deletion must preserve.

- [ ] **Step 1: Write the characterization test**

Add to the `mod tests` block in `src/data_svc.rs`, immediately after `test_read_nonexistent_file`:

```rust
    #[tokio::test]
    async fn parse_day_for_a_missing_file_reads_as_absent() {
        let (service, _dir) = hermetic_service(60);

        let never_written = date!(2001 - 10 - 15);
        assert!(
            service.parse_day(&never_written).await.unwrap().is_none(),
            "a date with no day file must parse as absent, not error"
        );
        assert_eq!(
            service.parse_count(),
            0,
            "a missing file must not reach the parser at all"
        );
    }

    #[tokio::test]
    async fn parse_day_for_a_file_deleted_after_caching_reads_as_absent() {
        // The `Path::exists` guard this test outlives used to be the only
        // thing answering "no file" once an entry was already cached.
        // `get_valid_entry`'s re-stat and `read_day`'s NotFound mapping cover
        // it; this pins that they actually do.
        let (service, _dir) = hermetic_service(60);
        let test_date = date!(2026 - 08 - 24);
        let path = service.get_file_path(test_date).await.unwrap();

        tokio::fs::write(&path, "8-10 admin\n").await.unwrap();
        assert!(service.parse_day(&test_date).await.unwrap().is_some());

        tokio::fs::remove_file(&path).await.unwrap();
        assert!(
            service.parse_day(&test_date).await.unwrap().is_none(),
            "a deleted day file must read as absent even with a warm cache entry"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they PASS on unchanged code**

Run: `cargo test --lib data_svc::tests::parse_day_for_a`
Expected: **2 passed**. If either fails, stop — the test does not describe current behavior and the deletion in Step 3 is not safe.

- [ ] **Step 3: Delete the redundant existence check**

In `src/data_svc.rs`, `parse_day` currently opens:

```rust
    pub async fn parse_day(&self, date: &Date) -> Result<Option<TimeTrackingData>> {
        let file_path = self.get_file_path(*date).await?;

        if !file_path.exists() {
            return Ok(None);
        }

        if let Some(parsed) = self.get_cached_parsed(date, &file_path).await? {
```

Remove the `if !file_path.exists()` block so it reads:

```rust
    pub async fn parse_day(&self, date: &Date) -> Result<Option<TimeTrackingData>> {
        let file_path = self.get_file_path(*date).await?;

        if let Some(parsed) = self.get_cached_parsed(date, &file_path).await? {
```

Then extend the doc comment above `parse_day` with a sentence recording why no `exists()` check belongs here:

```rust
    /// Parse a day's time tracking data, using the cached parse when the
    /// backing file hasn't changed. This is the hot path for the TUI: a
    /// single navigation key can call this ~97 times, and on a full cache
    /// hit none of those calls should re-run the markdown parser.
    ///
    /// Deliberately does **no** up-front existence check. A synchronous
    /// `Path::exists` here was a blocking syscall on the async task that ran
    /// even on a full cache hit, ~97 times per keystroke, and answered a
    /// question two later steps already answer: `get_valid_entry` re-stats
    /// before certifying a cache hit, and `read_day` maps `NotFound` to
    /// `Ok(None)`.
```

- [ ] **Step 4: Run the gate**

Run: `just gate`
Expected: green. The two new tests appear in the lib count, which was 111 at the start of this batch.

- [ ] **Step 5: Strip and commit**

```bash
todo-parser bughunt.md --strip B3
git add -A
git commit -m 'fix(caching): drop the blocking existence stat from parse_day [B3]'
```

---

### Task 2 (B4): Log day-data read failures before collapsing them to a 500

**Files:**
- Modify: `Cargo.toml` (add `"util"` to the optional `tower` dependency's features, if `tower::ServiceExt` does not already resolve)
- Modify: `src/data_svc.rs` (widen the test-only `parse_count` accessor to `pub(crate)`)
- Modify: `src/web.rs:203-260` (`get_day_data_impl`), `src/web.rs:296-306` (`aggregate_week_days`)
- Test: `src/web.rs` — new `#[cfg(test)] mod tests` at the end of the file

**Interfaces:**
- Consumes: `DataService::new_with_dir(cache_timeout_seconds: u64, data_dir: PathBuf, parse_settings: ParseSettings) -> DataService` (existing).
- Produces:
  - `pub(crate) async fn get_day_data_impl_with(svc: &DataService, date: Date, state: &AppState) -> Result<DayData, StatusCode>` — the body of the old `get_day_data_impl`, with the service injected. **Task 3 rewrites this function's body; Task 12's router tests do not touch it.**
  - `pub async fn get_day_data_impl(date: Date, state: &AppState) -> Result<DayData, StatusCode>` — unchanged signature, now a one-line delegate.
  - `struct LogCapture`, `fn capture_logs<T>(body: impl FnOnce() -> T) -> (T, String)`, `fn runtime() -> tokio::runtime::Runtime`, and `fn unreadable_service() -> (DataService, TempDir)` in `web.rs`'s test module — **Task 3 and Task 12 add tests to this same module and reuse these.**

- [ ] **Step 1: Make `tower::ServiceExt` available under the `webapp` feature**

Check first — if `tower::util::ServiceExt` already resolves, skip this step. Otherwise, in `Cargo.toml`, change:

```toml
tower = { version = "0.5", optional = true }
```

to:

```toml
# `util` is what provides `ServiceExt::oneshot`, which the web tests use to
# drive the router in-process instead of binding a port.
tower = { version = "0.5", features = ["util"], optional = true }
```

- [ ] **Step 2: Widen the test-only parse counter so `web.rs`'s tests can read it**

In `src/data_svc.rs`, the accessor is currently private to the `data_svc` module:

```rust
    #[cfg(test)]
    fn parse_count(&self) -> usize {
        self.parse_count.load(Ordering::Relaxed)
    }
```

Change `fn` to `pub(crate) fn`:

```rust
    #[cfg(test)]
    pub(crate) fn parse_count(&self) -> usize {
        self.parse_count.load(Ordering::Relaxed)
    }
```

(Task 3 asserts on it from `src/web.rs`. It stays `#[cfg(test)]`, so nothing ships.)

- [ ] **Step 3: Add the injection seam to `get_day_data_impl`**

In `src/web.rs`, replace the `pub async fn get_day_data_impl(...)` definition's first lines. It currently starts:

```rust
pub async fn get_day_data_impl(date: Date, state: &AppState) -> Result<DayData, StatusCode> {
    let date_str = date
        .format(&DATE_FORMAT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Use DataService so concurrent requests share the 30-second in-memory cache
    let content = DataService::get()
        .read_day(&date)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
```

Split it into a thin public wrapper plus an injectable core. The wrapper keeps the exact signature it has today:

```rust
/// Build one day's [`DayData`] for the REST and GraphQL endpoints.
///
/// Delegates to [`get_day_data_impl_with`] against the process-wide
/// [`DataService`], which is what makes concurrent requests share its
/// 30-second cache. The service is a parameter there and not here so tests
/// can hand in a hermetic one instead of the global singleton.
pub async fn get_day_data_impl(date: Date, state: &AppState) -> Result<DayData, StatusCode> {
    get_day_data_impl_with(DataService::get(), date, state).await
}

pub(crate) async fn get_day_data_impl_with(
    svc: &DataService,
    date: Date,
    state: &AppState,
) -> Result<DayData, StatusCode> {
    let date_str = date
        .format(&DATE_FORMAT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let content = svc.read_day(&date).await.map_err(|e| {
        // `read_day` attaches the path and the failing syscall via
        // `.with_context`. Collapsing straight to a StatusCode threw all of
        // it away, and every entry point funnels through here — a permission
        // error, a broken symlink or a full disk reached the operator as a
        // content-free 500. Log once, here, where the detail still exists.
        tracing::error!(%date_str, error = %e, "failed to read day data");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
```

Leave the rest of the function body exactly as it is — it now belongs to `get_day_data_impl_with`. It continues to use `state.config.get_prefix()` / `get_suffix()`; Task 3 changes that.

- [ ] **Step 4: Drop the now-redundant re-log in `aggregate_week_days`**

In `src/web.rs`, `aggregate_week_days` currently has:

```rust
        match outcome {
            Ok((idx, Ok(day_data))) => results.push((idx, day_data)),
            Ok((_, Err(e))) => tracing::warn!("Failed to load day data: {}", e),
            Err(e) => tracing::warn!("Task panicked loading day data: {}", e),
        }
```

By the time that middle arm runs, `e` is already just a `StatusCode` — the real error was logged one call down. Replace it with a comment recording that, and keep the panic arm, which logs something nothing else does:

```rust
        match outcome {
            Ok((idx, Ok(day_data))) => results.push((idx, day_data)),
            // Dropped rather than logged: `e` here is only the StatusCode.
            // `get_day_data_impl_with` already logged the underlying error
            // with its date and its I/O detail, so re-logging the status adds
            // a line and no information.
            Ok((_, Err(_))) => {}
            Err(e) => tracing::warn!("Task panicked loading day data: {}", e),
        }
```

- [ ] **Step 5: Write the failing test**

Append to the end of `src/web.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_svc::ParseSettings;
    use std::io::Write as _;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;
    use time::macros::date;

    /// A `tracing` writer that keeps everything written to it in memory.
    #[derive(Clone, Default)]
    struct LogCapture(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for LogCapture {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("log buffer").extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogCapture {
        type Writer = Self;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// Run `body` with a capturing subscriber installed, returning its value
    /// and everything it logged.
    ///
    /// A plain `#[test]` with its own current-thread runtime rather than
    /// `#[tokio::test]`: `tracing::subscriber::with_default` sets a
    /// thread-local, and only a current-thread runtime keeps the async work
    /// on the thread that has it.
    fn capture_logs<T>(body: impl FnOnce() -> T) -> (T, String) {
        let capture = LogCapture::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(capture.clone())
            .with_ansi(false)
            .finish();
        let value = tracing::subscriber::with_default(subscriber, body);
        let logged = String::from_utf8(capture.0.lock().expect("log buffer").clone())
            .expect("log output is utf-8");
        (value, logged)
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime")
    }

    /// A service whose data directory sits *below a regular file*, so every
    /// stat under it fails with ENOTDIR — an I/O error that is emphatically
    /// not `NotFound`, which is the only kind `read_day` swallows.
    fn unreadable_service() -> (DataService, TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let blocker = dir.path().join("not-a-directory");
        std::fs::File::create(&blocker)
            .expect("blocker file")
            .write_all(b"x")
            .expect("blocker contents");
        let svc = DataService::new_with_dir(
            60,
            blocker.join("days"),
            ParseSettings::default(),
        );
        (svc, dir)
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_day_file_logs_its_cause_before_becoming_a_500() {
        let (svc, _dir) = unreadable_service();
        let state = AppState::default();
        let rt = runtime();

        let (result, logged) = capture_logs(|| {
            rt.block_on(get_day_data_impl_with(&svc, date!(2026 - 08 - 24), &state))
        });

        assert_eq!(
            result.unwrap_err(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "an unreadable day file must still surface as a 500"
        );
        assert!(
            logged.contains("failed to read day data"),
            "the failure must be logged, not silently collapsed: {logged}"
        );
        assert!(
            logged.contains("2026-08-24"),
            "the log must name the date that failed: {logged}"
        );
        assert!(
            logged.contains("could not stat"),
            "the log must carry read_day's own context, not just a status: {logged}"
        );
    }
}
```

- [ ] **Step 6: Run the test to verify it FAILS on the pre-fix code**

Temporarily `git stash push -u -m 'codehealth-b4-verify'` is **not** allowed here (the stash stack is shared across worktrees). Instead, confirm RED by reverting just the `map_err` closure to `|_| StatusCode::INTERNAL_SERVER_ERROR`, running the test, then restoring it:

Run: `cargo test --features webapp --lib web::tests::an_unreadable_day_file`
Expected with the old `map_err`: **FAIL** on `the failure must be logged, not silently collapsed`.
Expected with the new `map_err`: **PASS**.

- [ ] **Step 7: Run the gate**

Run: `just gate`
Expected: green. The new test is `#[cfg(unix)]` and lives in a `webapp`-gated module, so it runs in the default and `webapp`-only configs and is absent from the `tui`-only one.

- [ ] **Step 8: Strip and commit**

```bash
todo-parser bughunt.md --strip B4
git add -A
git commit -m 'fix(observability): log day-data read failures before collapsing to 500 [B4]'
```

---

### Task 3 (B5): Route web/GraphQL day data through the memoized parse

**Files:**
- Modify: `src/web.rs` (`get_day_data_impl_with` body)
- Test: `src/web.rs` `mod tests`

**Interfaces:**
- Consumes: `get_day_data_impl_with`, `capture_logs`, `runtime`, `unreadable_service` (Task 2); `DataService::parse_day(&self, date: &Date) -> Result<Option<TimeTrackingData>>`; `DataService::parse_count(&self) -> usize` (test-only, `pub(crate)` as of Task 2).
- Produces: `pub(crate) async fn get_day_data_impl_with(svc: &DataService, date: Date) -> Result<DayData, StatusCode>` — the `state` parameter Task 2 gave it is **dropped here**, because after this change nothing in the body reads it and clippy runs with `-D warnings`. It is `pub(crate)`, so this is not an API change; `pub async fn get_day_data_impl(date, state)` keeps its signature and simply ignores `state`. **Must not regress Task 2's `tracing::error!` on the read failure path.**

**Behavior note to preserve:** today a *missing* file yields an empty `DayData` and a *present but empty* file is parsed. `parse_day` returns `None` only when the file is absent and `Some(parsed)` otherwise, so the mapping is one-to-one.

**Invariant this change resolves:** the endpoint used to parse with `state.config`'s prefix/suffix; it now parses with the `DataService`'s. In production those are the same values (both derive from `Config::get()`; `run_server` is constructed from a `Config::get()` clone at `cli/src/main.rs:45`). Step 2's second test pins *which one wins* so a future divergence fails loudly instead of silently changing output.

- [ ] **Step 1: Rewrite the body to use `parse_day`**

In `src/web.rs`, `get_day_data_impl_with` currently reads (post-Task-2):

```rust
    let content = svc.read_day(&date).await.map_err(|e| {
        tracing::error!(%date_str, error = %e, "failed to read day data");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let Some(content) = content else {
        return Ok(DayData {
            date: date_str,
            total_hours: 0.0,
            dead_time_hours: 0.0,
            projects: vec![],
            warnings: vec![],
            start_time: None,
            end_time: None,
        });
    };

    let data = time_tracking_parser::parse_time_tracking_data(
        &content,
        state.config.get_prefix(),
        state.config.get_suffix(),
    );
```

Replace all of that with:

```rust
    // `parse_day`, not `read_day` + a fresh parse: the service memoizes the
    // parse alongside the raw content, and going around it meant every REST
    // and GraphQL call reparsed from scratch — fanned out ×7 per week
    // request, and re-run on both queries per 500ms editor autosave.
    let data = svc.parse_day(&date).await.map_err(|e| {
        tracing::error!(%date_str, error = %e, "failed to read day data");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // `None` is "no file on disk", the same case the old `read_day` -> `None`
    // arm answered with an empty day.
    let Some(data) = data else {
        return Ok(DayData::empty(date));
    };
```

Nothing in the body reads `state` any more, and clippy runs with `-D warnings`, so **drop the parameter** rather than underscore-prefixing it. Change the signature and the wrapper:

```rust
pub async fn get_day_data_impl(date: Date, _state: &AppState) -> Result<DayData, StatusCode> {
    // `_state`: the endpoint's parse markers now come from the DataService,
    // which resolves them from the same `Config::get()` this state was cloned
    // from. The parameter stays because this is a public signature other
    // crates may name.
    get_day_data_impl_with(DataService::get(), date).await
}

pub(crate) async fn get_day_data_impl_with(
    svc: &DataService,
    date: Date,
) -> Result<DayData, StatusCode> {
```

`aggregate_week_days` keeps calling `get_day_data_impl(day_date, &state)` unchanged.

Then update Task 2's two existing tests, which currently pass `&state`: drop the `let state = AppState::default();` line and the `&state` argument from `an_unreadable_day_file_logs_its_cause_before_becoming_a_500`.

- [ ] **Step 2: Write the failing tests**

Add to `src/web.rs`'s `mod tests`:

```rust
    /// A hermetic service whose parse markers differ from `Config::default()`'s
    /// (which has none), so a test can tell which of the two the endpoint
    /// honoured.
    fn service_with_markers(prefix: &str, suffix: &str) -> (DataService, TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let svc = DataService::new_with_dir(
            60,
            dir.path().to_path_buf(),
            ParseSettings {
                prefix: Some(prefix.to_owned()),
                suffix: Some(suffix.to_owned()),
                template_file: None,
            },
        );
        (svc, dir)
    }

    #[test]
    fn repeated_day_requests_reuse_the_memoized_parse() {
        let dir = tempfile::tempdir().expect("temp dir");
        let svc = DataService::new_with_dir(60, dir.path().to_path_buf(), ParseSettings::default());
        let day = date!(2026 - 08 - 24);
        let rt = runtime();

        rt.block_on(async {
            let path = svc.get_file_path(day).await.unwrap();
            tokio::fs::write(&path, "8-10 admin\n  - note\n")
                .await
                .unwrap();

            for _ in 0..5 {
                get_day_data_impl_with(&svc, day).await.expect("day data");
            }
        });

        assert_eq!(
            svc.parse_count(),
            1,
            "five requests for an unchanged day must run the parser once, \
             not once per request"
        );
    }

    #[test]
    fn day_data_is_parsed_with_the_services_markers() {
        // The endpoint used to parse with `state.config`'s markers; it now
        // parses with the service's. Production keeps the two in step —
        // `run_server` is handed a clone of the same `Config::get()` the
        // process-wide `DataService` reads — so this pins which one actually
        // governs the parse, and a future divergence fails here instead of
        // silently changing endpoint output.
        let (svc, _dir) = service_with_markers("```timetracking", "```");
        let day = date!(2026 - 08 - 24);
        let rt = runtime();

        let data = rt.block_on(async {
            let path = svc.get_file_path(day).await.unwrap();
            tokio::fs::write(
                &path,
                "8-9 outside-the-fence\n```timetracking\n9-11 admin\n```\n",
            )
            .await
            .unwrap();
            get_day_data_impl_with(&svc, day).await.expect("day data")
        });

        let names: Vec<&str> = data.projects.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"admin"),
            "the fenced entry must be parsed: {names:?}"
        );
        assert!(
            !names.contains(&"outside-the-fence"),
            "the service's markers must bound the parse: {names:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_parse_failure_still_logs_its_cause_before_becoming_a_500() {
        // Guards the read-failure logging across this rewrite of the same
        // `map_err`.
        let (svc, _dir) = unreadable_service();
        let rt = runtime();

        let (result, logged) =
            capture_logs(|| rt.block_on(get_day_data_impl_with(&svc, date!(2026 - 08 - 24))));

        assert_eq!(result.unwrap_err(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            logged.contains("failed to read day data") && logged.contains("2026-08-24"),
            "the read-failure log must survive the parse_day rewrite: {logged}"
        );
    }
```

- [ ] **Step 3: Run the tests to verify the first two FAIL before the Step-1 edit**

Run: `cargo test --features webapp --lib web::tests::`
Expected before Step 1 (write the tests against the Task-2 three-argument signature to see this, then drop the argument alongside Step 1): `repeated_day_requests_reuse_the_memoized_parse` FAILS — `parse_count` is 0, because the endpoint never went through `parse_day` at all — and `day_data_is_parsed_with_the_services_markers` FAILS, because `outside-the-fence` is present when `state.config` supplies the (absent) markers.
Expected after Step 1: all PASS.

- [ ] **Step 4: Run the gate**

Run: `just gate`
Expected: green.

- [ ] **Step 5: Strip and commit**

```bash
todo-parser bughunt.md --strip B5
git add -A
git commit -m 'fix(caching): parse day data through the memoized DataService cache [B5]'
```

---

### Task 4 (B6): Stop `DateEditor` auto-resaving freshly-loaded content

**Files:**
- Modify: `site/src/components/DateEditor.tsx:19-26`
- Modify: `site/src/components/__tests__/DateEditor.deps.test.tsx` (authorized exception — it currently works around this bug)
- Test: `site/src/components/__tests__/DateEditor.mount.test.tsx` (create)

**Interfaces:**
- Consumes: `useDateData(date: Date) => { content: string | null, parsedData: …, updater: (s: string) => void }` (current shape; Task 6 adds fields).
- Produces: nothing later tasks rely on.

- [ ] **Step 1: Write the failing test**

Create `site/src/components/__tests__/DateEditor.mount.test.tsx`:

```tsx
import { fireEvent, render, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { DateEditor } from '../DateEditor';
import * as useDateDataModule from 'hooks/useDateData';

describe('DateEditor mount', () => {
  it('does not save the content it just loaded', async () => {
    // Opening a day used to re-write its file ~500ms later with the exact
    // bytes just read, because the init effect deliberately left
    // lastSentData null. An external edit landing in that window was
    // clobbered by the stale in-browser copy.
    const updater = vi.fn();
    const spy = vi.spyOn(useDateDataModule, 'useDateData');
    spy.mockReturnValue({ content: 'loaded from server', parsedData: null, updater });

    render(<DateEditor date={new Date('2026-08-29T00:00:00')} />);

    await new Promise((r) => setTimeout(r, 700));
    expect(updater).not.toHaveBeenCalled();
  });

  it('still saves once the user actually edits', async () => {
    const updater = vi.fn();
    const spy = vi.spyOn(useDateDataModule, 'useDateData');
    spy.mockReturnValue({ content: 'loaded from server', parsedData: null, updater });

    const { getByRole } = render(<DateEditor date={new Date('2026-08-29T00:00:00')} />);
    await new Promise((r) => setTimeout(r, 700));

    const textarea = getByRole('textbox') as HTMLTextAreaElement;
    fireEvent.change(textarea, { target: { value: 'edited by the user' } });

    await waitFor(() => expect(updater).toHaveBeenCalledWith('edited by the user'), {
      timeout: 2000,
    });
  });
});
```

- [ ] **Step 2: Run the tests to verify the first FAILS**

Run: `cd site && yarn test --run src/components/__tests__/DateEditor.mount.test.tsx`
Expected: `does not save the content it just loaded` FAILS — `updater` was called once with `'loaded from server'`. `still saves once the user actually edits` PASSES (it must pass before and after; it is the guard that the fix does not simply disable saving).

- [ ] **Step 3: Fix the init effect**

In `site/src/components/DateEditor.tsx`, replace:

```tsx
  // Initialize local data when content first loads
  useEffect(() => {
    if (content !== null && content !== undefined && !hasInitialized) {
      setLocalData(content);
      setHasInitialized(true);
      // Don't set lastSentData here - let it remain null so first user change will be detected
    }
  }, [content, hasInitialized]);
```

with:

```tsx
  // Initialize local data when content first loads
  useEffect(() => {
    if (content !== null && content !== undefined && !hasInitialized) {
      setLocalData(content);
      setHasInitialized(true);
      // Seed the baseline with what we just loaded. Leaving it null made the
      // debounce effect fire 500ms after every mount and re-write the file
      // with the bytes it had just read — a save the user never asked for,
      // and one that clobbered any external edit landing in that window.
      lastSentData.current = content;
    }
  }, [content, hasInitialized]);
```

- [ ] **Step 4: Run the tests to verify they PASS**

Run: `cd site && yarn test --run src/components/__tests__/DateEditor.mount.test.tsx`
Expected: 2 passed.

- [ ] **Step 5: Remove the workaround from the existing deps test**

`site/src/components/__tests__/DateEditor.deps.test.tsx` waits for the mount save that no longer happens, so it now hangs and fails. Replace lines 15-20:

```tsx
    // Mount settles into an initial debounced save regardless of this fix
    // (a separate, pre-existing quirk unrelated to the content dep — see
    // task-8-9-report.md). Let it finish before exercising the case this
    // test actually targets.
    await waitFor(() => expect(updater).toHaveBeenCalledWith('a'));
    updater.mockClear();
```

with:

```tsx
    // Mount used to settle into an initial debounced save of the content it
    // had just loaded; that quirk is fixed, so the settle window must now
    // pass with no save at all.
    await new Promise((r) => setTimeout(r, 600));
    expect(updater).not.toHaveBeenCalled();
```

Then drop the now-unused `waitFor` from the import on line 1:

```tsx
import { render } from '@testing-library/react';
```

- [ ] **Step 6: Run the frontend gate**

Run: `cd site && yarn test --run && yarn lint && npx tsc --noEmit`
Expected: green — the two new tests included, lint clean, no type errors.

- [ ] **Step 7: Strip and commit**

```bash
todo-parser bughunt.md --strip B6
git add -A
git commit -m 'fix(frontend): stop DateEditor re-saving the content it just loaded [B6]'
```

---

### Task 5 (B7): Refetch by operation name so every mounted date updates

**Files:**
- Modify: `site/src/hooks/useDateData.ts:26-32`
- Test: `site/src/hooks/__tests__/useDateData.test.tsx` (create)
- Test: `site/src/hooks/__tests__/queries.test.ts` (create)

**Interfaces:**
- Consumes: `FILE_CONTENT_FOR_DATE_QUERY`, `GET_DAY_DATA_FOR_DATE_QUERY`, `GET_WEEK_DATA_FOR_DATE_QUERY`, `UPDATE_FILE_CONTENT_FOR_DATE_MUTATION` from `site/src/hooks/queries.ts`.
- Produces: `site/src/hooks/__tests__/useDateData.test.tsx` with its `@apollo/client` module mock — **Task 6 extends this same file.**

The operation names are `FileContentForDate`, `DayDataForDate`, and `WeekDataForDate` (verified against `site/src/hooks/queries.ts:4,22,50`). The second test file pins them so a rename of a `gql` document breaks the build instead of silently refetching nothing.

- [ ] **Step 1: Write the operation-name guard test**

Create `site/src/hooks/__tests__/queries.test.ts`:

```ts
import type { DocumentNode, OperationDefinitionNode } from 'graphql';
import { describe, expect, it } from 'vitest';
import {
  FILE_CONTENT_FOR_DATE_QUERY,
  GET_DAY_DATA_FOR_DATE_QUERY,
  GET_WEEK_DATA_FOR_DATE_QUERY,
} from '../queries';

const operationName = (doc: DocumentNode) =>
  (doc.definitions[0] as OperationDefinitionNode).name?.value;

describe('query operation names', () => {
  // useDateData's refetchQueries names these as strings. A rename here with
  // no matching rename there makes Apollo refetch nothing, silently.
  it('match the strings useDateData refetches by', () => {
    expect(operationName(FILE_CONTENT_FOR_DATE_QUERY)).toBe('FileContentForDate');
    expect(operationName(GET_DAY_DATA_FOR_DATE_QUERY)).toBe('DayDataForDate');
    expect(operationName(GET_WEEK_DATA_FOR_DATE_QUERY)).toBe('WeekDataForDate');
  });
});
```

- [ ] **Step 2: Write the failing refetch test**

Create `site/src/hooks/__tests__/useDateData.test.tsx`:

```tsx
import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { mutate, useQueryMock } = vi.hoisted(() => ({
  mutate: vi.fn(),
  useQueryMock: vi.fn(),
}));

vi.mock('@apollo/client', () => ({
  gql: (strings: TemplateStringsArray, ...values: unknown[]) =>
    String.raw({ raw: strings }, ...values),
  useQuery: (...args: unknown[]) => useQueryMock(...args),
  useMutation: () => [mutate],
}));

import { useDateData } from '../useDateData';

describe('useDateData.updater', () => {
  beforeEach(() => {
    mutate.mockReset();
    mutate.mockResolvedValue({});
    useQueryMock.mockReset();
    useQueryMock.mockReturnValue({ data: undefined, error: undefined });
  });

  it('refetches by operation name, not pinned to the edited date', () => {
    // Pinning {date: '2026-08-27'} left the weekly-summary page's own cache
    // entry — mounted with the week-start date — untouched, so navigating
    // back served pre-edit numbers until a manual reload.
    const { result } = renderHook(() => useDateData(new Date('2026-08-27T00:00:00')));

    result.current.updater('new content');

    expect(mutate).toHaveBeenCalledTimes(1);
    expect(mutate.mock.calls[0][0]).toMatchObject({
      variables: { date: '2026-08-27', content: 'new content' },
      refetchQueries: ['FileContentForDate', 'DayDataForDate', 'WeekDataForDate'],
    });
  });
});
```

- [ ] **Step 3: Run the tests to verify the refetch one FAILS**

Run: `cd site && yarn test --run src/hooks/__tests__`
Expected: `queries.test.ts` PASSES (it describes today's names). `useDateData.test.tsx` FAILS — `refetchQueries` is an array of `{query, variables}` objects, not names.

- [ ] **Step 4: Switch to operation names**

In `site/src/hooks/useDateData.ts`, replace:

```ts
      refetchQueries: [
        // Refetch daily queries for the current date
        { query: FILE_CONTENT_FOR_DATE_QUERY, variables: { date: dateString } },
        { query: GET_DAY_DATA_FOR_DATE_QUERY, variables: { date: dateString } },
        // Refetch weekly query for the week containing this date
        { query: GET_WEEK_DATA_FOR_DATE_QUERY, variables: { date: dateString } },
      ],
```

with:

```ts
      // Operation names, not {query, variables} pairs. Pinning the variables
      // refetched only the cache entry for the edited date; a weekly-summary
      // page mounted with its week-start date is a different entry, and with
      // Apollo's default cache-first policy it kept serving pre-edit numbers
      // until a manual reload. Names refetch every currently-active instance
      // of each query, whatever date it was mounted with.
      // Pinned by src/hooks/__tests__/queries.test.ts.
      refetchQueries: ['FileContentForDate', 'DayDataForDate', 'WeekDataForDate'],
```

Then remove the three now-unused query imports from the top of the file, keeping the two still in use:

```ts
import {
  FILE_CONTENT_FOR_DATE_QUERY,
  GET_DAY_DATA_FOR_DATE_QUERY,
  UPDATE_FILE_CONTENT_FOR_DATE_MUTATION,
} from './queries';
```

(`GET_WEEK_DATA_FOR_DATE_QUERY` is no longer referenced by this file. `FILE_CONTENT_FOR_DATE_QUERY` and `GET_DAY_DATA_FOR_DATE_QUERY` are still used by the two `useQuery` calls.)

- [ ] **Step 5: Run the tests to verify they PASS**

Run: `cd site && yarn test --run src/hooks/__tests__`
Expected: all PASS.

- [ ] **Step 6: Run the frontend gate**

Run: `cd site && yarn test --run && yarn lint && npx tsc --noEmit`
Expected: green. `yarn lint` runs with `--max-warnings 0`, so an unused import fails it — that is the check on Step 4's import cleanup.

- [ ] **Step 7: Strip and commit**

```bash
todo-parser bughunt.md --strip B7
git add -A
git commit -m 'fix(frontend): refetch by operation name so every mounted date updates [B7]'
```

- [ ] **Step 8: MILESTONE — full suite**

Run: `just gate` and `cd site && yarn test --run && yarn lint && npx tsc --noEmit`
Expected: both green. On red, bisect within B3-B7, revert the offender, and surface the diagnosis before continuing.

---

### Task 6 (B8): Surface Apollo query errors instead of discarding them

**Files:**
- Modify: `site/src/hooks/useDateData.ts`
- Modify: `site/src/hooks/useWeekData.ts`
- Modify: `site/src/components/DateEditor.tsx`
- Modify: `site/src/components/WeeklySummary.tsx:8-26`
- Modify: `site/src/page/WeeklySummaryPage.tsx:10,16`
- Modify: `site/src/page/__tests__/DateEditorPage.test.tsx:6-8` (mock must match the new hook shape)
- Modify: `site/src/components/__tests__/DateEditor.deps.test.tsx` and `DateEditor.mount.test.tsx` (mocks must match the new hook shape)
- Test: `site/src/hooks/__tests__/useDateData.test.tsx` (extend), `site/src/components/__tests__/DateEditor.error.test.tsx` (create)

**Interfaces:**
- Consumes: the `@apollo/client` module mock and `useQueryMock` from Task 5's `useDateData.test.tsx`.
- Produces:
  - `useDateData(date) => { content, parsedData, updater, error: Error | undefined }` — widened to `Error` (not `ApolloError`) so test mocks can pass a plain `new Error(...)` without a cast.
  - `useWeekData(date) => [data, error]` — positional, so the existing `const [data] = useWeekData(date)` destructuring keeps working.
  - `WeeklySummary` gains an optional `error?: unknown` prop.

**Type-safety note:** adding a *required* `error` field to `useDateData`'s return type breaks every `vi.spyOn(...).mockReturnValue({...})` that omits it. `npx tsc --noEmit` is what catches this; `yarn test` and `yarn lint` do not. Update all three mock sites in Step 4.

- [ ] **Step 1: Write the failing tests**

Append to `site/src/hooks/__tests__/useDateData.test.tsx`, inside a new `describe`:

```tsx
describe('useDateData error surfacing', () => {
  beforeEach(() => {
    mutate.mockReset();
    mutate.mockResolvedValue({});
    useQueryMock.mockReset();
  });

  it('returns the query error rather than swallowing it', () => {
    // Discarding `error` left `content` permanently undefined with no log
    // and no toast; DateEditor's init effect then never fired, so its
    // debounced save never fired either and a whole session of typing was
    // silently dropped.
    const boom = new Error('network down');
    useQueryMock.mockReturnValue({ data: undefined, error: boom });

    const { result } = renderHook(() => useDateData(new Date('2026-08-27T00:00:00')));

    expect(result.current.error).toBe(boom);
    expect(result.current.content).toBeNull();
  });
});
```

Create `site/src/components/__tests__/DateEditor.error.test.tsx`:

```tsx
import { render } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom';
import { DateEditor } from '../DateEditor';
import * as useDateDataModule from 'hooks/useDateData';

describe('DateEditor load failure', () => {
  it('tells the user and refuses input instead of silently dropping it', () => {
    const spy = vi.spyOn(useDateDataModule, 'useDateData');
    spy.mockReturnValue({
      content: null,
      parsedData: null,
      updater: vi.fn(),
      error: new Error('network down'),
    });

    const { getByRole, getByText } = render(<DateEditor date={new Date('2026-08-29T00:00:00')} />);

    expect(getByText(/could not load/i)).toBeInTheDocument();
    expect(getByRole('textbox')).toBeDisabled();
  });
});
```

(The `@testing-library/jest-dom` import is what provides `toBeInTheDocument`/`toBeDisabled`, matching `site/src/page/__tests__/DateEditorPage.test.tsx:4`.)

- [ ] **Step 2: Run the tests to verify they FAIL**

Run: `cd site && yarn test --run src/hooks/__tests__/useDateData.test.tsx src/components/__tests__/DateEditor.error.test.tsx`
Expected: FAIL — `result.current.error` is `undefined` (the hook never returns it), and `DateEditor` renders neither the message nor a disabled textarea.

- [ ] **Step 3: Surface the errors from both hooks**

In `site/src/hooks/useDateData.ts`, replace the two `useQuery` destructures and the return object:

```ts
  const { data, error: contentError } = useQuery(FILE_CONTENT_FOR_DATE_QUERY, {
    variables: { date: dateString },
    skip: !dateString,
  });
  const { data: parsedData, error: parsedError } = useQuery(GET_DAY_DATA_FOR_DATE_QUERY, {
    variables: { date: dateString },
    skip: !dateString,
  });
```

and:

```ts
  // Widened to `Error` rather than left as `ApolloError` so a test can hand
  // the hook's consumers a plain `new Error(...)` without a cast.
  const error: Error | undefined = contentError ?? parsedError;

  return {
    // `||`, not `??`, deliberately left as it was. Switching would change how
    // an empty day file behaves, which is a separate question from error
    // surfacing and not this batch's to answer.
    content: data?.fileContentForDate || null,
    parsedData: parsedData?.dataForDate || null,
    updater,
    // Surfaced, not swallowed. The mutation path below has logged and toasted
    // its failures since it was written; the read path did neither, so a
    // failed load looked identical to an empty day and quietly disabled
    // saving for the rest of the session.
    error,
  };
```

In `site/src/hooks/useWeekData.ts`:

```ts
export const useWeekData = (date: Date) => {
  const { data, error } = useQuery(GET_WEEK_DATA_FOR_DATE_QUERY, {
    variables: { date: toDateString(date) },
    skip: !date,
  });

  // Positional so existing `const [data] = useWeekData(date)` call sites keep
  // working unchanged.
  return [data, error] as const;
};
```

- [ ] **Step 4: Update the three hook mocks so they still typecheck**

`site/src/page/__tests__/DateEditorPage.test.tsx:6-8`:

```tsx
vi.mock('hooks/useDateData', () => ({
  useDateData: () => ({ content: '', parsedData: null, updater: vi.fn(), error: undefined }),
}));
```

`site/src/components/__tests__/DateEditor.deps.test.tsx` — both `spy.mockReturnValue` calls:

```tsx
    spy.mockReturnValue({ content: 'a', parsedData: null, updater, error: undefined });
```

`site/src/components/__tests__/DateEditor.mount.test.tsx` — both `spy.mockReturnValue` calls:

```tsx
    spy.mockReturnValue({
      content: 'loaded from server',
      parsedData: null,
      updater,
      error: undefined,
    });
```

- [ ] **Step 5: Render the error in `DateEditor`**

In `site/src/components/DateEditor.tsx`, change the destructure on line 12:

```tsx
  const { content, updater, parsedData, error } = useDateData(date);
```

and replace the returned JSX's textarea half:

```tsx
  return (
    <div className="w-full p-4 rounded shadow flex flex-col">
      {error && (
        <div
          role="alert"
          className="mb-4 p-3 rounded bg-red-900 text-white border border-red-500"
        >
          Could not load this day&apos;s file. Editing is disabled so your typing
          isn&apos;t silently discarded — reload once the server is reachable.
        </div>
      )}
      <div className="w-full flex">
        <textarea
          value={localData}
          disabled={Boolean(error)}
          className="w-1/2 h-full p-2 border rounded mr-4 bg-gray-900 text-white disabled:opacity-50"
          onChange={(e) => setLocalData(e.target.value)}
        />
        <div className="w-1/2 p-4 border-l overflow-y-auto">
          <DateSummary
            parsedData={
              parsedData || {
                date: 'N/A',
                totalHours: 0,
                deadTimeHours: 0,
                startTime: null,
                endTime: null,
                projects: [],
                warnings: [],
              }
            }
          />
        </div>
      </div>
    </div>
  );
```

- [ ] **Step 6: Distinguish a failed week query from an empty one**

In `site/src/components/WeeklySummary.tsx`, widen the props and the fallback:

```tsx
interface WeeklySummaryProps {
  data: { weekDataForDate?: WeekData } | null;
  error?: unknown;
}

const WeeklySummary = ({ data, error }: WeeklySummaryProps) => {
```

and replace the `if (!weekData)` block:

```tsx
  if (!weekData) {
    return (
      <div className="p-4 bg-gray-900 text-white rounded">
        {/* A failed query used to render this same "No data available",
            indistinguishable from a genuinely empty week. */}
        <p>{error ? 'Could not load this week. Please try again.' : 'No data available'}</p>
      </div>
    );
  }
```

In `site/src/page/WeeklySummaryPage.tsx`, thread it through:

```tsx
  const [data, error] = useWeekData(date);
```

```tsx
      <WeeklySummary data={data} error={error} />
```

- [ ] **Step 7: Run the tests to verify they PASS**

Run: `cd site && yarn test --run`
Expected: all green, with the three new tests included.

- [ ] **Step 8: Run the frontend gate**

Run: `cd site && yarn test --run && yarn lint && npx tsc --noEmit`
Expected: green. If `tsc` flags `data` in `WeeklySummaryPage` (the `as const` tuple narrows the type), give `useWeekData`'s `data` an explicit type or drop `as const` — do **not** reach for `any`.

- [ ] **Step 9: Strip and commit**

```bash
todo-parser bughunt.md --strip B8
git add -A
git commit -m 'fix(frontend): surface Apollo query errors instead of discarding them [B8]'
```

---

### Task 7 (B9): Stop writing to stdout/stderr while the TUI owns the screen

**Files:**
- Modify: `cli/src/main.rs:107`, `cli/src/main.rs:121-123`
- Test: `cli/tests/cli_runtime_smoke.rs` (add one test)

**Interfaces:**
- Consumes: `common::ttcli()` from `cli/tests/common/mod.rs`.
- Produces: nothing later tasks rely on.

**Leave `cli/src/main.rs:58`'s `eprintln!` alone.** It runs only after `tui()` returns, and `tui()` always calls `ratatui::restore()` first, so the terminal is back in cooked mode by then.

- [ ] **Step 1: Write the failing test**

Append to `cli/tests/cli_runtime_smoke.rs`:

```rust
/// The startup banner used to print to stdout unconditionally whenever the
/// webserver task was spawned — including under `--serve --tui`, where it
/// raced `ratatui::init()`'s alternate-screen entry and could leave text
/// stranded in a frame ratatui's diff renderer does not know changed.
#[cfg(feature = "webapp")]
#[test]
fn serving_prints_no_banner_to_stdout() {
    use std::io::Read;
    use std::process::Stdio;
    use std::thread;
    use std::time::Duration;

    let data_dir = tempfile::tempdir().expect("failed to create temp dir");

    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind to ephemeral port");
    let port = listener
        .local_addr()
        .expect("failed to get local addr")
        .port();
    drop(listener);

    let mut child = common::ttcli()
        .args([
            "--serve",
            "--port",
            &port.to_string(),
            "--noedit",
            "--data-directory",
        ])
        .arg(data_dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn ttcli");

    thread::sleep(Duration::from_millis(500));
    child.kill().expect("failed to kill ttcli");
    child.wait().expect("failed to reap ttcli");

    let mut stdout = String::new();
    child
        .stdout
        .take()
        .expect("piped stdout")
        .read_to_string(&mut stdout)
        .expect("failed to read ttcli stdout");

    assert!(
        !stdout.contains("Other jobs are running"),
        "the background-task banner must not reach stdout — it races the \
         TUI's alternate screen under `--serve --tui`. Got:\n{stdout}"
    );
}
```

- [ ] **Step 2: Run the test to verify it FAILS**

Run: `cargo test -p cli --test cli_runtime_smoke serving_prints_no_banner`
Expected: FAIL — stdout contains `Other jobs are running (webserver or tui), press ctrl-c to quit (webserver)`.

- [ ] **Step 3: Delete the two writes**

In `cli/src/main.rs`, `spawn_webserver_if_configured` currently has:

```rust
        set.spawn(async move {
            if let Err(e) = time_tracking_cli::web::run_server(port, config, rx).await {
                error!("Error running web server: {}", e);
                eprintln!("Error running web server: {}", e);
            }
        });
```

Drop the `eprintln!` and record why:

```rust
        set.spawn(async move {
            if let Err(e) = time_tracking_cli::web::run_server(port, config, rx).await {
                // Logged only. `--serve` and `--tui` are independent flags
                // with no `conflicts_with`, so this task can be running while
                // the TUI owns the alternate screen and raw mode — and
                // ratatui's diff renderer does not know a region something
                // else wrote to has changed. The `tracing::error!` above
                // already carries the whole message to the log file.
                error!("Error running web server: {}", e);
            }
        });
```

And `wait_for_background_tasks`:

```rust
    if webserver_running {
        println!("Other jobs are running (webserver or tui), press ctrl-c to quit (webserver)");
    }
```

becomes:

```rust
    if webserver_running {
        // `info!`, not `println!`: this fires on every combined
        // `--serve --tui` launch, where writing to stdout races the TUI's
        // own `ratatui::init()` alternate-screen entry. Feedback that must
        // reach a user while the TUI is up belongs on its status line, the
        // way `LoadFailed` and the clipboard failures already do.
        tracing::info!("Background tasks are running; press ctrl-c to quit");
    }
```

Leave the file-level `use tracing::error;` at `cli/src/main.rs:8` exactly as it is. Write `tracing::info!` fully qualified rather than adding `info` to that import: `spawn_webserver_if_configured` and the TUI branch each already have their own `#[cfg]`-local `use tracing::info;`, and a file-level one would shadow-conflict with them under some feature combinations.

- [ ] **Step 4: Run the test to verify it PASSES**

Run: `cargo test -p cli --test cli_runtime_smoke serving_prints_no_banner`
Expected: PASS.

- [ ] **Step 5: Run the gate**

Run: `just gate`
Expected: green. Watch for `unused_imports` on `tracing::error` in the `tui`-only build — the `eprintln!` removal does not touch that, but confirm clippy `-D warnings` is clean in all three configs.

- [ ] **Step 6: Strip and commit**

```bash
todo-parser bughunt.md --strip B9
git add -A
git commit -m 'fix(observability): keep background-task messages off the TUI screen [B9]'
```

---

### Task 8 (B10): Spawn a multi-word `$EDITOR` correctly

**Files:**
- Modify: `src/editor.rs:21-43`
- Test: `src/editor.rs` (`mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `fn split_editor_command(editor: &str) -> Option<(String, Vec<String>)>` (private) — program plus leading args, `None` for an all-whitespace value.
  - `fn open_in_editor_with(editor: &str, file_path: &Path) -> Result<()>` (private) — the testable core.
  - `pub fn open_in_editor(file_path: &PathBuf) -> Result<()>` — **signature unchanged**, now a delegate.

**Splitting strategy:** `str::split_whitespace`, not `shlex`. `shlex` handles quoted paths inside `$EDITOR` correctly but is a new third-party dependency for a case (`EDITOR='"/opt/my editor/bin" --wait'`) nobody in this project has hit, and the crate-decisions file favours a minimal dependency set. Whitespace splitting fixes every value the finding names (`code --wait`, `emacsclient -c`, `subl -n -w`). Record the trade-off in the doc comment so a future quoted-path bug has a pointer.

- [ ] **Step 1: Extract the seam without changing behavior**

In `src/editor.rs`, replace `open_in_editor` with:

```rust
/// Open `file_path` in `$EDITOR` — or `$VISUAL`, or a platform default —
/// inheriting stdio so a terminal editor can take over the screen.
///
/// Blocks until the editor exits, and errors if it exits non-zero.
pub fn open_in_editor(file_path: &PathBuf) -> Result<()> {
    open_in_editor_with(&get_editor(), file_path)
}

/// [`open_in_editor`] with the editor supplied rather than read from the
/// environment, so tests can exercise it without mutating process-wide state.
fn open_in_editor_with(editor: &str, file_path: &Path) -> Result<()> {
    let mut command = Command::new(editor);
    command.arg(file_path);

    // For some editors like vim/nano, we need to inherit stdio
    command.stdin(Stdio::inherit());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let status = command.status().context("error running command")?;

    if !status.success() {
        bail!("Editor '{}' exited with non-zero status", editor);
    }

    Ok(())
}
```

Add `Path` to the imports on line 3:

```rust
use std::path::{Path, PathBuf};
```

- [ ] **Step 2: Run the gate to confirm the extraction changed nothing**

Run: `just gate`
Expected: green, with the lib test count unchanged. This step is a pure refactor; if anything moved, stop and fix it before the behavior change.

- [ ] **Step 3: Write the failing tests**

Add to `src/editor.rs`'s `mod tests`:

```rust
    #[test]
    fn a_single_word_editor_splits_to_itself_with_no_args() {
        assert_eq!(
            split_editor_command("nano"),
            Some(("nano".to_owned(), vec![]))
        );
    }

    #[test]
    fn a_multi_word_editor_splits_into_program_and_args() {
        assert_eq!(
            split_editor_command("code --wait"),
            Some(("code".to_owned(), vec!["--wait".to_owned()]))
        );
        assert_eq!(
            split_editor_command("  subl   -n  -w  "),
            Some((
                "subl".to_owned(),
                vec!["-n".to_owned(), "-w".to_owned()]
            ))
        );
    }

    #[test]
    fn an_empty_editor_value_splits_to_nothing() {
        assert_eq!(split_editor_command("   "), None);
    }

    /// `EDITOR="code --wait"`, `"emacsclient -c"` and `"subl -n -w"` are all
    /// ordinary configurations. `Command::new` does no word splitting, so the
    /// OS looked for a binary literally named `code --wait` and `spawn`
    /// failed with `NotFound` — aborting the whole run on the CLI path, and
    /// silently disabling the `e` key for the session in the TUI.
    #[cfg(unix)]
    #[test]
    fn a_multi_word_editor_actually_runs() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();

        // `env true <file>` runs `true` with the file as an argument and
        // exits 0 — a real two-word command that exists everywhere.
        open_in_editor_with("env true", &test_file).expect("a multi-word editor must spawn");
    }

    #[cfg(unix)]
    #[test]
    fn a_multi_word_editor_exiting_non_zero_still_errors() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();

        let err = open_in_editor_with("env false", &test_file)
            .expect_err("a non-zero editor exit must still be an error");
        assert!(
            err.to_string().contains("non-zero status"),
            "the error must name the failure mode: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_editor_that_does_not_exist_errors_rather_than_panicking() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();

        assert!(
            open_in_editor_with("definitely-not-a-real-editor-binary", &test_file).is_err(),
            "a missing editor binary must be an error, not a panic"
        );
    }
```

- [ ] **Step 4: Run the tests to verify they FAIL**

Run: `cargo test --lib editor::tests`
Expected: the three `split_editor_command` tests FAIL to compile (`cannot find function`), and — once you stub `split_editor_command` to `Some((editor.to_owned(), vec![]))` to get a compile — `a_multi_word_editor_actually_runs` FAILS with `error running command: No such file or directory`. Confirm that specific failure before writing the real implementation; it is the bug reproducing.

- [ ] **Step 5: Implement the split**

Add above `open_in_editor` in `src/editor.rs`:

```rust
/// Split a configured editor value into the program to spawn and the
/// arguments that precede the file path.
///
/// `Command::new` performs no word splitting, so a perfectly ordinary
/// `EDITOR="code --wait"` made the OS look for a binary literally named
/// `code --wait`.
///
/// Whitespace splitting, deliberately, rather than a shell-words crate: it
/// covers every documented multi-word editor configuration (`code --wait`,
/// `emacsclient -c`, `subl -n -w`) with no new dependency. It does *not*
/// handle a quoted path containing spaces (`EDITOR='"/opt/my editor" -w'`);
/// that case wants `shlex::split` here and nothing else changed.
fn split_editor_command(editor: &str) -> Option<(String, Vec<String>)> {
    let mut parts = editor.split_whitespace();
    let program = parts.next()?.to_owned();
    Some((program, parts.map(str::to_owned).collect()))
}
```

and rewrite `open_in_editor_with`:

```rust
fn open_in_editor_with(editor: &str, file_path: &Path) -> Result<()> {
    let Some((program, args)) = split_editor_command(editor) else {
        bail!("No editor configured: EDITOR/VISUAL is empty");
    };

    let mut command = Command::new(&program);
    command.args(&args);
    command.arg(file_path);

    // For some editors like vim/nano, we need to inherit stdio
    command.stdin(Stdio::inherit());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let status = command.status().context("error running command")?;

    if !status.success() {
        bail!("Editor '{}' exited with non-zero status", editor);
    }

    Ok(())
}
```

- [ ] **Step 6: Run the tests to verify they PASS**

Run: `cargo test --lib editor::tests`
Expected: all PASS.

- [ ] **Step 7: Run the gate**

Run: `just gate`
Expected: green. The integration harness still points `EDITOR` at the single-word `false`, whose behavior is unchanged.

- [ ] **Step 8: Strip and commit**

```bash
todo-parser bughunt.md --strip B10
git add -A
git commit -m 'fix(correctness): split a multi-word $EDITOR into program and args [B10]'
```

---

### Task 9 (B11): Clone only the cache field each caller asked for

**Files:**
- Modify: `src/data_svc.rs:604-655`
- Test: `src/data_svc.rs` (`mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `async fn get_valid_entry<T>(&self, date: &Date, file_path: &Path, select: impl Fn(&CacheEntry) -> Option<T>) -> Result<Option<T>>` (private). Callers `get_cached_content` and `get_cached_parsed` keep their existing signatures.

The comment at `src/data_svc.rs:604-608` records an earlier regression here (a whole-entry clone taken *before* the validity check). Preserve that intent and extend it, rather than replacing the comment.

- [ ] **Step 1: Write the characterization tests**

These pin the behavior the projection must preserve — both must pass before and after.

```rust
    #[tokio::test]
    async fn a_cache_hit_serves_content_and_parse_from_the_same_entry() {
        let (service, _dir) = hermetic_service(60);
        let test_date = date!(2026 - 08 - 24);
        let path = service.get_file_path(test_date).await.unwrap();
        tokio::fs::write(&path, "8-10 admin\n").await.unwrap();

        let first = service.parse_day(&test_date).await.unwrap().unwrap();
        let content = service.read_day(&test_date).await.unwrap().unwrap();
        let second = service.parse_day(&test_date).await.unwrap().unwrap();

        assert_eq!(content, "8-10 admin\n");
        assert_eq!(first.total_minutes, second.total_minutes);
        assert_eq!(
            service.parse_count(),
            1,
            "content and parse must come from one shared validity check, \
             not two that can disagree"
        );
    }

    #[tokio::test]
    async fn an_entry_with_content_but_no_parse_yet_yields_no_parse() {
        // `cache_content` inserts with `parsed: None`. A projection that
        // conflated "no cached parse" with "no valid entry" would still be
        // correct here, but one that returned a stale or defaulted parse
        // would not.
        let (service, _dir) = hermetic_service(60);
        let test_date = date!(2026 - 08 - 24);
        let path = service.get_file_path(test_date).await.unwrap();
        tokio::fs::write(&path, "8-10 admin\n").await.unwrap();

        service.read_day(&test_date).await.unwrap();
        let file_path = service.get_file_path(test_date).await.unwrap();
        assert!(
            service
                .get_cached_parsed(&test_date, &file_path)
                .await
                .unwrap()
                .is_none(),
            "content cached without a parse must report no cached parse"
        );
        assert!(
            service
                .get_cached_content(&test_date, &file_path)
                .await
                .unwrap()
                .is_some(),
            "the same entry must still serve its content"
        );
    }
```

- [ ] **Step 2: Run them to verify they PASS on unchanged code**

Run: `cargo test --lib data_svc::tests::a_cache_hit_serves data_svc::tests::an_entry_with_content`
Expected: 2 passed.

- [ ] **Step 3: Project inside the lock**

In `src/data_svc.rs`, change `get_valid_entry`'s signature and its final success block. The signature becomes:

```rust
    /// Return the cache entry for `date` if it is still valid for
    /// `file_path`: within the cache timeout and not modified on disk since
    /// it was cached. Both `get_cached_content` and `get_cached_parsed` are
    /// built on this so the raw content and the parsed value share exactly
    /// one validity check and always expire together.
    ///
    /// `select` runs under the lock and decides what gets cloned out. The
    /// callers each want one field and immediately dropped the other; on a
    /// path that runs ~97 times per navigation keystroke, cloning the whole
    /// entry meant copying a day's raw text or its parsed form for nothing
    /// on every cache hit.
    async fn get_valid_entry<T>(
        &self,
        date: &Date,
        file_path: &Path,
        select: impl Fn(&CacheEntry) -> Option<T>,
    ) -> Result<Option<T>> {
```

Leave the metadata copy, the `let Some(...) else`, and the whole `if let Ok(duration) = ...` condition chain exactly as they are — including the `file_mod_time == cached_mod_time` inequality comment, which records its own separate fix. Only the block inside the `if` changes:

```rust
        {
            // File hasn't been modified, the entry is still good. Re-acquire
            // the lock to project it for return rather than reusing anything
            // read above: another task can have invalidated the entry
            // between the metadata copy and here, so this re-fetches and
            // yields `None` if it is gone instead of assuming it is still
            // there.
            let cache = self.cache.lock().await;
            return Ok(cache.get(date).and_then(select));
        }
```

- [ ] **Step 4: Update the two callers**

```rust
    /// Get cached content if valid, None otherwise
    async fn get_cached_content(&self, date: &Date, file_path: &Path) -> Result<Option<String>> {
        self.get_valid_entry(date, file_path, |entry| entry.data.clone())
            .await
    }

    /// Get the cached parse for `date` if it is still valid, None otherwise
    async fn get_cached_parsed(
        &self,
        date: &Date,
        file_path: &Path,
    ) -> Result<Option<TimeTrackingData>> {
        self.get_valid_entry(date, file_path, |entry| entry.parsed.clone())
            .await
    }
```

- [ ] **Step 5: Run the gate**

Run: `just gate`
Expected: green. Watch for a clippy lint about the closure — if it suggests `Clone::clone`, take the suggestion.

- [ ] **Step 6: Strip and commit**

```bash
todo-parser bughunt.md --strip B11
git add -A
git commit -m 'fix(caching): clone only the cache field each caller reads [B11]'
```

---

### Task 10 (B12): Fail loudly on an unparseable `--date`

**Files:**
- Modify: `src/config.rs:479-500` (`resolve_requested_date`), `src/config.rs:291` (`Config::load`), `src/config.rs:259-277` (add `try_get`)
- Modify: `cli/src/main.rs:21`
- Test: `src/config.rs` (`mod tests`), `cli/tests/cli_runtime_smoke.rs`

**Interfaces:**
- Consumes: `Config::try_init(use_args: bool) -> anyhow::Result<&'static Config>` (existing, private).
- Produces: `pub fn try_get() -> anyhow::Result<&'static Config>` — **additive**, mirrors the existing `try_get_no_args`. **Task 14 reads `config` from `main_impl` after this change; nothing else depends on it.**

**Constraint: this must not become a panic.** `Config::get()` goes through `init()`'s `.expect("Could not load configuration")`, so propagating the error out of `Config::load` alone would turn a typo'd date into a panic — strictly worse than today's silent fallback. The fallible accessor is the other half of the fix, not an optional extra.

- [ ] **Step 1: Write the failing unit test**

Add to `src/config.rs`'s `mod tests`:

```rust
    #[cfg(feature = "cli")]
    #[test]
    fn an_unparseable_date_is_an_error_not_a_silent_fallback_to_today() {
        // Silently substituting today meant `--date <typo>` exited 0 with a
        // full report for the wrong day — undetectable from a script, and
        // under `--tui` the eprintln was hidden by the alternate screen for
        // the whole session and never written to the log.
        let err = resolve_requested_date(Some("definitely-not-a-date".to_owned()))
            .expect_err("an unparseable date must be an error");
        let message = format!("{err:#}");
        assert!(
            message.contains("definitely-not-a-date"),
            "the error must quote the value that failed: {message}"
        );
    }

    #[cfg(feature = "cli")]
    #[test]
    fn a_parseable_date_still_resolves() {
        let resolved = resolve_requested_date(Some("2026-08-24".to_owned()))
            .expect("a well-formed date must resolve");
        assert_eq!(resolved, date!(2026 - 08 - 24));
    }

    #[cfg(feature = "cli")]
    #[test]
    fn no_date_argument_still_defaults_to_today() {
        let resolved = resolve_requested_date(None).expect("no date is not an error");
        assert_eq!(resolved, today_date());
    }
```

If `time::macros::date` is not already imported in `config.rs`'s test module, add `use time::macros::date;` there.

**Before relying on `"definitely-not-a-date"`, confirm `interim` actually rejects it** — `interim::parse_date_string` accepts relative forms like `tomorrow` and `next friday`. Run Step 2 and read the failure: if the test fails because the value *parsed*, pick a value that does not (`"@@@"` is a safe fallback) and use the same value in Step 5's integration test.

- [ ] **Step 2: Run the tests to verify they FAIL to compile**

Run: `cargo test --lib config::tests::an_unparseable_date`
Expected: FAIL — `resolve_requested_date` returns `Date`, not `Result<Date>`, so `.expect_err` does not exist on it.

- [ ] **Step 3: Make `resolve_requested_date` fallible**

`resolve_requested_date` is private (`src/config.rs:479`, no `pub`), so this is not an API change. Replace it:

```rust
#[cfg(feature = "cli")]
fn resolve_requested_date(date_str: Option<String>) -> Result<Date> {
    let Some(date_str) = date_str else {
        // No date argument at all is not a failure — it means today.
        return Ok(today_date());
    };

    use interim::{Dialect, parse_date_string};
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    parse_date_string(&date_str, now, Dialect::Us)
        .map(|date_time| date_time.date())
        .map_err(|e| anyhow::anyhow!("{e:?}"))
        .with_context(|| format!("could not parse the requested date '{date_str}'"))
}
```

Then at `src/config.rs:291`, inside `Config::load`:

```rust
        config.date = resolve_requested_date(args.date.or(args.positional_date))?;
```

- [ ] **Step 4: Add the fallible accessor and use it from `main`**

In `src/config.rs`, beside `get_no_args` / `try_get_no_args` / `get`:

```rust
    /// [`Config::get`]'s fallible twin: the process-wide configuration,
    /// parsing real argv on the first call, surfacing a load failure instead
    /// of panicking on it.
    ///
    /// `get` reaches [`Config::init`], which `.expect`s. That is fine for a
    /// missing config directory and wrong for a mistyped `--date`, which
    /// should exit non-zero with a message rather than dump a panic. The CLI
    /// entry point uses this; the library paths that cannot report an error
    /// usefully still use `get`.
    pub fn try_get() -> anyhow::Result<&'static Config> {
        Self::try_init(true)
    }
```

In `cli/src/main.rs`, replace line 21:

```rust
    let config = Config::get();
```

with:

```rust
    // `try_get`, not `get`: a mistyped `--date` used to be swallowed and
    // replaced with today, exiting 0 with a report for the wrong day. It is
    // now a load error, and it should reach the user as a message and a
    // non-zero exit rather than as a panic out of `Config::get`'s `.expect`.
    let config = Config::try_get()?;
```

- [ ] **Step 5: Write the integration test**

Append to `cli/tests/cli_runtime_smoke.rs`:

```rust
/// A mistyped date must not silently become today.
#[test]
fn an_unparseable_date_exits_non_zero() {
    use std::time::Duration;

    let data_dir = tempfile::tempdir().expect("failed to create temp dir");
    let config_home = tempfile::tempdir().expect("failed to create config dir");

    let mut cmd = common::ttcli();
    cmd.args(["--noedit", "--date", "definitely-not-a-date"]);
    common::scoped(&mut cmd, data_dir.path(), config_home.path());

    let output = common::output_within(cmd, Duration::from_secs(20));

    assert!(
        !output.status.success(),
        "an unparseable --date must exit non-zero, not report the wrong day"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("definitely-not-a-date"),
        "the message must name the value that failed: {stderr}"
    );
}
```

- [ ] **Step 6: Run the tests to verify they PASS**

Run: `cargo test --lib config::tests` then `cargo test -p cli --test cli_runtime_smoke an_unparseable_date`
Expected: PASS. Verify by hand too, with the safe invocation:

```bash
cargo run -p cli -- --noedit --data-directory /tmp/ttcli-check --date definitely-not-a-date; echo "exit=$?"
```

Expected: a message naming the date and `exit=1`.

- [ ] **Step 7: Run the gate**

Run: `just gate`
Expected: green. `resolve_requested_date` is `#[cfg(feature = "cli")]` and all three gate configs enable `cli` transitively, so the new tests run in all three.

- [ ] **Step 8: Strip and commit**

```bash
todo-parser bughunt.md --strip B12
git add -A
git commit -m 'fix(api-surface): fail non-zero on an unparseable --date [B12]'
```

- [ ] **Step 9: MILESTONE — full suite**

Run: `just gate` and `cd site && yarn test --run && yarn lint && npx tsc --noEmit`
Expected: both green. On red, bisect within B8-B12, revert the offender, surface the diagnosis.

---

### Task 11 (B13): Sweep expired entries out of the cache map

**Files:**
- Modify: `src/data_svc.rs:672-686` (`cache_content`)
- Test: `src/data_svc.rs` (`mod tests`)

**Interfaces:**
- Consumes: `CacheEntry { data, parsed, file_mod_time, cached_at }`, `self.cache_timeout: u64`.
- Produces: nothing later tasks rely on.

`cache_content` (line 685) is the **only** production `cache.insert` — `cache_parsed` mutates an existing entry via `get_mut` and never grows the map, and `invalidate_date`/`clear_cache` only remove. Sweeping on that one insert bounds the map. Sweep **before** inserting, so the entry being written always survives.

- [ ] **Step 1: Write the failing test**

```rust
    #[tokio::test]
    async fn caching_a_day_sweeps_entries_that_have_expired() {
        // The 30s TTL was enforced only by `get_valid_entry` refusing to
        // *return* a stale entry; nothing removed it. A `--serve` daemon
        // accumulated one entry — raw content plus parsed struct — per
        // distinct date ever requested, for the life of the process.
        let (service, _dir) = hermetic_service(0);

        let old = date!(2026 - 08 - 24);
        let new = date!(2026 - 08 - 25);
        for day in [old, new] {
            let path = service.get_file_path(day).await.unwrap();
            tokio::fs::write(&path, "8-10 admin\n").await.unwrap();
        }

        service.read_day(&old).await.unwrap();
        {
            let cache = service.cache.lock().await;
            assert_eq!(cache.len(), 1, "the first read caches its own day");
        }

        service.read_day(&new).await.unwrap();
        {
            let cache = service.cache.lock().await;
            assert_eq!(
                cache.len(),
                1,
                "caching a day must evict entries already past the TTL, not \
                 grow the map forever"
            );
            assert!(
                cache.contains_key(&new),
                "the entry just written must survive its own sweep"
            );
        }
    }

    #[tokio::test]
    async fn caching_a_day_keeps_entries_still_inside_the_ttl() {
        let (service, _dir) = hermetic_service(60);

        let first = date!(2026 - 08 - 24);
        let second = date!(2026 - 08 - 25);
        for day in [first, second] {
            let path = service.get_file_path(day).await.unwrap();
            tokio::fs::write(&path, "8-10 admin\n").await.unwrap();
            service.read_day(&day).await.unwrap();
        }

        let cache = service.cache.lock().await;
        assert_eq!(
            cache.len(),
            2,
            "a sweep must not evict entries that are still fresh"
        );
    }
```

- [ ] **Step 2: Run the tests to verify the first FAILS**

Run: `cargo test --lib data_svc::tests::caching_a_day`
Expected: `caching_a_day_sweeps_entries_that_have_expired` FAILS with `cache.len()` of 2. `caching_a_day_keeps_entries_still_inside_the_ttl` PASSES (it guards against over-eviction).

- [ ] **Step 3: Sweep on insert**

In `src/data_svc.rs`, `cache_content` currently ends:

```rust
    async fn cache_content(&self, date: Date, file_mod_time: Option<SystemTime>, content: &str) {
        let mut cache = self.cache.lock().await;

        let entry = CacheEntry {
            data: Some(content.to_string()),
            parsed: None,
            file_mod_time,
            cached_at: SystemTime::now(),
        };

        cache.insert(date, entry);
    }
```

Extend the doc comment and add the sweep:

```rust
    /// Cache content for a date. Freshly read content has no known parse
    /// yet, so `parsed` starts `None` and is filled in by `cache_parsed`
    /// once `parse_day` actually runs the parser.
    ///
    /// Also the map's one growth point in production — `cache_parsed` only
    /// mutates an existing entry — so it is where expired entries get swept.
    /// The TTL used to be enforced only on read, by `get_valid_entry`
    /// declining to serve a stale entry; nothing ever removed one, so a
    /// long-lived `--serve` process held every date it had ever been asked
    /// for. The sweep runs before the insert, so the entry being written
    /// always survives it.
    async fn cache_content(&self, date: Date, file_mod_time: Option<SystemTime>, content: &str) {
        let mut cache = self.cache.lock().await;

        let now = SystemTime::now();
        cache.retain(|_, entry| {
            now.duration_since(entry.cached_at)
                .is_ok_and(|age| age.as_secs() < self.cache_timeout)
        });

        let entry = CacheEntry {
            data: Some(content.to_string()),
            parsed: None,
            file_mod_time,
            cached_at: now,
        };

        cache.insert(date, entry);
    }
```

Note `duration_since` returning `Err` (a `cached_at` in the future, from clock skew) evicts the entry. That matches `get_valid_entry`, which also declines to serve it.

- [ ] **Step 4: Run the tests to verify they PASS**

Run: `cargo test --lib data_svc::tests::caching_a_day`
Expected: 2 passed.

- [ ] **Step 5: Run the gate**

Run: `just gate`
Expected: green. Pay attention to `test_clear_cache` and `test_cache_invalidation`, which insert entries by hand with `cached_at: SystemTime::now()` and a `hermetic_service(60)` — still fresh, so the sweep leaves them alone.

- [ ] **Step 6: Strip and commit**

```bash
todo-parser bughunt.md --strip B13
git add -A
git commit -m 'fix(caching): sweep expired entries instead of only declining them [B13]'
```

---

### Task 12 (B15): 404 unmatched `/api` and `/graphql` paths

**Files:**
- Modify: `src/web.rs:103-146` (extract `build_router`, scope the SPA fallback)
- Test: `src/web.rs` `mod tests`

**Interfaces:**
- Consumes: `LogCapture`, `capture_logs`, `runtime` from Task 2's test module; `tower::ServiceExt` (Task 2, Step 1).
- Produces: `fn build_router(state: AppState) -> Router` (private) — everything `run_server` does except binding and serving.

- [ ] **Step 1: Extract `build_router`**

In `src/web.rs`, `run_server` currently builds `state`, `context`, `qm_schema`, the two routers, and `app` inline. Split the router construction out, leaving `run_server`'s signature untouched:

```rust
/// Everything [`run_server`] assembles except the listener: the GraphQL
/// routes, the embedded SPA, the API routes and the middleware stack.
///
/// A separate function so tests can drive the real router with
/// `ServiceExt::oneshot` instead of binding a port and issuing HTTP.
fn build_router(state: AppState) -> Router {
    let context = GraphQLContext::new(state.clone());
    let qm_schema = create_schema();

    let middleware = ServiceBuilder::new().layer(CompressionLayer::new());
    let graphql_routes = Router::new()
        .route(
            "/",
            on(MethodFilter::GET.or(MethodFilter::POST), custom_graphql),
        )
        .route(
            "/graphiql",
            get(graphiql("/graphql", "/graphql/subscriptions")),
        )
        .route(
            "/playground",
            get(playground("/graphql", "/graphql/subscriptions")),
        )
        // The GraphQL side gets its own fallback rather than an outer
        // `/graphql/{*rest}` catch-all: `nest` already installs an internal
        // catch-all for its prefix, and a second one on the outer router is a
        // route conflict that panics at build time. A nested router's own
        // fallback wins inside its prefix, which is exactly the scope wanted.
        .fallback(api_not_found)
        .layer(Extension(context.clone()))
        .layer(Extension(Arc::new(qm_schema)))
        .layer(middleware.clone());

    let serve_assets = ServeEmbed::<SiteAssets>::with_parameters(
        Some("/index.html".to_string()),
        FallbackBehavior::Ok,
        None,
    );

    let fallback_serve_assets = serve_assets.clone();

    Router::new()
        .route_service("/assets/{*uri}", serve_assets)
        .layer(middleware::from_fn(set_static_cache_control))
        .route("/api/day", get(get_day_data))
        .route("/api/day/{date}", get(get_day_data_by_date))
        .route("/api/week", get(get_week_data))
        .route("/api/week/{date}", get(get_week_data_by_date))
        // Unmatched `/api/*` must 404 rather than fall through to the SPA —
        // see `api_not_found` below. Nothing nests under `/api`, so a plain
        // catch-all route is safe here.
        .route("/api/{*rest}", any(api_not_found))
        .nest("/graphql", graphql_routes)
        .fallback_service(fallback_serve_assets)
        .layer(CorsLayer::permissive())
        .layer(Extension(context))
        .layer(middleware)
        .with_state(state)
}

/// The 404 the SPA fallback would otherwise swallow.
///
/// `axum-embed`'s `FallbackBehavior::Ok` forces HTTP 200 with index.html for
/// *any* unresolved path, so `/api/dayz`, `/api/day/2026-01-01/extra` and
/// `/graphql/nonexistent` all looked like successes to a probing monitor or
/// a client with a typo. The SPA fallback is right for app routes and wrong
/// for the API surface; these catch-alls draw that line.
async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}
```

and reduce `run_server`'s body to:

```rust
pub async fn run_server(port: u16, config: Config, rx: Receiver<()>) -> anyhow::Result<()> {
    let app = build_router(AppState { config });

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

    info!(
        "Time Tracking Web Server running on http://localhost:{}",
        port
    );
    info!("Access your time tracking data via the web interface");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(rx))
        .await?;

    Ok(())
}
```

Add `any` to the routing import at `src/web.rs:12`:

```rust
    routing::{MethodFilter, any, get, on},
```

**Route-precedence check:** axum matches the more specific route first, so `/api/day/{date}` still wins over `/api/{*rest}`, and `/graphql`, `/graphql/graphiql`, `/graphql/playground` still win over the nested router's fallback. `/api/{*rest}` requires at least one segment after `/api/`, so a bare `/api` still falls to the SPA — Step 2's tests assert every one of these.

**Do not add a `/graphql/{*rest}` route.** `Router::nest` registers its own internal catch-all for the nest prefix; a second wildcard on the same prefix is a route conflict, and axum panics on it while building the router. If Step 2's `/graphql/nonexistent` test still returns 200 with the inner `.fallback(api_not_found)` in place, that is the finding to chase — do not work around it by adding the conflicting route.

- [ ] **Step 2: Write the failing tests**

Add to `src/web.rs`'s `mod tests`:

```rust
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt as _;

    fn get_status(uri: &str) -> StatusCode {
        let rt = runtime();
        rt.block_on(async {
            build_router(AppState::default())
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response")
                .status()
        })
    }

    #[test]
    fn unmatched_api_and_graphql_paths_are_not_found() {
        for uri in [
            "/api/dayz",
            "/api/day/2026-01-01/extra",
            "/api/nope",
            "/graphql/nonexistent",
        ] {
            assert_eq!(
                get_status(uri),
                StatusCode::NOT_FOUND,
                "{uri} must 404, not return 200 with the SPA's index.html"
            );
        }
    }

    #[test]
    fn registered_routes_still_resolve() {
        // The other half of the assertion: the catch-alls must not shadow
        // anything real.
        assert_ne!(
            get_status("/api/day/2026-08-24"),
            StatusCode::NOT_FOUND,
            "a registered day route must still resolve"
        );
        assert_ne!(
            get_status("/api/week/2026-08-24"),
            StatusCode::NOT_FOUND,
            "a registered week route must still resolve"
        );
        assert_ne!(
            get_status("/graphql/graphiql"),
            StatusCode::NOT_FOUND,
            "the nested graphiql route must still resolve"
        );
    }

    #[test]
    fn spa_routes_still_fall_through_to_index() {
        // The SPA fallback is right for app routes; only the API surface
        // changed.
        assert_eq!(
            get_status("/editor/2026-08-24"),
            StatusCode::OK,
            "a client-side route must still be served the SPA"
        );
        assert_eq!(
            get_status("/api"),
            StatusCode::OK,
            "a bare /api has no trailing segment and is not an API path"
        );
    }
```

- [ ] **Step 3: Run the tests to verify the first FAILS before the Step-1 additions**

Comment out the `.route("/api/{*rest}", any(api_not_found))` line and the `graphql_routes` `.fallback(api_not_found)` line, then run:

Run: `cargo test --features webapp --lib web::tests::unmatched_api`
Expected: FAIL — every URI returns 200 with the SPA's index.html. Restore both lines and re-run: PASS.

- [ ] **Step 4: Run the gate**

Run: `just gate`
Expected: green.

- [ ] **Step 5: Strip and commit**

```bash
todo-parser bughunt.md --strip B15
git add -A
git commit -m 'fix(api-surface): 404 unmatched /api and /graphql paths [B15]'
```

---

### Task 13 (B16): Report a failed watch retarget on the status line

**Files:**
- Modify: `src/tui/app.rs:1741-1747`
- Test: `src/tui/app.rs` (`mod tests`)

**Interfaces:**
- Consumes: `App::set_status(&mut self, message: impl Into<String>)` (`src/tui/app.rs:1316`); `App::new(TuiContext::for_test())`; the `status_text(&App) -> Option<&str>` test helper (`src/tui/app.rs:2098`).
- Produces: `fn report_watch_failure(&mut self, error: &anyhow::Error)` (private) — the seam that makes the `Err` arm reachable from a test.

`retarget_watch`'s `Err` arm is unreachable in a test: `DataService::get_file_path` fails only via `DataDir::FromConfig`'s `get_time_tracking_dir()`, and test services use `DataDir::Fixed`, whose `resolve` is infallible (`src/data_svc.rs:52-59`). Extracting the reporting into its own method makes the branch testable without contorting the service.

- [ ] **Step 1: Write the failing test**

Add to `src/tui/app.rs`'s `mod tests`:

```rust
    #[test]
    fn a_failed_watch_retarget_reaches_the_status_line() {
        // Every other fallible path in this file pairs its warn!/error! with
        // a set_status, because — per the clipboard fix's own comment above
        // `copy_to_clipboard` — a log-only failure "went to a log file the
        // alternate screen hides" and was not observable at all. This one
        // was log-only, so the mtime watch could stop and the user would
        // never learn that external edits had stopped being detected.
        let mut app = App::new(TuiContext::for_test());
        assert_eq!(status_text(&app), None);

        app.report_watch_failure(&anyhow::anyhow!("no such directory"));

        let message = status_text(&app).expect("a failed watch must set a status");
        assert!(
            message.contains("no such directory"),
            "the status must carry the cause: {message}"
        );
    }
```

- [ ] **Step 2: Run the test to verify it FAILS**

Run: `cargo test --lib tui::app::tests::a_failed_watch_retarget`
Expected: FAIL to compile — `no method named report_watch_failure`.

- [ ] **Step 3: Add the reporting method and call it**

In `src/tui/app.rs`, replace `retarget_watch`'s `Err` arm and add the method beside it:

```rust
    async fn retarget_watch(&mut self) {
        self.stop_watch();
        match self.data_svc.get_file_path(self.active_date).await {
            Ok(path) => self.watch = Some(spawn_mtime_watch(path, self.events.sender())),
            Err(e) => self.report_watch_failure(&e),
        }
    }

    /// Report a watch that could not be established, both ways.
    ///
    /// A `tracing::warn!` on its own goes to a log file the alternate screen
    /// hides — the same failure mode the clipboard path was fixed for. When
    /// this fires the mtime watch is not running, so external edits to the
    /// active day silently stop being picked up for the rest of the session;
    /// the user needs to see that, not just the log.
    ///
    /// A separate method rather than an inline arm because
    /// `get_file_path` only fails under `DataDir::FromConfig`, which no test
    /// service uses — this is what makes the branch reachable from a test.
    fn report_watch_failure(&mut self, error: &anyhow::Error) {
        tracing::warn!("could not resolve a path to watch: {error}");
        self.set_status(format!("Could not watch for external changes: {error}"));
    }
```

- [ ] **Step 4: Run the test to verify it PASSES**

Run: `cargo test --lib tui::app::tests::a_failed_watch_retarget`
Expected: PASS.

- [ ] **Step 5: Run the gate**

Run: `just gate`
Expected: green. This code is `tui`-gated, so it is exercised in the default and `tui`-only configs.

- [ ] **Step 6: Strip and commit**

```bash
todo-parser bughunt.md --strip B16
git add -A
git commit -m 'fix(observability): report a failed watch retarget on the status line [B16]'
```

---

### Task 14 (B17): Warn when `--stdin` ignores the other mode flags

**Files:**
- Modify: `cli/src/main.rs:23-30`
- Test: `cli/src/main.rs` (new `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `Config::try_get()` (Task 10); `Config` fields `stdin: bool`, `serve: Option<bool>`, `week: bool`, and — under the `tui` feature — `tui: Option<bool>`. Note `serve` is **not** feature-gated (`src/config.rs:181`) while `tui` is (`src/config.rs:187`).
- Produces: `fn ignored_stdin_flags(config: &Config) -> Vec<&'static str>` (private in `cli/src/main.rs`).

**Scope note — `--noedit` is deliberately excluded** from the warning even though the finding lists it. In stdin mode no editor is ever launched, so `--noedit`'s intent is satisfied rather than ignored; warning about it would alarm anyone who passes it as a habitual safety flag. The three flags that genuinely change nothing while silently promising to are `--serve`, `--week`, and `--tui`.

**Sink: `tracing::warn!`, not `eprintln!`.** Consistent with B9's fix, and stdin mode writes its report to stdout, so keep both streams clean for the caller.

- [ ] **Step 1: Write the failing test**

Append to `cli/src/main.rs`:

```rust
#[cfg(test)]
mod tests {
    // `Config` already arrives via the file's own
    // `use time_tracking_cli::{Config, ...}`; importing it again here is an
    // E0252 duplicate-import error.
    use super::*;

    fn stdin_config() -> Config {
        Config {
            stdin: true,
            ..Config::default()
        }
    }

    #[test]
    fn a_plain_stdin_run_reports_nothing_ignored() {
        assert!(ignored_stdin_flags(&stdin_config()).is_empty());
    }

    #[test]
    fn stdin_names_the_mode_flags_it_drops() {
        // `main_impl` returns straight after `show_single_day_stdin`, before
        // serve/week/tui are ever consulted, so
        // `ttcli --stdin --serve --port 3000` started no server and said so
        // nowhere.
        let config = Config {
            serve: Some(true),
            week: true,
            ..stdin_config()
        };
        let ignored = ignored_stdin_flags(&config);
        assert!(ignored.contains(&"--serve"), "{ignored:?}");
        assert!(ignored.contains(&"--week"), "{ignored:?}");
    }

    #[cfg(feature = "tui")]
    #[test]
    fn stdin_names_a_dropped_tui_flag() {
        let config = Config {
            tui: Some(true),
            ..stdin_config()
        };
        assert_eq!(ignored_stdin_flags(&config), vec!["--tui"]);
    }

    #[test]
    fn flags_that_are_off_are_not_reported() {
        let config = Config {
            serve: Some(false),
            week: false,
            ..stdin_config()
        };
        assert!(ignored_stdin_flags(&config).is_empty());
    }
}
```

- [ ] **Step 2: Run the tests to verify they FAIL**

Run: `cargo test -p cli --bin ttcli`
Expected: FAIL to compile — `cannot find function ignored_stdin_flags`.

- [ ] **Step 3: Implement the warning**

In `cli/src/main.rs`, add above `main_impl`:

```rust
/// The mode flags `--stdin` silently drops, in the order they are documented.
///
/// `main_impl` answers `--stdin` and returns before `serve`/`week`/`tui` are
/// ever consulted, so `ttcli --stdin --serve --port 3000` started no server
/// and printed nothing to say so. `--noedit` is deliberately absent: stdin
/// mode launches no editor at all, so its intent is satisfied rather than
/// ignored, and naming it would alarm anyone passing it as a safety habit.
fn ignored_stdin_flags(config: &Config) -> Vec<&'static str> {
    let mut ignored = Vec::new();
    if config.serve == Some(true) {
        ignored.push("--serve");
    }
    if config.week {
        ignored.push("--week");
    }
    #[cfg(feature = "tui")]
    if config.tui == Some(true) {
        ignored.push("--tui");
    }
    ignored
}
```

and change the `config.stdin` block in `main_impl`:

```rust
    if config.stdin {
        let ignored = ignored_stdin_flags(config);
        if !ignored.is_empty() {
            // Logged rather than printed: stdin mode writes its report to
            // stdout, and that stream belongs to the caller.
            tracing::warn!(
                "--stdin takes precedence; these flags were ignored: {}",
                ignored.join(", ")
            );
        }

        let formatter = config.get_formatter();
        show_single_day_stdin(formatter.as_ref())
            .await
            .context("generating report from stdin")?;

        return Ok(());
    }
```

- [ ] **Step 4: Run the tests to verify they PASS**

Run: `cargo test -p cli --bin ttcli`
Expected: all PASS.

- [ ] **Step 5: Run the gate**

Run: `just gate`
Expected: green. `ignored_stdin_flags` must compile in all three feature configs — the `#[cfg(feature = "tui")]` guards the only gated field, and `serve` is ungated.

- [ ] **Step 6: Strip and commit**

```bash
todo-parser bughunt.md --strip B17
git add -A
git commit -m 'fix(api-surface): warn when --stdin ignores the other mode flags [B17]'
```

- [ ] **Step 7: MILESTONE — full suite, final**

Run: `just gate` and `cd site && yarn test --run && yarn lint && npx tsc --noEmit`
Expected: both green.

Then confirm `bughunt.md` holds only the unmarked items:

```bash
todo-parser bughunt.md --summary
```

Expected: 3 active items, 0 marked execute, 0 marked skip.

---

## Notes for the executor

- **`decision-needed`, not a fix.** If any task turns out to require a big rewrite, a `pub` signature break, or an architectural change, stop that task, convert the finding to a `decision-needed` marker in `bughunt.md`, and move on. Never auto-apply one.
- **Do not use bare `git stash` / `git stash pop`.** The stash stack is shared with the main checkout and other worktrees. Where a step needs to see pre-fix behavior, revert the specific lines by hand as the step describes.
- **Task 2 Step 1 and Task 12 both need `tower::ServiceExt`.** If Task 2 skipped the `Cargo.toml` edit because `oneshot` already resolved, Task 12 needs no change either.
- **`site/build/` must keep existing** for `just gate`. No task rebuilds it; if it disappears, run `cd site && yarn install && yarn build`.
