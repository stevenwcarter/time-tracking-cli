# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Rust (core library + CLI)
```bash
cargo build                        # Debug build
cargo build --release              # Release build
cargo build --release -p cli       # CLI binary only
cargo test --workspace             # Run all tests
cargo run -p cli -- --help         # Run CLI with args
```

**`just gate` is the verification gate**, not `cargo test` alone. It runs
check / test / clippy `-D warnings` / `fmt --check` across all three supported
feature combinations (default, `tui`-only, `webapp`-only) and asserts real
feature isolation with `cargo tree -i`. CI only runs `cargo build --release -p cli`,
so `just gate` is the only thing that exercises the non-default builds. It needs
`site/build/` to exist first (`cd site && yarn install && yarn build`), because
`rust-embed` pulls it in at compile time under the `webapp` feature.

**Never smoke-run the binary bare.** A plain `ttcli` or `cargo run -p cli --`
defaults to the real `~/.time-tracking/` and opens `$EDITOR` on today's file —
which is where stray orphaned editor processes holding the user's real data
come from. Always pass `--noedit --data-directory <tmp>`.

### Frontend (site/)
```bash
cd site && yarn install            # Install dependencies
cd site && yarn dev                # Dev server
cd site && yarn build              # Production build
cd site && yarn lint               # ESLint
cd site && yarn test               # Vitest
```

### Releases
```bash
npm run release                    # Bump version + update CHANGELOG (standard-version)
```

Commits must follow conventional commit format (`feat:`, `fix:`, `chore:`, etc.) — enforced via Husky + commitlint.

## Architecture

This is a Rust workspace with two crates (`src/` library + `cli/` binary) and a React frontend (`site/`).

### Cargo Features
Three optional features gate major subsystems:
- `webapp` — Axum HTTP server + Juniper GraphQL + `rust-embed` for serving the React SPA
- `tui` — Ratatui terminal UI
- `cli` — Clap argument parsing

All three are enabled by default. `cli/src/main.rs` uses `#[cfg(feature = ...)]` to conditionally compile each mode.

### Core Library (`src/`)
| Module | Purpose |
|--------|---------|
| `config.rs` | CLI args (Clap) + TOML config deserialization + path resolution. Includes `theme` (TUI colour preset, default `"dark"`) and `daily_target_hours` (default `8.0`) — both are TUI-only and read once into `TuiContext` (see below), never accessed elsewhere |
| `data_svc.rs` | `DataService` — the whole data layer. A 30-second in-memory cache holding both the raw content *and* the memoized parse per date; `existing_dates` (one `read_dir` rather than a `stat` per queried day) and `find_populated_dates` for the calendar; and weekly aggregation (`get_weekly_summary`, which owns the project rollup and the minutes-desc-then-name-asc ordering). One unreadable day file is logged and skipped, never fatal to a scan |
| `file_utils.rs` | Directory setup, template handling, file discovery |
| `editor.rs` | Launch `$EDITOR`/`$VISUAL` for a given date file |
| `time_utils.rs` | Date/weekday helpers |
| `display/` | `DisplayFormatter` trait + three impls: `Default` (emoji), `Plain`, `Markdown`. Rendering only — the weekly aggregation it used to do now lives in `data_svc.rs`, and it is handed a `WeeklySummary` to print |
| `graphql.rs` | Juniper schema (queries + mutations) |
| `web.rs` | Axum server with `/graphql` and static asset endpoints |
| `tui/` | Ratatui app — see the breakdown below |

`tui/` submodules:
| File | Purpose |
|------|---------|
| `app.rs` | `App` — state machine + the terminal event loop |
| `ui.rs` | Rendering: breakpoints, layout, the day/week views |
| `context.rs` | `TuiContext` — TUI config resolved once at startup and threaded through everything else. TUI code reads config through it and must never call `Config::get()` directly (only `tui()` in `mod.rs` does) |
| `event.rs` | The terminal input + file-watch event stream |
| `keymap.rs` | The one keybinding table; feeds the help popup and the generated README section |
| `mode.rs` | `Mode`/`Overlay` — what's on screen and who gets first refusal on a keypress |
| `project_list.rs` | The day view's project list widget |
| `week_list.rs` | The weekly per-project rollup pane |
| `theme.rs` | Colour palettes: `dark`, `light`, `none` |
| `testing.rs` | `#[cfg(test)]` render/fixture helpers shared by the unit tests |
| `widgets/` | `Calendar`, `DatePrompt`, `HelpPopup`, `Popup`, `RawFileView`, `WeeklyBarChart` |

### Data Flow
1. Time entries stored as markdown files at `~/.time-tracking/YYYY-MM-DD.md` (configurable)
2. Parsed by the external `time-tracking-parser` crate
3. Cached by `DataService` (30s TTL)
4. Output via CLI formatters **or** served through GraphQL to the React SPA **or** rendered in the TUI

### React Frontend (`site/`)
- Apollo Client for GraphQL
- React Router for navigation
- Tailwind CSS for styling
- Built assets are embedded into the Rust binary via `rust-embed` when the `webapp` feature is enabled

### Key Entry Points
- CLI: `cli/src/main.rs`
- Library public API: `src/lib.rs`
- Web server: `src/web.rs`
- TUI: `src/tui/mod.rs` + `src/tui/app.rs`
- React SPA: `site/src/main.tsx`
