test:
    watchexec -e rs,toml cargo test

# CI only runs `cargo build`, and `just test` above watches the default
# feature set only — neither exercises the `tui`-only or `webapp`-only
# builds. This recipe is the one place that does. `webapp` embeds
# `site/build/` into the binary via rust-embed at compile time, so it has
# to exist before anything below runs, or rust-embed fails with a macro
# error rather than a clear message. `SKIP_YARN=1` only skips build.rs's
# own `yarn build` step; it does not create this directory.
#
# The `cargo tree -i` lines assert real feature isolation, not just that
# `cli`'s own #[cfg]s toggle correctly: `cli/Cargo.toml`'s dependency on
# `time-tracking-cli` used to omit `default-features = false`, so Cargo
# unioned in every feature regardless of what was requested here, and the
# library silently compiled with both `webapp` and `tui` under every
# combination below. A gate that cannot detect that defeat is worse than
# no gate, so these fail loudly (naming the exact cause) rather than
# passing quietly if that regresses.
# Runs the four-command verification gate across all three supported feature combinations.
gate:
    test -d site/build || { echo "site/build is missing — run 'cd site && yarn install && yarn build' first (rust-embed needs it at compile time for the webapp feature)" >&2; exit 1; }
    SKIP_YARN=1 cargo check --workspace --all-targets --all-features
    SKIP_YARN=1 cargo test --workspace
    SKIP_YARN=1 cargo clippy --all-targets --all-features -- -D warnings
    cargo fmt --all -- --check
    SKIP_YARN=1 cargo tree --workspace --no-default-features --features tui -i axum >/dev/null 2>&1 && { echo "REGRESSION: axum leaked into the tui-only build — cli/Cargo.toml's time-tracking-cli dependency must keep default-features = false" >&2; exit 1; } || true
    SKIP_YARN=1 cargo check --workspace --no-default-features --features tui --all-targets
    SKIP_YARN=1 cargo clippy --workspace --no-default-features --features tui --all-targets -- -D warnings
    SKIP_YARN=1 cargo test --workspace --no-default-features --features tui
    SKIP_YARN=1 cargo tree --workspace --no-default-features --features webapp -i ratatui >/dev/null 2>&1 && { echo "REGRESSION: ratatui leaked into the webapp-only build — cli/Cargo.toml's time-tracking-cli dependency must keep default-features = false" >&2; exit 1; } || true
    SKIP_YARN=1 cargo check --workspace --no-default-features --features webapp --all-targets
    SKIP_YARN=1 cargo clippy --workspace --no-default-features --features webapp --all-targets -- -D warnings
    SKIP_YARN=1 cargo test --workspace --no-default-features --features webapp

cover:
    cargo llvm-cov --lcov --output-path lcov.info

changelog:
    npx standard-version --skip.bump --skip.commit --skip.tag --dry-run=false
