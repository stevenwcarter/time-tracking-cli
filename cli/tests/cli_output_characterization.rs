//! Characterization tests that pin `ttcli`'s stdout across formatters.
//!
//! These exist to make the W18 weekly-aggregation extraction (Task 11 of the
//! TUI overhaul plan) provably output-preserving: if the extraction changes a
//! single byte of stdout for any of these invocations, one of these tests
//! fails. Golden files live in `tests/golden/`; regenerate them with
//! `BLESS_GOLDEN=1 cargo test -p cli --test cli_output_characterization`.

use std::path::{Path, PathBuf};
use std::process::Command;

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

fn run_ttcli(args: &[&str], data_dir: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_ttcli"))
        .args(args)
        .arg("--data-directory")
        .arg(data_dir)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run ttcli");
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

/// Pins the `⚠️  WEEKLY WARNINGS` block, which no other fixture triggers.
/// `week_with_warnings` has one day with a single entry over
/// `MAX_ENTRY_DURATION_MINUTES` (8h) and another with a gap between
/// consecutive entries over `MAX_GAP_DURATION_MINUTES` (6h), so both
/// `time-tracking-parser` warning message shapes are covered.
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

#[test]
fn tui_only_launch_prints_no_webserver_banner() {
    // --tui with no TTY exits immediately; we only care about stdout.
    let dir = staged("week_no_ties");
    let out = Command::new(env!("CARGO_BIN_EXE_ttcli"))
        .args(["--tui", "--data-directory"])
        .arg(dir.path())
        .env("TERM", "dumb")
        .output()
        .expect("run ttcli");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("webserver"),
        "a TUI-only launch must not mention the webserver: {stdout}"
    );
}
