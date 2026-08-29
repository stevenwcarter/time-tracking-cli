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
# The last line below is the only thing in the repo that actually runs the
# (webapp, not tui) combination; the check/clippy lines above it only
# compile it.
# Runs the four-command verification gate across all three supported feature combinations.
gate:
    test -d site/build || { echo "site/build is missing — run 'cd site && yarn install && yarn build' first (rust-embed needs it at compile time for the webapp feature)" >&2; exit 1; }
    SKIP_YARN=1 cargo check --workspace --all-targets --all-features
    SKIP_YARN=1 cargo test --workspace
    SKIP_YARN=1 cargo clippy --all-targets --all-features -- -D warnings
    cargo fmt --all -- --check
    SKIP_YARN=1 cargo check -p cli --no-default-features --features tui --all-targets
    SKIP_YARN=1 cargo clippy -p cli --no-default-features --features tui --all-targets -- -D warnings
    SKIP_YARN=1 cargo test -p cli --no-default-features --features tui
    SKIP_YARN=1 cargo check -p cli --no-default-features --features webapp --all-targets
    SKIP_YARN=1 cargo clippy -p cli --no-default-features --features webapp --all-targets -- -D warnings
    SKIP_YARN=1 cargo test -p cli --no-default-features --features webapp

cover:
    cargo llvm-cov --lcov --output-path lcov.info

changelog:
    npx standard-version --skip.bump --skip.commit --skip.tag --dry-run=false
