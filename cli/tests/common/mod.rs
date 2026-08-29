//! Shared spawn helper for the `ttcli` integration tests.
//!
//! Every test that runs the binary must go through [`ttcli`]. The reason is
//! the editor: `display::show_single_day` calls `open_in_editor` for *any*
//! day-view run that does not pass `--noedit` — the file having just been
//! created makes no difference — and `open_in_editor` spawns the editor with
//! `Stdio::inherit()`. A test that forgets the flag therefore hands the test
//! runner's terminal to a full-screen editor that never exits, and the
//! process outlives the suite: this really happened on this branch, leaving
//! orphaned editors holding files open for hours.
//!
//! `--noedit` on every call site is a discipline, not a guarantee, and it is
//! one new golden away from being forgotten. Neutralising `EDITOR` in the
//! harness is the structural version of the same protection.
//!
//! Each integration test file is its own binary and uses a different subset
//! of this module, so unused items here are expected rather than dead.
#![allow(dead_code)]

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// What the tests point `EDITOR`/`VISUAL` at.
///
/// Deliberately a command that *fails* rather than one that quietly succeeds.
/// `true` would make an unexpected editor launch invisible — the run would
/// pass and nobody would learn the test had opened an editor at all. `false`
/// turns the same event into a non-zero exit with `Editor 'false' exited with
/// non-zero status` on stderr, so the test fails and names its own cause.
///
/// This matters more than it looks: `editor::get_editor` falls back to `nano`
/// when both variables are unset, so an *unset* environment is not a safe
/// environment — it is an interactive one.
const NON_INTERACTIVE_EDITOR: &str = "false";

/// A `Command` for the `ttcli` binary with the editor neutralised.
///
/// Callers add their own arguments and environment. `stdin` is closed here
/// too: it is not sufficient on its own — crossterm opens `/dev/tty` directly
/// rather than reading stdin, which is why the TUI test additionally needs
/// `setsid` — but it costs nothing and closes the ordinary case.
pub fn ttcli() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ttcli"));
    cmd.env("EDITOR", NON_INTERACTIVE_EDITOR)
        .env("VISUAL", NON_INTERACTIVE_EDITOR)
        .stdin(Stdio::null());
    cmd
}

/// Run `cmd` to completion, or kill it and panic once `limit` has passed.
///
/// `Command::output` waits forever. Every hang this harness has produced
/// looked identical from the outside — a suite that never finishes and gives
/// no clue which test is responsible — so anything that could plausibly hang
/// goes through here instead, and a regression fails with a named test rather
/// than wedging the run. This mirrors the timeout already wrapping the FIFO
/// scan test in `data_svc`, for the same reason.
pub fn output_within(mut cmd: Command, limit: Duration) -> std::process::Output {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn ttcli");

    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait().expect("failed to poll ttcli") {
            Some(_) => break,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "ttcli did not exit within {limit:?} — it is almost certainly waiting on an \
                     interactive editor or a terminal. Check that this call goes through \
                     `common::ttcli()`."
                );
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }

    child
        .wait_with_output()
        .expect("failed to collect ttcli output")
}

/// The arguments every test needs: a scratch data directory, and a scratch
/// `HOME`/`XDG_CONFIG_HOME` so an ambient `~/.config/time-tracking-cli/
/// config.toml` cannot reach the run.
pub fn scoped(cmd: &mut Command, data_dir: &Path, config_home: &Path) {
    cmd.arg("--data-directory")
        .arg(data_dir)
        .env("HOME", config_home)
        .env("XDG_CONFIG_HOME", config_home);
}
