//! Characterization tests that pin `ttcli`'s stdout across formatters.
//!
//! These exist to make the W18 weekly-aggregation extraction (Task 11 of the
//! TUI overhaul plan) provably output-preserving: if the extraction changes a
//! single byte of stdout for any of these invocations, one of these tests
//! fails. Golden files live in `tests/golden/`; regenerate them with
//! `BLESS_GOLDEN=1 cargo test -p cli --test cli_output_characterization`.

use std::path::{Path, PathBuf};
use std::time::Duration;

mod common;

/// Copy a fixture week into a fresh temp dir. Required because
/// `show_single_day` creates a template file for a missing date, which
/// would otherwise mutate the checked-in fixtures.
fn staged(fixture: &str) -> tempfile::TempDir {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture);
    let dir = tempfile::tempdir().expect("tempdir");
    for entry in std::fs::read_dir(&src).expect("fixture dir must exist") {
        let entry = entry.expect("dir entry");
        std::fs::copy(entry.path(), dir.path().join(entry.file_name())).expect("copy fixture");
    }
    dir
}

/// Run `ttcli` against `data_dir` and return its stdout.
///
/// `HOME` and `XDG_CONFIG_HOME` are pointed at a throwaway directory so the
/// child cannot read the developer's own
/// `~/.config/time-tracking-cli/config.toml`. That matters because `prefix`,
/// `suffix` and `template_file` have no CLI flag and no env override, so both
/// parse paths take them from the ambient config: a developer whose config
/// sets the `prefix = "```timetracking"` that the shipped config and the
/// README both suggest would otherwise see every fixture fenced out, every
/// total collapse to zero, and every golden here fail on a clean checkout.
/// `Config::load` creates its own default config under the scratch dir, which
/// is deterministic. (`dirs::config_dir` reads `XDG_CONFIG_HOME` on Linux and
/// derives from `HOME` on macOS, hence both.)
///
/// `EDITOR`/`VISUAL` are neutralised by [`common::ttcli`] rather than left to
/// each caller's `--noedit`. Six of the seven invocations below happen to use
/// `--week`, which never reaches the editor, and the seventh remembers the
/// flag — but that is a discipline one new day-view golden away from being
/// forgotten, and forgetting it hands the test runner's terminal to a
/// full-screen editor that outlives the suite.
fn run_ttcli(args: &[&str], data_dir: &Path) -> String {
    let cfg_home = tempfile::tempdir().expect("config tempdir");
    let mut cmd = common::ttcli();
    cmd.args(args).env("NO_COLOR", "1");
    common::scoped(&mut cmd, data_dir, cfg_home.path());
    let out = common::output_within(cmd, Duration::from_secs(30));
    assert!(
        out.status.success(),
        "ttcli exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("stdout was not utf-8")
}

/// Golden-file comparison. `BLESS_GOLDEN=1` (re)writes the golden.
fn compare_golden(name: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.txt"));
    if std::env::var("BLESS_GOLDEN").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden {}; rerun with BLESS_GOLDEN=1",
            path.display()
        )
    });
    assert_eq!(expected, actual, "stdout changed for {name}");
}

#[test]
fn weekly_summary_output_is_stable_across_formatters() {
    for f in ["default", "plain", "markdown"] {
        let dir = staged("week_no_ties");
        let got = run_ttcli(
            &[
                "--week",
                "--date",
                "2026-08-24",
                "--formatter",
                f,
                "--week-start-day",
                "Saturday",
            ],
            dir.path(),
        );
        compare_golden(&format!("weekly_{f}"), &got);
    }
}

#[test]
fn single_day_output_is_stable_across_formatters() {
    for f in ["default", "plain", "markdown"] {
        let dir = staged("week_no_ties");
        let got = run_ttcli(
            &["--date", "2026-08-24", "--noedit", "--formatter", f],
            dir.path(),
        );
        compare_golden(&format!("day_{f}"), &got);
    }
}

#[test]
fn missing_day_still_renders_no_file_found() {
    let dir = staged("week_no_ties");
    let got = run_ttcli(
        &[
            "--week",
            "--date",
            "2026-08-24",
            "--formatter",
            "plain",
            "--week-start-day",
            "Saturday",
        ],
        dir.path(),
    );
    assert!(
        got.contains("2026-08-23"),
        "the omitted day must still appear in the week"
    );
}

/// Pins three things: the `⚠️  WEEKLY WARNINGS` block itself (which no
/// other fixture triggers), and both `time-tracking-parser` warning
/// message shapes it can contain. `week_with_warnings` has one day with a
/// single entry over `MAX_ENTRY_DURATION_MINUTES` (8h) and another with a
/// gap between consecutive entries over `MAX_GAP_DURATION_MINUTES` (6h).
#[test]
fn weekly_summary_includes_warnings_section() {
    let dir = staged("week_with_warnings");
    let got = run_ttcli(
        &[
            "--week",
            "--date",
            "2026-08-24",
            "--formatter",
            "default",
            "--week-start-day",
            "Saturday",
        ],
        dir.path(),
    );
    assert!(
        got.contains("WEEKLY WARNINGS"),
        "expected a warnings section in:\n{got}"
    );
    assert!(
        got.contains("appears to be longer than 8 hours"),
        "expected the over-long-entry warning in:\n{got}"
    );
    assert!(
        got.contains("appears to be longer than 6 hours"),
        "expected the over-long-gap warning in:\n{got}"
    );
    compare_golden("weekly_with_warnings_default", &got);
}

/// The `plain` formatter exists to emit no emoji, and the weekly warnings
/// block is the one part of the weekly render that used to be printed by the
/// shared renderer rather than by the formatter — so `plain` emitted `\u{26a0}\u{fe0f}` and
/// `\u{26a0}` here regardless. Nothing covered `plain` + warnings, so the
/// inconsistency went unpinned.
#[test]
fn plain_weekly_warnings_carry_no_emoji() {
    let dir = staged("week_with_warnings");
    let got = run_ttcli(
        &[
            "--week",
            "--date",
            "2026-08-24",
            "--formatter",
            "plain",
            "--week-start-day",
            "Saturday",
        ],
        dir.path(),
    );
    assert!(
        got.contains("WEEKLY WARNINGS"),
        "expected a warnings section in:\n{got}"
    );
    let stray: Vec<char> = got
        .chars()
        .filter(|c| !c.is_ascii() && !c.is_alphanumeric())
        .collect();
    assert!(
        stray.is_empty(),
        "the plain formatter must emit no emoji, found {stray:?} in:\n{got}"
    );
    compare_golden("weekly_with_warnings_plain", &got);
}

/// Ties are broken by project name, so this asserts a property rather than
/// exact bytes: repeated runs over the same fixture must agree.
#[test]
fn weekly_tie_ordering_is_deterministic() {
    let dir = staged("week_with_ties");
    let first = run_ttcli(
        &[
            "--week",
            "--date",
            "2026-08-24",
            "--week-start-day",
            "Saturday",
        ],
        dir.path(),
    );
    for _ in 0..20 {
        let again = run_ttcli(
            &[
                "--week",
                "--date",
                "2026-08-24",
                "--week-start-day",
                "Saturday",
            ],
            dir.path(),
        );
        assert_eq!(first, again, "tie ordering varied between runs");
    }
    // Determinism alone would be satisfied by any fixed order, so pin which
    // one: alpha and zulu tie on minutes, and the name tiebreak is ascending.
    assert!(
        first.find("alpha") < first.find("zulu"),
        "tied projects must be ordered by name ascending in:\n{first}"
    );
}

/// The banner is gated on `webserver_running` (`cli/src/main.rs`), so a
/// TUI-only launch must not mention the webserver.
///
/// **The child must be put in a new session, or this test does not
/// terminate.** There is no such thing as "no TTY" here just because
/// `output()` pipes stdout: nothing in the code path reads `TERM`, and on Unix
/// crossterm does not read stdin at all — it opens `/dev/tty` directly. A
/// child that keeps the test runner's controlling terminal therefore starts a
/// real TUI on the developer's terminal and reads their keystrokes; an `e`
/// reaches `App::run_editor`, and `open_in_editor` spawns `$EDITOR` with
/// `Stdio::inherit()`, so the editor holds the write end of `output()`'s pipe
/// and EOF never arrives even after `ttcli` exits. `setsid(2)` gives the child
/// no controlling terminal, so crossterm's `open("/dev/tty")` fails with
/// `ENXIO` and the TUI bails out immediately. `Stdio::null()` on stdin alone
/// is not sufficient, precisely because crossterm bypasses stdin.
#[cfg(unix)]
#[test]
fn tui_only_launch_prints_no_webserver_banner() {
    use std::os::unix::process::CommandExt;

    let dir = staged("week_no_ties");
    let mut cmd = common::ttcli();
    cmd.args(["--tui", "--data-directory"]).arg(dir.path());
    // SAFETY: `setsid` is async-signal-safe and touches no allocator or lock
    // state, which is the entire requirement on a `pre_exec` hook. The child
    // is freshly forked and so is never already a process-group leader, the
    // one condition under which `setsid` fails.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let out = common::output_within(cmd, Duration::from_secs(30));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("webserver"),
        "a TUI-only launch must not mention the webserver: {stdout}"
    );
}

/// The harness must neutralise the editor, not merely avoid provoking it.
///
/// `display::show_single_day` calls `open_in_editor` on *every* day-view run
/// that omits `--noedit`; creating the file first makes no difference. So the
/// suite's safety currently rests on each caller remembering a flag — and
/// `editor::get_editor` falls back to `nano` when `EDITOR` and `VISUAL` are
/// both unset, meaning an unset environment is an interactive one, not a safe
/// one. This runs the exact invocation a forgetful future test would write and
/// asserts it dies instead of waiting for a human.
///
/// It is deliberately the failure mode, not the success one: pointing `EDITOR`
/// at `true` would let an unexpected launch pass silently, which is how the
/// orphaned editors went unnoticed for hours in the first place.
///
/// `output_within` is what keeps a regression here honest. Without it, undoing
/// the neutralisation would make this test *hang* rather than fail, which is
/// precisely the symptom it exists to prevent.
#[test]
fn a_day_view_without_noedit_fails_fast_instead_of_opening_an_editor() {
    let dir = staged("week_no_ties");
    let cfg_home = tempfile::tempdir().expect("config tempdir");
    let mut cmd = common::ttcli();
    cmd.args(["--date", "2026-08-24", "--formatter", "plain"]);
    common::scoped(&mut cmd, dir.path(), cfg_home.path());

    let out = common::output_within(cmd, Duration::from_secs(30));

    assert!(
        !out.status.success(),
        "a run that reaches the editor must fail, not succeed quietly"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Editor"),
        "the failure must name the editor as its cause, so a future reader is not left \
         guessing why a day view exits non-zero; got: {stderr}"
    );
}
