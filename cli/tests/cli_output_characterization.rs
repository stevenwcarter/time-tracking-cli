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

/// Ties are ordered by `HashMap` iteration today, so this asserts a property
/// rather than exact bytes. It is expected to be unstable until Task 11 adds
/// the name tiebreak.
#[test]
#[ignore = "unstable until Task 11 sorts weekly projects by (minutes desc, name asc)"]
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
}
