# Tidy Batch (25 findings) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the 25 findings the user selected from `TIDY.md` — style and structure cleanup plus a handful of real bug fixes — one revertable commit per finding.

**Architecture:** Sequential execution in dependency waves. Two shared helpers (a local-date module and a clipboard module) land before the findings that consume them, because extracting them naively would bake in the very bug three other findings exist to fix. Rust and TypeScript findings are independent of each other except through the shared verification gate.

**Tech Stack:** Rust 2024 (workspace: `.` library + `cli/` binary; features `webapp`, `tui`, `cli`), React 19 + TypeScript + Vite + Apollo + Tailwind (`site/`), Vitest, ratatui, Axum, Juniper.

**Spec:** `docs/superpowers/specs/2026-08-29-tidy-batch-design.md`

## Global Constraints

- Work in the worktree `/home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy` on branch `tidy/2026-08-29`. It already has `site/build/` and a `site/node_modules` symlink; both are required to compile the `webapp` feature.
- `export SKIP_YARN=1` for every cargo command, or `build.rs` reruns `yarn build`.
- Rust edition 2024. All workspace crates' editions must equal `rustfmt.toml`'s edition.
- Commits follow Conventional Commits (Husky + commitlint enforce it). **Never use `--no-verify`.**
- Every commit body ends with a blank line then: `Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP`
- Each task commits the code change **and** the `todo-parser TIDY.md --strip T<n>` result **together**, in one commit. This is what keeps each fix independently revertable. Non-negotiable.
- Commit message format: `tidy(<lens>): <summary> [T<n>]`
- **Never run the binary bare.** A plain `ttcli` or `cargo run -p cli --` defaults to the real `~/.time-tracking/` and opens `$EDITOR` on today's file. Always pass `--noedit --data-directory <tmp>`.
- **Do not refactor existing tests.** New tests are welcome. The only permitted edit to an existing test artifact in this batch is Task 15's golden-file regeneration.
- **Never delete a `pub` Rust item.** The library is linked out-of-repo by `time-tracking-nvim`, so "no callers per `git grep`" does not prove an item is unused. No task here deletes one; if a fix appears to require it, stop and surface it.
- Frontend lint must be run as `./node_modules/.bin/eslint src`, never `eslint .`, until Task 1 lands.

## Verification commands

```bash
export SKIP_YARN=1
cargo check --workspace --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
cargo fmt --all
cd site && ./node_modules/.bin/eslint src --report-unused-disable-directives --max-warnings 0
cd site && yarn test --run
cd site && yarn build
```

Full gate (three feature combinations, slow — final milestone only): `just gate`

## File Structure

**Created:**
- `site/src/utils/date.ts` — local-time date string formatting and parsing. Single source of truth for the `YYYY-MM-DD` boundary between `Date` objects and the GraphQL API.
- `site/src/utils/__tests__/date.test.ts` — its tests.
- `site/src/utils/clipboard.ts` — notes-to-clipboard with toast feedback.
- `site/src/components/WeeklySummary/useWeeklyTableData.ts`, `useNotesLookup.ts`, `ProjectRow.tsx`, `DailyTotalsRow.tsx` — the four units split out of `WeeklySummary.tsx` (Task 25).

**Deleted:**
- `site/src/components/BorderedTableCell.tsx` (Task 10).

**Heavily modified:** `src/config.rs`, `src/data_svc.rs`, `src/display/{mod,plain,default}.rs`, `cli/src/main.rs`, `site/src/components/WeeklySummary.tsx`.

---

## Wave 0 — Unblock the toolchain

### Task 1: Point ESLint at the real build output [T1]

**Files:**
- Modify: `site/eslint.config.js:11`

**Interfaces:**
- Consumes: nothing.
- Produces: a fast `yarn lint`. Every later frontend task depends on this to run lint in seconds instead of 12+ minutes.

**Why first:** Vite's `outDir` is `build` (`site/vite.config.ts:14`), but the ESLint flat config ignores only `dist`. So `eslint .` lints the entire 1.4 MB production bundle. Measured: 12+ minutes (killed, pegged at 100% CPU) vs 1.07s with `build/` ignored, zero errors either way.

- [ ] **Step 1: Confirm the slow path before changing it**

```bash
cd site
time timeout 120 ./node_modules/.bin/eslint . --report-unused-disable-directives --max-warnings 0
```

Expected: TIMES OUT at 120s (exit 124). This is the bug.

- [ ] **Step 2: Apply the fix**

In `site/eslint.config.js`, change line 11 from:

```javascript
  { ignores: ["dist"] },
```

to:

```javascript
  { ignores: ["dist", "build", "coverage"] },
```

- [ ] **Step 3: Verify it is now fast and still clean**

```bash
cd site
time ./node_modules/.bin/eslint . --report-unused-disable-directives --max-warnings 0
```

Expected: exit 0, no output, completes in under 10 seconds.

- [ ] **Step 4: Confirm source files are still actually linted**

```bash
cd site
./node_modules/.bin/eslint . --report-unused-disable-directives --max-warnings 0 --debug 2>&1 | grep -c 'src/components'
```

Expected: a non-zero count. This proves the new ignore list did not accidentally exclude `src/`.

- [ ] **Step 5: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T1
git add -A && git commit -m "tidy(idioms): ignore build/ and coverage/ in eslint config [T1]

Vite's outDir is build, but the flat config only ignored dist, so
\`eslint .\` linted the whole 1.4MB production bundle: 12+ minutes
versus 1.07s once build/ is excluded. Zero lint errors either way.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

---

## Wave 1 — Independent deletions and one-liners

### Task 2: Drop the unused serde_json dependency [T9]

**Files:**
- Modify: `Cargo.toml:21` (webapp feature list), `Cargo.toml:71` (dependency)

**Interfaces:**
- Consumes: nothing. Produces: nothing.

- [ ] **Step 1: Re-confirm zero usages**

```bash
git grep -n 'serde_json' -- src cli/src build.rs
```

Expected: no output. (`web.rs`/`graphql.rs` use `serde::{Deserialize, Serialize}` through axum's `Json<T>`, not `serde_json` directly.)

- [ ] **Step 2: Remove from the feature list**

In `Cargo.toml`, delete the `"serde_json",` line from the `webapp = [...]` array.

- [ ] **Step 3: Remove the dependency**

Delete the line `serde_json = { version = "1.0", optional = true }`.

- [ ] **Step 4: Verify all three feature combinations still build**

```bash
export SKIP_YARN=1
cargo check --workspace --all-targets --all-features
cargo check --workspace --no-default-features --features webapp --all-targets
cargo check --workspace --no-default-features --features tui --all-targets
```

Expected: all three exit 0.

- [ ] **Step 5: Strip and commit**

```bash
todo-parser TIDY.md --strip T9
git add -A && git commit -m "tidy(dead-code): drop the unused serde_json dependency [T9]

Declared under the webapp feature but never referenced in src/ or
cli/src/; JSON responses go through axum's Json<T> with serde derive.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 3: Drop the unused juniper_graphql_ws dependency [T10]

**Files:**
- Modify: `Cargo.toml:24` (webapp feature list), `Cargo.toml:72-75` (dependency)

**Interfaces:**
- Consumes: nothing. Produces: nothing.

**Context:** `src/graphql.rs:138` defines `Schema = RootNode<Query, Mutation, EmptySubscription<GraphQLContext>>`. `src/web.rs:109` and `:113` mention `"/graphql/subscriptions"` only as a **string URL** passed to `graphiql()`/`playground()`; no websocket route is ever mounted. Subscriptions have never worked. Removing the dependency changes no behavior — do NOT also remove those URL strings, that is out of scope.

- [ ] **Step 1: Re-confirm zero usages**

```bash
git grep -n 'juniper_graphql_ws\|graphql_ws' -- src cli/src build.rs
```

Expected: no output.

- [ ] **Step 2: Remove from the feature list**

In `Cargo.toml`, delete the `"juniper_graphql_ws",` line from the `webapp = [...]` array.

- [ ] **Step 3: Remove the dependency**

Delete the four-line `juniper_graphql_ws = { git = ..., features = ["graphql-transport-ws"], optional = true }` entry.

- [ ] **Step 4: Verify**

```bash
export SKIP_YARN=1
cargo check --workspace --all-targets --all-features
cargo check --workspace --no-default-features --features webapp --all-targets
cargo test --workspace --no-default-features --features webapp
```

Expected: all exit 0.

- [ ] **Step 5: Strip and commit**

```bash
todo-parser TIDY.md --strip T10
git add -A && git commit -m "tidy(dead-code): drop the unused juniper_graphql_ws dependency [T10]

The schema uses EmptySubscription and no websocket route is ever
mounted; /graphql/subscriptions appears only as a URL string handed
to graphiql and playground.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 4: Drop four unused npm dependencies [T12, T13, T14, T15]

**Files:**
- Modify: `site/package.json` (4 lines), `site/yarn.lock` (regenerated)

**Interfaces:**
- Consumes: nothing. Produces: nothing.

**Note:** This is one task covering four findings because they share a single `yarn install` + verification cycle and would otherwise thrash the lockfile four times. Strip all four IDs in the one commit.

- [ ] **Step 1: Re-confirm zero usages for all four**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
for d in '@react-hook/debounce' '@uidotdev/usehooks' 'uuid' 'webfontloader'; do
  printf '%-24s -> ' "$d"
  git grep -l "$d" -- site/src site/index.html site/vite.config.ts | wc -l
done
```

Expected: `0` for each. (`site/src/hooks/useDebounce.ts` hand-rolls its own `useState`/`useEffect`/`setTimeout` debounce rather than importing `@react-hook/debounce`.)

- [ ] **Step 2: Remove the four dependency lines**

In `site/package.json`, delete these four lines from `"dependencies"`:

```json
    "@react-hook/debounce": "^4.0.0",
    "@uidotdev/usehooks": "^2.4.1",
    "uuid": "^11.1.0",
    "webfontloader": "^1.6.28"
```

Mind the trailing comma on the entry that ends up last in the object.

- [ ] **Step 3: Regenerate the lockfile**

```bash
cd site && yarn install
```

Expected: succeeds; `yarn.lock` shrinks.

- [ ] **Step 4: Verify the app still builds, tests and lints**

```bash
cd site
yarn build
yarn test --run
./node_modules/.bin/eslint src --report-unused-disable-directives --max-warnings 0
```

Expected: all three exit 0. If `yarn build` fails, one of the four was a transitive requirement — restore just that one, note which, and continue with the rest.

- [ ] **Step 5: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T12 --strip T13 --strip T14 --strip T15
git add -A && git commit -m "tidy(dead-code): drop four unused npm dependencies [T12] [T13] [T14] [T15]

@react-hook/debounce, @uidotdev/usehooks, uuid and webfontloader have
zero imports in site/src, index.html or vite.config.ts. useDebounce.ts
implements its own debounce rather than using the package.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 5: Delete the eleven-month-old commented-out clsx entries [T41]

**Files:**
- Modify: `site/src/components/Button/index.tsx:24,40,41`

**Interfaces:**
- Consumes: nothing.
- Produces: leaves `getVariant` with no remaining reference, which Task 6 then deletes. **Task 5 must run before Task 6.**

**Context:** `git blame` dates all three lines to commit `b3857ca`, 2025-10-04.

- [ ] **Step 1: Delete the three commented lines**

In `site/src/components/Button/index.tsx`, remove these three lines from the `clsx(...)` call:

```javascript
    // 'text-black',
    // getVariant(type, disabled),
    // block && 'w-full',
```

- [ ] **Step 2: Handle the now-unused destructured fields**

Removing `// getVariant(type, disabled),` and `// block && 'w-full',` leaves `type` and `block` destructured but unused at line 20. They must stay in the destructure — they are deliberately pulled out of `props` so they are NOT forwarded to the DOM via `...remainingProps` (React would warn about unknown attributes on `<button>`). Prefix them to say so:

```javascript
  const { block: _block, disabled, children, nomargin, type: _type, className, truetype, ...remainingProps } =
    props;
```

If ESLint's unused-vars rule does not accept the `_` prefix, add its `argsIgnorePattern`-equivalent inline disable instead — do not delete the fields.

- [ ] **Step 3: Verify lint and tests**

```bash
cd site
./node_modules/.bin/eslint src --report-unused-disable-directives --max-warnings 0
yarn test --run src/components/__tests__/Button.test.tsx
```

Expected: both exit 0. `Button.test.tsx` asserts the default and `PRIMARY` variants render identical class lists — still true, since `getVariant` was already not being called.

- [ ] **Step 4: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T41
git add -A && git commit -m "tidy(dead-code): delete commented-out clsx entries from Button [T41]

Dead since b3857ca (2025-10-04). block and type stay destructured so
they are not spread onto the DOM button element.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 6: Delete the now-unused getVariant export [T17]

**Files:**
- Modify: `site/src/components/Button/ButtonTypes.ts:10-21`

**Interfaces:**
- Consumes: Task 5 (which removed the last, commented-out reference).
- Produces: nothing.

**Note:** `ButtonTypes` (the enum) stays — it is imported by `Button/index.tsx` and `__tests__/Button.test.tsx`. Only the `getVariant` function goes.

- [ ] **Step 1: Confirm only the enum is used**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
git grep -n 'getVariant' -- site/
git grep -n 'ButtonTypes' -- site/ | head
```

Expected: `getVariant` returns no hits (Task 5 removed the last one). `ButtonTypes` returns several — leave it alone.

- [ ] **Step 2: Delete the function**

In `site/src/components/Button/ButtonTypes.ts`, delete the entire `export const getVariant = (...) => { ... };` block (lines 10-21), leaving the `ButtonTypes` enum above it.

- [ ] **Step 3: Verify**

```bash
cd site
yarn build
yarn test --run
./node_modules/.bin/eslint src --report-unused-disable-directives --max-warnings 0
```

Expected: all exit 0. `yarn build` runs `tsc` first, so a missed import would fail here.

- [ ] **Step 4: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T17
git add -A && git commit -m "tidy(dead-code): delete the unused getVariant helper [T17]

Its only call site was the commented-out line removed in T41. The
ButtonTypes enum it sits beside is still used and stays.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 7: Delete the never-imported BorderedTableCell component [T16]

**Files:**
- Delete: `site/src/components/BorderedTableCell.tsx`

**Interfaces:**
- Consumes: nothing. Produces: nothing.

- [ ] **Step 1: Confirm no importers**

```bash
git grep -n 'BorderedTableCell' -- site/
```

Expected: only `site/src/components/BorderedTableCell.tsx` itself.

- [ ] **Step 2: Delete the file**

```bash
git rm site/src/components/BorderedTableCell.tsx
```

- [ ] **Step 3: Verify**

```bash
cd site && yarn build && yarn test --run
```

Expected: both exit 0.

- [ ] **Step 4: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T16
git add -A && git commit -m "tidy(dead-code): delete the never-imported BorderedTableCell [T16]

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 8: Remove the unread content dependency from the debounced-save effect [T18]

**Files:**
- Modify: `site/src/components/DateEditor.tsx:68`

**Interfaces:**
- Consumes: nothing. Produces: nothing.

**Context:** The effect body at `site/src/components/DateEditor.tsx:55-68` reads `isMountedRef`, `hasInitialized`, `debouncedData`, `lastSentData`, `currentDateRef` and `date`. It never reads `content`, but `content` is in the dependency array — so every server refetch re-runs the save effect for nothing.

- [ ] **Step 1: Write a failing test**

Create `site/src/components/__tests__/DateEditor.deps.test.tsx`:

```tsx
import { render, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { DateEditor } from '../DateEditor';
import * as useDateDataModule from 'hooks/useDateData';

describe('DateEditor debounced save', () => {
  it('does not re-save when only content changes identity', async () => {
    const updater = vi.fn();
    const spy = vi.spyOn(useDateDataModule, 'useDateData');
    spy.mockReturnValue({ content: 'a', parsedData: null, updater });

    const date = new Date('2026-08-29T00:00:00');
    const { rerender } = render(<DateEditor date={date} />);
    await waitFor(() => expect(updater).not.toHaveBeenCalled());

    // Same content value, new object identity from a refetch.
    spy.mockReturnValue({ content: 'a', parsedData: null, updater });
    rerender(<DateEditor date={date} />);

    await new Promise((r) => setTimeout(r, 600));
    expect(updater).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run it against the unchanged code**

```bash
cd site && yarn test --run src/components/__tests__/DateEditor.deps.test.tsx
```

Record the result. If it already passes, the guard conditions in the effect body are masking the extra re-runs — that is fine; keep the test as a regression guard and note it in the commit body rather than claiming it was failing.

- [ ] **Step 3: Apply the fix**

In `site/src/components/DateEditor.tsx`, change the dependency array on line 68 from:

```javascript
  }, [debouncedData, updater, date, content, hasInitialized]);
```

to:

```javascript
  }, [debouncedData, updater, date, hasInitialized]);
```

- [ ] **Step 4: Verify**

```bash
cd site
yarn test --run
./node_modules/.bin/eslint src --report-unused-disable-directives --max-warnings 0
```

Expected: both exit 0. ESLint's `react-hooks/exhaustive-deps` must not complain — `content` is genuinely unread in the effect, so removing it is what the rule wants.

- [ ] **Step 5: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T18
git add -A && git commit -m "tidy(idioms): drop the unread content dep from the save effect [T18]

The effect never reads content, so listing it re-ran the debounced
save on every server refetch.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 9: Drop the inline style duplicating the textarea's Tailwind classes [T19]

**Files:**
- Modify: `site/src/components/DateEditor.tsx:73-77`

**Interfaces:**
- Consumes: nothing. Produces: nothing.

- [ ] **Step 1: Apply the fix**

In `site/src/components/DateEditor.tsx`, change the `<textarea>` from:

```tsx
      <textarea
        value={localData}
        className="w-1/2 p-2 border rounded mr-4 bg-gray-900 text-white"
        onChange={(e) => setLocalData(e.target.value)}
        style={{ width: '50%', height: '100%' }}
      />
```

to:

```tsx
      <textarea
        value={localData}
        className="w-1/2 h-full p-2 border rounded mr-4 bg-gray-900 text-white"
        onChange={(e) => setLocalData(e.target.value)}
      />
```

`w-1/2` already means `width: 50%`; `h-full` means `height: 100%` and is the idiom used in `site/src/page/PageTemplate.tsx`.

- [ ] **Step 2: Verify**

```bash
cd site
yarn build
yarn test --run
./node_modules/.bin/eslint src --report-unused-disable-directives --max-warnings 0
```

Expected: all exit 0.

- [ ] **Step 3: Visually confirm the editor still fills its pane**

```bash
cd site && yarn dev
```

Open the printed URL, navigate to `/editor`, and confirm the textarea still occupies the left half at full height. Stop the dev server when done. If the height collapses, the parent lacks a definite height — revert to keeping `height: '100%'` inline and note it in the commit.

- [ ] **Step 4: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T19
git add -A && git commit -m "tidy(idioms): replace the textarea's inline style with h-full [T19]

w-1/2 already sets width 50%; h-full is the height idiom used in
PageTemplate.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

**MILESTONE after Task 9:** run the full suite.

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
export SKIP_YARN=1 && cargo test --workspace
cd site && yarn build && yarn test --run
```

Both must be green before Wave 2. On red: bisect within Tasks 2-9, revert the offender, surface the diagnosis.

---

## Wave 2 — Shared helpers

### Task 10: Add a shared local-date module [T42]

**Files:**
- Create: `site/src/utils/date.ts`
- Create: `site/src/utils/__tests__/date.test.ts`
- Modify: `site/src/hooks/useDateData.ts:11`, `site/src/hooks/useWeekData.ts:6`, `site/src/components/DateSelector.tsx:23`, `site/src/page/DateEditorPage.tsx:10,20`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `toDateString(date: Date): string` — formats to `YYYY-MM-DD` from **local** components.
  - `parseDateString(value: string): Date` — parses `YYYY-MM-DD` at **local** midnight.
  - `todayDateString(): string` — `toDateString(new Date())`.

  Tasks 12, 13 and 14 all call these. **This task must land before them.**

**CRITICAL — this is the whole point of the finding.** The five existing call sites use `date.toISOString().split('T')[0]`, which formats the **UTC** calendar day. Extracting that verbatim into a shared helper would enshrine the bug that T2/T4/T5 exist to fix. The helper must format from local components.

There is a second half the original finding did not name: `new Date("2026-08-29")` parses as **UTC** midnight, so in a negative-offset zone it is already the previous local day. `DateEditorPage.tsx:10` and `WeeklySummaryPage.tsx:8` both do this. Hence `parseDateString`, which mirrors the `+ 'T00:00:00'` trick already used deliberately at `site/src/components/WeeklySummary.tsx:85`.

- [ ] **Step 1: Write the failing tests**

Create `site/src/utils/__tests__/date.test.ts`:

```typescript
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { parseDateString, todayDateString, toDateString } from '../date';

describe('toDateString', () => {
  it('formats from local components, not UTC', () => {
    // 2026-08-29 23:30 local. In any negative UTC offset this instant is
    // already 2026-08-30 in UTC, which is exactly the bug.
    const d = new Date(2026, 7, 29, 23, 30, 0);
    expect(toDateString(d)).toBe('2026-08-29');
  });

  it('zero-pads single-digit months and days', () => {
    expect(toDateString(new Date(2026, 0, 5))).toBe('2026-01-05');
  });

  it('round-trips with parseDateString', () => {
    expect(toDateString(parseDateString('2026-03-07'))).toBe('2026-03-07');
  });
});

describe('parseDateString', () => {
  it('parses at local midnight, not UTC midnight', () => {
    const d = parseDateString('2026-08-29');
    expect(d.getFullYear()).toBe(2026);
    expect(d.getMonth()).toBe(7);
    expect(d.getDate()).toBe(29);
    expect(d.getHours()).toBe(0);
  });
});

describe('todayDateString', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('uses the local calendar day at a late-evening instant', () => {
    vi.setSystemTime(new Date(2026, 7, 29, 23, 45, 0));
    expect(todayDateString()).toBe('2026-08-29');
  });
});
```

- [ ] **Step 2: Run to verify they fail**

```bash
cd site && yarn test --run src/utils/__tests__/date.test.ts
```

Expected: FAIL — cannot resolve `../date`.

- [ ] **Step 3: Write the module**

Create `site/src/utils/date.ts`:

```typescript
/**
 * Date-string helpers for the `YYYY-MM-DD` boundary between `Date` objects
 * and the GraphQL API.
 *
 * Both directions work in **local** time on purpose. `toISOString()` formats
 * the UTC calendar day, so at 23:30 in any negative UTC offset it reports
 * tomorrow; `new Date('2026-08-29')` parses as UTC midnight, which in the
 * same zone is yesterday evening. A day file is named for the user's local
 * day, so both of those are off by one.
 */

/** Format `date` as `YYYY-MM-DD` using its local calendar day. */
export const toDateString = (date: Date): string => {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
};

/** Parse a `YYYY-MM-DD` string as local midnight on that calendar day. */
export const parseDateString = (value: string): Date => new Date(`${value}T00:00:00`);

/** The current local calendar day as `YYYY-MM-DD`. */
export const todayDateString = (): string => toDateString(new Date());
```

- [ ] **Step 4: Run to verify they pass**

```bash
cd site && yarn test --run src/utils/__tests__/date.test.ts
```

Expected: PASS, 5 tests.

- [ ] **Step 5: Migrate the five call sites**

`site/src/hooks/useDateData.ts` — add `import { toDateString } from 'utils/date';` and change line 11:

```typescript
  const dateString = toDateString(date);
```

`site/src/hooks/useWeekData.ts` — add the same import and change the variables line:

```typescript
    variables: { date: toDateString(date) },
```

`site/src/components/DateSelector.tsx` — add the import and change line 23:

```typescript
  const formatDate = (d: Date) => toDateString(d);
```

`site/src/page/DateEditorPage.tsx` — add `import { parseDateString, todayDateString, toDateString } from 'utils/date';` and change lines 9-12 and 20:

```typescript
  const dateObject = useMemo(
    () => parseDateString(date || todayDateString()),
    [date],
  );
```

```typescript
  const currentDateString = toDateString(dateObject);
```

Check whether `utils/date` resolves as a bare specifier — `site/vite.config.ts` uses `vite-tsconfig-paths` but `site/tsconfig.json:4-8` has `paths` commented out, while existing code already imports `components/Button` and `hooks/useDateData` bare. If `utils/date` fails to resolve, use a relative import (`../utils/date`) consistent with what the neighbouring file does.

- [ ] **Step 6: Verify the whole frontend**

```bash
cd site
yarn build
yarn test --run
./node_modules/.bin/eslint src --report-unused-disable-directives --max-warnings 0
```

Expected: all exit 0.

- [ ] **Step 7: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T42
git add -A && git commit -m "tidy(duplication): add a shared local-date module [T42]

Replaces five copies of date.toISOString().split('T')[0]. The helper
formats and parses in local time on purpose: toISOString reports the
UTC calendar day, and new Date('YYYY-MM-DD') parses as UTC midnight,
so both are off by one for evening users behind UTC. T2, T4 and T5
build on this.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 11: Extract the shared clipboard-with-toast helper [T20]

**Files:**
- Create: `site/src/utils/clipboard.ts`
- Create: `site/src/utils/__tests__/clipboard.test.ts`
- Modify: `site/src/components/DateSummary.tsx:22-43`, `site/src/components/WeeklySummary.tsx:125-145`

**Interfaces:**
- Consumes: nothing.
- Produces: `copyNotesToClipboard(notes: string[], successMessage: string): Promise<void>`. Task 25 relies on this existing so the extracted `useNotesLookup` hook calls it rather than carrying a copy.

**Context:** `DateSummary.tsx:22-43` and `WeeklySummary.tsx:125-145` both join notes as `- ${note}` lines, `await navigator.clipboard.writeText(...)`, then raise a success or error toast with the same options. The callers keep their own concerns: `DateSummary`'s empty-notes early return and `WeeklySummary`'s "No notes for this day" tooltip fallback stay at the call sites.

- [ ] **Step 1: Write the failing test**

Create `site/src/utils/__tests__/clipboard.test.ts`:

```typescript
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from 'react-toastify';
import { copyNotesToClipboard } from '../clipboard';

vi.mock('react-toastify', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

describe('copyNotesToClipboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
  });

  it('writes notes as a dash-prefixed list', async () => {
    await copyNotesToClipboard(['first', 'second'], 'Copied!');
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('- first\n- second');
  });

  it('raises a success toast with the given message', async () => {
    await copyNotesToClipboard(['a'], 'Project X notes copied!');
    expect(toast.success).toHaveBeenCalledWith('Project X notes copied!', expect.any(Object));
  });

  it('raises an error toast when the clipboard write rejects', async () => {
    Object.assign(navigator, {
      clipboard: { writeText: vi.fn().mockRejectedValue(new Error('denied')) },
    });
    await copyNotesToClipboard(['a'], 'nope');
    expect(toast.error).toHaveBeenCalledWith('Failed to copy to clipboard', expect.any(Object));
    expect(toast.success).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd site && yarn test --run src/utils/__tests__/clipboard.test.ts
```

Expected: FAIL — cannot resolve `../clipboard`.

- [ ] **Step 3: Write the module**

Create `site/src/utils/clipboard.ts`:

```typescript
import { toast } from 'react-toastify';

/**
 * Copy `notes` to the clipboard as a dash-prefixed list and report the
 * outcome with a toast. Callers own the decision of whether there is
 * anything worth copying.
 */
export const copyNotesToClipboard = async (
  notes: string[],
  successMessage: string,
): Promise<void> => {
  const formattedNotes = notes.map((note) => `- ${note}`).join('\n');

  try {
    await navigator.clipboard.writeText(formattedNotes);
    toast.success(successMessage, {
      position: 'top-right',
      autoClose: 2000,
      hideProgressBar: false,
      closeOnClick: true,
      pauseOnHover: true,
      draggable: true,
    });
  } catch {
    toast.error('Failed to copy to clipboard', {
      position: 'top-right',
      autoClose: 2000,
    });
  }
};
```

- [ ] **Step 4: Run to verify it passes**

```bash
cd site && yarn test --run src/utils/__tests__/clipboard.test.ts
```

Expected: PASS, 3 tests.

- [ ] **Step 5: Migrate DateSummary**

In `site/src/components/DateSummary.tsx`, add `import { copyNotesToClipboard } from 'utils/clipboard';`, drop the now-unused `toast` import if nothing else in the file uses it, and replace the whole `copyProjectNotesToClipboard` body (lines 22-43) with:

```typescript
  const copyProjectNotesToClipboard = async (projectName: string, notes: string[]) => {
    if (notes.length === 0) return;
    await copyNotesToClipboard(notes, `${projectName} notes copied to clipboard!`);
  };
```

- [ ] **Step 6: Migrate WeeklySummary**

In `site/src/components/WeeklySummary.tsx`, add the same import and replace the body of its `copyNotesToClipboard` (lines 125-145) with a call to the shared helper, preserving its own success-message wording. Do not touch `formatNotesTooltip` (lines 119-124) — that is tooltip text, a separate concern. If the local function name now shadows the import, rename the local one to `copyDayNotes`.

- [ ] **Step 7: Verify**

```bash
cd site
yarn build
yarn test --run
./node_modules/.bin/eslint src --report-unused-disable-directives --max-warnings 0
```

Expected: all exit 0.

- [ ] **Step 8: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T20
git add -A && git commit -m "tidy(duplication): extract the shared clipboard-with-toast helper [T20]

DateSummary and WeeklySummary hand-rolled the same notes-join,
clipboard-write and toast sequence. Callers keep their own empty-notes
and tooltip-fallback handling.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

---

## Wave 3 — The UTC-vs-local date bugs

All three are `risk: high` and need characterization tests first. All three consume Task 10's helpers.

**How to test a timezone bug:** Vitest reads the `TZ` environment variable. Run these tests under a fixed negative-offset zone so the bug is deterministic rather than dependent on the developer's machine:

```bash
cd site && TZ=America/New_York yarn test --run <file>
```

Combine with `vi.setSystemTime(new Date(2026, 7, 29, 23, 30))` — 23:30 local on 2026-08-29 in `America/New_York` is 03:30 UTC on 2026-08-30, so UTC-based code reports `2026-08-30` and local-based code reports `2026-08-29`.

### Task 12: Format the date picker's value from local components [T2]

**Files:**
- Modify: `site/src/components/DateSelector.tsx`
- Create: `site/src/components/__tests__/DateSelector.test.tsx`

**Interfaces:**
- Consumes: `toDateString` from Task 10.
- Produces: nothing.

**Note:** Task 10 Step 5 already rewrote `formatDate` to delegate to `toDateString`, which fixes the underlying defect. This task's job is to **pin that behavior with a test** and collapse the now-pointless one-line indirection.

- [ ] **Step 1: Write the characterization test**

Create `site/src/components/__tests__/DateSelector.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import DateSelector from '../DateSelector';

describe('DateSelector in a negative UTC offset', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('links to the local calendar day late in the evening', () => {
    // 23:30 local on 2026-08-29 is already 2026-08-30 in UTC.
    vi.setSystemTime(new Date(2026, 7, 29, 23, 30, 0));
    render(
      <MemoryRouter>
        <DateSelector date={new Date(2026, 7, 29, 23, 30, 0)} linkBase="/editor" />
      </MemoryRouter>,
    );
    const todayLink = screen.getByRole('link', { name: /today/i });
    expect(todayLink).toHaveAttribute('href', '/editor/2026-08-29');
  });
});
```

Adjust the query for the actual markup — read `site/src/components/DateSelector.tsx` first and match how the "Today" control is rendered (it may be a `Link` wrapping a `Button`, so `getByRole('link')` may need a different accessible name).

- [ ] **Step 2: Run it**

```bash
cd site && TZ=America/New_York yarn test --run src/components/__tests__/DateSelector.test.tsx
```

Expected: PASS, because Task 10 already fixed `formatDate`. To confirm the test actually discriminates, temporarily revert `formatDate` to `d.toISOString().split('T')[0]`, re-run, see it FAIL with `/editor/2026-08-30`, then restore.

- [ ] **Step 3: Collapse the one-line indirection**

In `site/src/components/DateSelector.tsx`, delete the `formatDate` wrapper and its stale comment, calling `toDateString` directly at its use sites:

```typescript
  // (delete) // Helper function to format date for URL and input
  // (delete) const formatDate = (d: Date) => toDateString(d);
```

Replace each `formatDate(x)` with `toDateString(x)`.

- [ ] **Step 4: Verify**

```bash
cd site
TZ=America/New_York yarn test --run
yarn build
./node_modules/.bin/eslint src --report-unused-disable-directives --max-warnings 0
```

Expected: all exit 0.

- [ ] **Step 5: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T2
git add -A && git commit -m "tidy(opportunistic): pin DateSelector to the local calendar day [T2]

toISOString formatted the UTC day, so the Today button and the current
date display jumped to tomorrow for evening users behind UTC. T42's
helper fixed the formatting; this adds the regression test and drops
the leftover one-line wrapper.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 13: Default the editor route's date from local components [T4]

**Files:**
- Modify: `site/src/page/DateEditorPage.tsx`
- Create: `site/src/page/__tests__/DateEditorPage.test.tsx`

**Interfaces:**
- Consumes: `parseDateString`, `todayDateString`, `toDateString` from Task 10.
- Produces: nothing.

**Note:** As with Task 12, Task 10 Step 5 already applied the fix. This task pins it.

- [ ] **Step 1: Write the characterization test**

Create `site/src/page/__tests__/DateEditorPage.test.tsx`. Mock `hooks/useDateData` so no Apollo provider is needed, render under `MemoryRouter` with no `:date` param, and assert the heading/link reflect `2026-08-29` at a system time of `new Date(2026, 7, 29, 23, 30)`:

```tsx
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('hooks/useDateData', () => ({
  useDateData: () => ({ content: '', parsedData: null, updater: vi.fn() }),
}));

import DateEditorPage from '../DateEditorPage';

describe('DateEditorPage default date', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('defaults to the local calendar day, not the UTC one', () => {
    vi.setSystemTime(new Date(2026, 7, 29, 23, 30, 0));
    render(
      <MemoryRouter>
        <DateEditorPage />
      </MemoryRouter>,
    );
    expect(screen.getByRole('link', { name: /weekly summary/i })).toHaveAttribute(
      'href',
      '/weekly-summary/2026-08-29',
    );
  });
});
```

- [ ] **Step 2: Run under a negative-offset zone**

```bash
cd site && TZ=America/New_York yarn test --run src/page/__tests__/DateEditorPage.test.tsx
```

Expected: PASS. To confirm it discriminates, temporarily restore `new Date(date || new Date().toISOString().split('T')[0])`, re-run, see `/weekly-summary/2026-08-30`, then restore the fix.

- [ ] **Step 3: Confirm line 20 also uses the helper**

Re-read `site/src/page/DateEditorPage.tsx` and confirm `currentDateString` is `toDateString(dateObject)` and no bare `.toISOString()` remains:

```bash
grep -n 'toISOString' site/src/page/DateEditorPage.tsx
```

Expected: no output.

- [ ] **Step 4: Verify**

```bash
cd site && TZ=America/New_York yarn test --run && yarn build
```

Expected: both exit 0.

- [ ] **Step 5: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T4
git add -A && git commit -m "tidy(opportunistic): default the editor route to the local day [T4]

Landing on /editor with no :date param used toISOString, opening
tomorrow's file for evening users behind UTC. Also fixes the UTC
midnight parse of an explicit :date param.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 14: Default the weekly route's week from a local date [T5]

**Files:**
- Modify: `site/src/page/WeeklySummaryPage.tsx:8`
- Create: `site/src/page/__tests__/WeeklySummaryPage.test.tsx`

**Interfaces:**
- Consumes: `parseDateString` from Task 10.
- Produces: nothing.

**Context:** `WeeklySummaryPage.tsx:8` reads `const date = inputDate ? new Date(inputDate) : new Date();`. Both branches are wrong: `new Date(inputDate)` parses `YYYY-MM-DD` as UTC midnight (previous local day in a negative offset), and the bare `new Date()` is then formatted by `useWeekData`. Task 10 fixed the formatting half; the parse half is still here.

- [ ] **Step 1: Write the characterization test**

Create `site/src/page/__tests__/WeeklySummaryPage.test.tsx`, mocking `hooks/useWeekData` and capturing the `Date` it receives:

```tsx
import { render } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const seen: Date[] = [];
vi.mock('hooks/useWeekData', () => ({
  useWeekData: (d: Date) => {
    seen.push(d);
    return [undefined];
  },
}));

import Homepage from '../WeeklySummaryPage';

describe('WeeklySummaryPage default week', () => {
  beforeEach(() => {
    seen.length = 0;
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  it('passes the local calendar day to useWeekData', () => {
    vi.setSystemTime(new Date(2026, 7, 29, 23, 30, 0));
    render(
      <MemoryRouter>
        <Homepage />
      </MemoryRouter>,
    );
    expect(seen[0].getDate()).toBe(29);
    expect(seen[0].getMonth()).toBe(7);
  });
});
```

- [ ] **Step 2: Run under a negative-offset zone**

```bash
cd site && TZ=America/New_York yarn test --run src/page/__tests__/WeeklySummaryPage.test.tsx
```

Record the result — this one may genuinely fail on the `inputDate` branch.

- [ ] **Step 3: Apply the fix**

In `site/src/page/WeeklySummaryPage.tsx`, add `import { parseDateString } from 'utils/date';` and change line 8:

```typescript
  const date = inputDate ? parseDateString(inputDate) : new Date();
```

- [ ] **Step 4: Verify**

```bash
cd site && TZ=America/New_York yarn test --run && yarn build
```

Expected: both exit 0.

- [ ] **Step 5: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T5
git add -A && git commit -m "tidy(opportunistic): parse the weekly route's date locally [T5]

new Date('YYYY-MM-DD') parses as UTC midnight, landing on the previous
local day behind UTC and so showing the wrong week.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

**MILESTONE after Task 14:** run the full suite in two timezones.

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
export SKIP_YARN=1 && cargo test --workspace
cd site && yarn build
TZ=America/New_York yarn test --run
TZ=UTC yarn test --run
```

All must be green. The two-timezone run matters: a date test that passes only in the developer's own zone is not a regression guard.

---

## Wave 4 — The plain-formatter output bug

### Task 15: Fix the plain formatter's 40-line dash rule [T8]

**Files:**
- Modify: `src/display/plain.rs:60`
- Modify: `cli/tests/golden/weekly_plain.txt` (regenerated)
- Possibly modify: `cli/tests/golden/weekly_with_warnings_plain.txt`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing.

**Context:** `src/display/plain.rs:60` is `msg.push_str(&"-\n".repeat(40));` — 40 lines each containing a single dash. `src/display/default.rs:60` is `msg.push_str(&format!("{}\n", "-".repeat(40)));` — one 40-column rule. The plain output has been wrong, and `cli/tests/golden/weekly_plain.txt` currently pins the broken output, so the golden must be regenerated in this same commit.

**Scope note:** the finding also proposed extending `DaySummaryStyle` to share banner rendering between the default and plain formatters. **Do the bug fix only.** The sharing refactor touches `weekly_header`, `daily_breakdowns_header` and `day_header` across two files and is a separate change with its own risk; landing it here would bury a one-character behavior fix inside a refactor diff. If you want it, re-file it as a new finding.

- [ ] **Step 1: Confirm the broken golden**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
sed -n '1,15p' cli/tests/golden/weekly_plain.txt
```

Expected: a `WEEKLY TOTALS` line followed by many lines each containing exactly one `-`.

- [ ] **Step 2: Apply the fix**

In `src/display/plain.rs`, change line 60 from:

```rust
        msg.push_str(&"-\n".repeat(40));
```

to:

```rust
        msg.push_str(&format!("{}\n", "-".repeat(40)));
```

- [ ] **Step 3: Watch the golden test fail**

```bash
export SKIP_YARN=1
cargo test -p cli --test cli_output_characterization
```

Expected: FAIL on the plain-formatter golden. This is the proof the fix changed output.

- [ ] **Step 4: Regenerate the goldens**

```bash
BLESS_GOLDEN=1 cargo test -p cli --test cli_output_characterization
```

- [ ] **Step 5: Inspect the golden diff — this is the load-bearing check**

```bash
git diff --stat cli/tests/golden/
git diff cli/tests/golden/ | head -60
```

Expected: **only** the plain goldens changed, and only by collapsing runs of single-dash lines into one 40-dash line. `weekly_default.txt` and `weekly_markdown.txt` must be byte-unchanged. If any other golden moved, the fix had a side effect — stop and diagnose before committing.

- [ ] **Step 6: Verify**

```bash
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
```

Expected: all exit 0.

- [ ] **Step 7: Strip and commit**

```bash
todo-parser TIDY.md --strip T8
git add -A && git commit -m "tidy(duplication): render one dash rule, not 40 dash lines [T8]

plain.rs built its weekly-totals separator with \"-\\n\".repeat(40),
emitting 40 lines of one dash where default.rs emits a single
40-column rule. The golden pinned the broken output and is
regenerated here.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

---

## Wave 5 — Rust refactors

### Task 16: Split Config::load into four phases [T6]

**Files:**
- Modify: `src/config.rs:248-358`
- Modify: `src/config.rs` (in-file `#[cfg(test)] mod tests`, new tests appended)

**Interfaces:**
- Consumes: nothing.
- Produces: `Config::load` keeps its signature. New private helpers: `fn synthetic_args() -> Args`, `fn load_or_create_config_file(config_path: &Path) -> Result<Config>`, `fn apply_arg_overrides(config: &mut Config, args: &Args)`, `fn resolve_requested_date(date_str: Option<String>) -> Date`. Task 17 modifies `Config::default`, which this task does not touch.

**Risk: high — needs characterization tests first.**

**Run before Task 17.** Both touch `src/config.rs`; this one moves the larger blocks.

- [ ] **Step 1: Write characterization tests for the current behavior**

Append to the existing `#[cfg(test)] mod tests` in `src/config.rs` — do not modify existing tests. Cover, at minimum: a config file that does not exist gets created with defaults; an existing config file is read rather than overwritten; `args.date` of `"today"`, an explicit `YYYY-MM-DD`, and a relative phrase each resolve as they do today; and each `Option` arg override actually overrides its config-file value.

Use `tempfile::tempdir()` and point the config path at it. Read the surrounding test module first for the established fixture style.

- [ ] **Step 2: Confirm they pass on the unchanged code**

```bash
export SKIP_YARN=1
cargo test --workspace config
```

Expected: PASS. These pin current behavior — if any fails now, you have found a real bug; stop and report rather than "fixing" it as part of a refactor.

- [ ] **Step 3: Commit the characterization tests alone**

```bash
git add -A && git commit -m "test: characterize Config::load before tidy [T6]

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

- [ ] **Step 4: Extract the four helpers**

Working in `src/config.rs`, in this order (each extraction is mechanical — move the block, thread the parameters, leave behavior identical):

1. `fn synthetic_args() -> Args` — the `else`-branch default `Args` construction (lines 251-269).
2. `fn load_or_create_config_file(config_path: &Path) -> Result<Config>` — the read-or-write-default block (lines 271-290).
3. `fn resolve_requested_date(date_str: Option<String>) -> Date` — the `interim`-based date parsing (lines 316-341).
4. `fn apply_arg_overrides(config: &mut Config, args: &Args)` — the run of `if let Some(...) = args.X` assignments (lines 292-355, excluding the date block now owned by `resolve_requested_date`).

`Config::load` then reads as four calls. Keep every helper private (`fn`, not `pub fn`) — the library has an out-of-repo consumer and this batch adds no public API.

- [ ] **Step 5: Verify behavior is unchanged**

```bash
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
cargo test --workspace --no-default-features --features tui
cargo test --workspace --no-default-features --features webapp
```

Expected: all green. The characterization tests from Step 1 are the ones that matter here.

- [ ] **Step 6: Strip and commit**

```bash
todo-parser TIDY.md --strip T6
git add -A && git commit -m "tidy(long-methods): split Config::load into four phases [T6]

111 lines of arg synthesis, file load-or-create, field overrides and
date parsing become four private helpers. Behavior pinned by the
characterization tests committed just before.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 17: Stop panicking in Config::default when there is no home directory [T21]

**Files:**
- Modify: `src/config.rs:205-211`

**Interfaces:**
- Consumes: Task 16 (which reorganised the surrounding function).
- Produces: `Config::default().data_directory` is now `None` rather than an eagerly-resolved path.

**Context:** `Config::default()` currently does `data_directory: Some(get_time_tracking_dir_with_override(None).unwrap().display().to_string())`. That `unwrap()` panics when `dirs::home_dir()` returns `None` — a container with no `$HOME`. Every consumer of `data_directory` already handles `None` by lazily re-resolving through a fallible path, so the eager resolution buys nothing.

- [ ] **Step 1: Write the failing test**

Append to `src/config.rs`'s test module:

```rust
#[test]
fn default_config_does_not_resolve_the_home_directory() {
    // Must not panic even when the home directory cannot be resolved;
    // consumers re-resolve data_directory lazily through a Result.
    let config = Config::default();
    assert!(
        config.data_directory.is_none(),
        "Config::default must not eagerly resolve a data directory"
    );
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
export SKIP_YARN=1
cargo test --workspace default_config_does_not_resolve
```

Expected: FAIL — `data_directory` is currently `Some(...)`.

- [ ] **Step 3: Apply the fix**

In `src/config.rs`, replace the `data_directory` field in the `Default` impl with:

```rust
            // Left unresolved on purpose: every consumer re-derives this
            // through get_data_directory(), which surfaces a Result instead
            // of panicking when there is no home directory to find.
            data_directory: None,
```

- [ ] **Step 4: Run to verify it passes**

```bash
cargo test --workspace
```

Expected: PASS. Pay attention to any existing test that asserted a concrete default `data_directory` — if one fails, it was asserting the eager behavior; read it before deciding, and if it genuinely pins the old contract, stop and surface it rather than editing the test.

- [ ] **Step 5: Verify the CLI still resolves a directory end to end**

```bash
TMP=$(mktemp -d)
cargo run -p cli -- --noedit --data-directory "$TMP" --week
```

Expected: runs and prints a weekly summary against the empty temp directory. **Never omit `--noedit --data-directory`.**

- [ ] **Step 6: Strip and commit**

```bash
cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all
todo-parser TIDY.md --strip T21
git add -A && git commit -m "tidy(idioms): stop resolving the home directory in Config::default [T21]

The unwrap panicked where dirs::home_dir() returns None, e.g. a
container with no HOME. Consumers already re-resolve lazily through a
Result, so the eager resolution only added a panic path.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 18: Replace the template-write race with an atomic create [T24]

**Files:**
- Modify: `src/data_svc.rs:361-372`

**Interfaces:**
- Consumes: nothing.
- Produces: `create_day_file_if_not_exists` keeps its signature.

**Context:** `create_day_file_if_not_exists` checks `file_path.exists()` and then `fs::write`s the template. Between the two, another process (a second `ttcli`, the TUI, the web server) can create and populate the file — and the `fs::write` then truncates it back to the empty template. That is data loss on the user's real time entries.

- [ ] **Step 1: Write the failing test**

Append to `src/data_svc.rs`'s `#[cfg(test)] mod tests`:

```rust
#[tokio::test]
async fn create_day_file_does_not_clobber_content_written_after_the_exists_check() {
    let dir = tempfile::tempdir().expect("tempdir");
    let svc = DataService::new_with_dir(dir.path().to_path_buf());
    let date = Date::from_calendar_date(2026, time::Month::August, 29).expect("date");

    // Simulate the racing writer having already won: the file exists with
    // real content by the time the template write would land.
    let path = svc.get_file_path(date).await.expect("path");
    tokio::fs::write(&path, "real user content\n").await.expect("seed");

    svc.create_day_file_if_not_exists(&date).await.expect("create");

    let after = tokio::fs::read_to_string(&path).await.expect("read");
    assert_eq!(after, "real user content\n", "template write clobbered real content");
}
```

Match `DataService`'s actual constructor and `get_file_path` signature — read the surrounding test module first and adapt.

- [ ] **Step 2: Run it**

```bash
export SKIP_YARN=1
cargo test --workspace create_day_file_does_not_clobber
```

Expected: PASS on the current code for this narrow shape (the `exists()` check catches it). The genuine race needs the write to land *between* check and write, which a test cannot reliably schedule. Keep this test as the regression guard, and rely on the atomic primitive below to close the window the test cannot reach. Note this honestly in the commit body — do not claim the test proves the race is fixed.

- [ ] **Step 3: Apply the fix**

In `src/data_svc.rs`, replace the `if !file_path.exists() { ... }` block with an atomic create-only open:

```rust
        // create_new is atomic: it either creates the file or fails with
        // AlreadyExists. An exists()-then-write pair leaves a window in
        // which another writer (a second ttcli, the TUI, the web server)
        // creates and fills the file, and our template write then truncates
        // their content back to empty.
        let template_content =
            create_template_content(date, self.parse_opts.template_file()).await?;
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file_path)
            .await
        {
            Ok(mut file) => {
                use tokio::io::AsyncWriteExt as _;
                file.write_all(template_content.as_bytes()).await?;
                self.invalidate_date(date).await;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Someone else got there first; their content stands.
            }
            Err(e) => return Err(e.into()),
        }
```

Note this now builds the template content before knowing whether it is needed. If `create_template_content` is expensive or has side effects, move it inside the `Ok(...)` arm instead — check what it does first.

- [ ] **Step 4: Verify**

```bash
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
```

Expected: all green.

- [ ] **Step 5: Strip and commit**

```bash
todo-parser TIDY.md --strip T24
git add -A && git commit -m "tidy(opportunistic): create the day file atomically [T24]

exists()-then-write left a window where a concurrent writer's real
content was truncated back to the empty template. create_new closes
it. The added test guards the narrow shape a test can schedule; the
atomic open is what closes the actual race.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 19: Split get_weekly_summary into load, fold and finalize [T25]

**Files:**
- Modify: `src/data_svc.rs:505-591`

**Interfaces:**
- Consumes: nothing.
- Produces: `get_weekly_summary` keeps its signature and its ordering guarantee (projects sorted minutes-descending, then name-ascending). New private helpers as described below.

**Context:** 87 lines mixing a concurrent `JoinSet` load, a sequential per-day fold, and a final sort/collect. Note `get_weekly_data` (line 597) is a thin projection of this function — leave it alone, it is `pub` and this batch deletes no public items.

- [ ] **Step 1: Confirm existing coverage before refactoring**

```bash
export SKIP_YARN=1
cargo test --workspace weekly
```

Expected: the existing `data_svc` weekly tests pass. This function already has coverage (`risk: low` in triage) — those tests are the safety net. Read them so you know what invariants they pin, especially the project ordering.

- [ ] **Step 2: Extract the three phases**

In `src/data_svc.rs`, extract in this order:

1. The `JoinSet` spawn + collect + reorder (lines 506-528) into an async helper returning the per-day loads in `dates` order.
2. The per-day accumulation (lines 531-563) into a fold helper taking `&mut WeeklySummary` and the project-rollup map.
3. The sort/collect (lines 566-577) into a finalize helper returning `Vec<WeeklyProject>`.

Keep all three private. Preserve the exact sort comparator — minutes descending, then name ascending — it is pinned by tests and by the CLI goldens.

- [ ] **Step 3: Verify behavior is unchanged**

```bash
cargo test --workspace
cargo test -p cli --test cli_output_characterization
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
```

Expected: all green, and **no golden file changes**:

```bash
git status --porcelain cli/tests/golden/
```

Expected: no output. A golden change here means the refactor altered output — revert and diagnose.

- [ ] **Step 4: Strip and commit**

```bash
todo-parser TIDY.md --strip T25
git add -A && git commit -m "tidy(long-methods): split get_weekly_summary into three phases [T25]

87 lines of concurrent load, sequential fold and final sort become
three private helpers. Project ordering (minutes desc, then name asc)
is unchanged and still pinned by the CLI goldens.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 20: Stop cloning the whole CacheEntry to check its metadata [T27]

**Files:**
- Modify: `src/data_svc.rs:613-643`

**Interfaces:**
- Consumes: nothing.
- Produces: `get_valid_entry` keeps its signature and semantics. `get_cached_content` and `get_cached_parsed` continue to build on it.

**Context:** `get_valid_entry` clones the entire `CacheEntry` — raw file text plus parsed data — under the lock, then uses only `cached_at` and `file_mod_time` (both `Copy`) to decide validity. On a documented ~97-calls-per-navigation hot path, that is a full copy of the day's content per call, valid or not.

**Preserve the mtime comparison exactly.** The `file_mod_time == cached_mod_time` equality (not `<=`) is deliberate and carries a long comment explaining why: a restore from backup, `git checkout`, `cp -p`, or clock skew all move mtime backwards, and `<=` misses every one. Do not "simplify" it.

- [ ] **Step 1: Confirm existing cache tests pass**

```bash
export SKIP_YARN=1
cargo test --workspace cache
```

Expected: green. `src/data_svc.rs` has a `parse_count` test helper that proves memoization by counting real parses — those tests are the safety net for this change.

- [ ] **Step 2: Apply the fix**

Restructure `get_valid_entry` so the first lock scope copies only the `Copy` metadata:

```rust
        // Copy only the Copy metadata under the lock. The entry itself holds
        // the day's raw text and its parsed form; cloning that per call was
        // a full copy of the day's content on a path that runs ~97 times per
        // navigation, whether or not the entry turned out to be valid.
        let meta = {
            let cache = self.cache.lock().await;
            cache.get(date).map(|e| (e.cached_at, e.file_mod_time))
        };
```

Then run the existing validity checks against `meta`, and only when they all pass re-acquire the lock to clone the entry for return. Keep the mtime equality comment verbatim.

- [ ] **Step 3: Verify**

```bash
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
```

Expected: all green. The memoization tests must still pass — if `parse_count` rises, the second lock acquisition is missing an entry that the first pass considered valid.

- [ ] **Step 4: Note the re-check subtlety in the commit**

Between releasing the lock and re-acquiring it, another task can invalidate the entry. Re-fetch inside the second scope and return `Ok(None)` if it is gone, rather than unwrapping. State this in the commit body.

- [ ] **Step 5: Strip and commit**

```bash
todo-parser TIDY.md --strip T27
git add -A && git commit -m "tidy(opportunistic): stop cloning CacheEntry to read Copy metadata [T27]

get_valid_entry cloned the day's raw text and parsed form just to
compare cached_at and file_mod_time, on a ~97-calls-per-navigation
path. Now copies only the metadata, and re-checks the entry under the
second lock in case it was invalidated in between.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

### Task 21: Split main_impl and de-duplicate its report dispatch [T11]

**Files:**
- Modify: `cli/src/main.rs:14-116`

**Interfaces:**
- Consumes: nothing.
- Produces: `main_impl` keeps its signature. New private helpers as described below.

**Context:** 103 lines of `cfg`-gated setup in which the weekly/single-day report block (lines 82-88) is duplicated verbatim in the `#[cfg(not(feature = "tui"))]` arm (lines 94-100). The doc-comment commit `edd4ae8` already deleted four redundant comments in this function and moved one, so line numbers have shifted — re-read the file before editing.

- [ ] **Step 1: Re-read the current state of the function**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
cat -n cli/src/main.rs
```

Note where the two copies of the report dispatch now sit.

- [ ] **Step 2: Extract the duplicated dispatch first**

Add:

```rust
/// Print the requested report: the week containing `config.date`, or that
/// single day. The two call sites differ only in whether the `tui` feature
/// is compiled in.
async fn show_report(config: &Config, week_start_weekday: Weekday) -> anyhow::Result<()> {
    let formatter = get_formatter(config);
    if config.week {
        show_weekly_summary(&config.date, week_start_weekday, formatter.as_ref()).await?;
    } else {
        show_single_day(&config.date, formatter.as_ref(), config.noedit).await?;
    }
    Ok(())
}
```

Match the real names and the way `formatter` is currently obtained — read the surrounding code rather than trusting this sketch. Replace both copies with `show_report(&config, week_start_weekday).await?;`.

- [ ] **Step 3: Verify the deduplication alone**

```bash
export SKIP_YARN=1
cargo check --workspace --all-targets --all-features
cargo check --workspace --no-default-features --features tui --all-targets
cargo check --workspace --no-default-features --features webapp --all-targets
cargo test --workspace
```

Expected: all green. The `webapp`-only build is the one that exercises the `not(tui)` arm — if `show_report` only compiles under one feature set, the extraction is in the wrong place.

- [ ] **Step 4: Extract the remaining two helpers**

- `spawn_webserver_if_configured(...) -> bool` for the `#[cfg(feature = "webapp")]` block, returning whether a server was started.
- `wait_for_background_tasks(set: JoinSet<()>, webserver_running: bool) -> anyhow::Result<()>` for the shutdown block.

Keep both private and `cfg`-gate them to match the code they replace.

- [ ] **Step 5: Verify all three feature combinations and the CLI end to end**

```bash
cargo test --workspace
cargo test --workspace --no-default-features --features tui
cargo test --workspace --no-default-features --features webapp
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
TMP=$(mktemp -d) && cargo run -p cli -- --noedit --data-directory "$TMP" --week
```

Expected: all green; the CLI prints a weekly summary. **Never omit `--noedit --data-directory`.**

- [ ] **Step 6: Strip and commit**

```bash
todo-parser TIDY.md --strip T11
git add -A && git commit -m "tidy(long-methods): split main_impl and dedupe its report dispatch [T11]

The weekly/single-day dispatch was duplicated verbatim between the tui
and not(tui) arms. Now one show_report helper, plus helpers for the
webserver spawn and the shutdown wait.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

**MILESTONE after Task 21:** full Rust suite across all three feature combinations.

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
export SKIP_YARN=1
cargo test --workspace
cargo test --workspace --no-default-features --features tui
cargo test --workspace --no-default-features --features webapp
git status --porcelain cli/tests/golden/
```

All green, and the golden status must be empty except for Task 15's intended change (already committed).

---

## Wave 6 — The React component split

### Task 22: Split WeeklySummary into two hooks and two row subcomponents [T3]

**Files:**
- Create: `site/src/components/WeeklySummary/useWeeklyTableData.ts`
- Create: `site/src/components/WeeklySummary/useNotesLookup.ts`
- Create: `site/src/components/WeeklySummary/ProjectRow.tsx`
- Create: `site/src/components/WeeklySummary/DailyTotalsRow.tsx`
- Create: `site/src/components/__tests__/WeeklySummary.test.tsx`
- Modify: `site/src/components/WeeklySummary.tsx`

**Interfaces:**
- Consumes: `copyNotesToClipboard` from Task 11; `parseDateString` from Task 10.
- Produces: nothing downstream.

**Risk: high — needs characterization tests first.**

**Context:** 219 lines combining table-data derivation, two lookup memos, clipboard side effects, and deeply nested JSX. Note the doc-comment commit `edd4ae8` rewrote the comment at line 85 (the `+ 'T00:00:00'` local-parse explanation) — that line is now a candidate to call `parseDateString` from Task 10 instead. Do that as part of this task.

- [ ] **Step 1: Write characterization tests for the rendered output**

Create `site/src/components/__tests__/WeeklySummary.test.tsx`. Build a fixture `weekData` covering: two projects with different totals, a day with notes and a day without, and a day whose weekday name would differ between UTC and local parsing. Assert the rendered table's row order, the per-project totals, the daily totals row, and the weekday headers.

Read the current `WeeklySummary.tsx` to shape the fixture to the real prop type. The point is to pin what renders **today**, before any extraction.

- [ ] **Step 2: Confirm they pass on the unchanged component**

```bash
cd site && TZ=America/New_York yarn test --run src/components/__tests__/WeeklySummary.test.tsx
```

Expected: PASS. If a weekday assertion fails, you have found a live bug — report it rather than encoding the wrong value into the test.

- [ ] **Step 3: Commit the characterization tests alone**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
git add -A && git commit -m "test: characterize WeeklySummary rendering before tidy [T3]

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

- [ ] **Step 4: Extract the two hooks**

- `useWeeklyTableData(weekData)` — the `tableData` useMemo (lines 41-93). While moving it, replace the `new Date(day.date + 'T00:00:00')` at line 85 with `parseDateString(day.date)`.
- `useNotesLookup(weekData)` — `daysByDate`, `notesByDate`, `getNotesForProjectDate`, `formatNotesTooltip` and the day-notes copy handler (lines 96-145). The copy handler calls Task 11's `copyNotesToClipboard`; it must not carry its own toast code.

Run the characterization tests after each hook extraction, not just at the end.

- [ ] **Step 5: Extract the two row subcomponents**

- `ProjectRow` (lines 202-234) and `DailyTotalsRow` (lines 236-249), each taking its derived data as props.

`WeeklySummary.tsx` then composes the two hooks plus the `<table>` shell.

- [ ] **Step 6: Verify**

```bash
cd site
TZ=America/New_York yarn test --run
TZ=UTC yarn test --run
yarn build
./node_modules/.bin/eslint src --report-unused-disable-directives --max-warnings 0
```

Expected: all exit 0, and the characterization tests from Step 1 still pass unchanged. If you had to edit them to make them pass, the extraction changed behavior — revert and redo.

- [ ] **Step 7: Strip and commit**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --strip T3
git add -A && git commit -m "tidy(long-methods): split WeeklySummary into hooks and row components [T3]

219 lines become useWeeklyTableData, useNotesLookup, ProjectRow and
DailyTotalsRow plus a table shell. The notes copy path now calls T20's
shared clipboard helper, and the day parse uses T42's parseDateString.
Rendering pinned by the characterization tests committed just before.

Claude-Session: https://claude.ai/code/session_01FP5AsieuxG28QbjDGqCQuP"
```

---

## Final verification

- [ ] **Step 1: Confirm every selected finding was stripped**

```bash
cd /home/steve/src/time-tracking/time-tracking-cli/.worktrees/tidy
todo-parser TIDY.md --summary
```

Expected: 23 active items remain (the unchecked ones), 0 marked execute, 1 archived under Skip. If any `[x] execute` item survives, its task did not complete — find it and finish it.

- [ ] **Step 2: Run the full gate across all three feature combinations**

```bash
export SKIP_YARN=1
just gate
```

Expected: exit 0. This is the only step that exercises the `tui`-only and `webapp`-only builds together with the `cargo tree -i` feature-isolation assertions.

- [ ] **Step 3: Run the frontend suite in both timezones**

```bash
cd site
yarn build
TZ=America/New_York yarn test --run
TZ=UTC yarn test --run
yarn lint
```

Expected: all exit 0. `yarn lint` (the bare `eslint .` form) should now finish in seconds thanks to Task 1 — if it hangs, Task 1 regressed.

- [ ] **Step 4: Confirm the commit log is one commit per finding**

```bash
git log --oneline 816020d..HEAD
```

Expected: one `tidy(...)` commit per finding ID, plus the `test:` characterization commits for T6 and T3, plus the earlier `docs:` and `chore:` commits. Every `tidy(...)` commit should carry its `[T<n>]` tag.

- [ ] **Step 5: Report status**

Report green/red plainly, naming any finding that was dropped or partially applied and why. No summary commit — the per-finding commits are the audit trail.
