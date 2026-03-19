# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Rust (core library + CLI)
```bash
cargo build                        # Debug build
cargo build --release              # Release build
cargo build --release -p cli       # CLI binary only
cargo test                         # Run all tests
cargo run -p cli -- --help         # Run CLI with args
```

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
| `config.rs` | CLI args (Clap) + TOML config deserialization + path resolution |
| `data_svc.rs` | `DataService` — async file reader with 30-second in-memory cache |
| `file_utils.rs` | Directory setup, template handling, file discovery |
| `editor.rs` | Launch `$EDITOR`/`$VISUAL` for a given date file |
| `time_utils.rs` | Date/weekday helpers |
| `display/` | `DisplayFormatter` trait + three impls: `Default` (emoji), `Plain`, `Markdown` |
| `graphql.rs` | Juniper schema (queries + mutations) |
| `web.rs` | Axum server with `/graphql` and static asset endpoints |
| `tui/` | Ratatui app: `app.rs` (state + event loop), `ui.rs` (rendering), custom widgets |

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
