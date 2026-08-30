use anyhow::{Context, Result, bail};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn get_editor() -> String {
    env::var("EDITOR")
        .or_else(|_| env::var("VISUAL"))
        .unwrap_or_else(|_| {
            // Default editors by platform
            if cfg!(target_os = "macos") {
                "nano".to_string()
            } else if cfg!(target_os = "windows") {
                "notepad".to_string()
            } else {
                "nano".to_string()
            }
        })
}

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

/// Open `file_path` in `$EDITOR` — or `$VISUAL`, or a platform default —
/// inheriting stdio so a terminal editor can take over the screen.
///
/// Blocks until the editor exits, and errors if it exits non-zero.
// `&PathBuf` (rather than clippy's suggested `&Path`) is pinned here: this
// function is re-exported from the crate root and consumed by an external
// Neovim plugin, so the parameter type is part of a stable public API this
// task must not change. The body immediately reborrows it as `&Path`.
#[allow(clippy::ptr_arg)]
pub fn open_in_editor(file_path: &PathBuf) -> Result<()> {
    open_in_editor_with(&get_editor(), file_path)
}

/// [`open_in_editor`] with the editor supplied rather than read from the
/// environment, so tests can exercise it without mutating process-wide state.
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_get_editor_returns_string() {
        // Just verify that get_editor returns a non-empty string
        let editor = get_editor();
        assert!(!editor.is_empty());
    }

    #[test]
    fn test_get_editor_platform_defaults() {
        // We can't easily test env var manipulation due to unsafe requirements,
        // but we can test that the function returns expected platform defaults
        // when env vars are not set (assuming they're not set in test environment)
        let editor = get_editor();

        // Should be one of the expected values
        assert!(
            editor == "nano"
                || editor == "notepad"
                || editor.contains("vim")
                || editor.contains("emacs")
                || editor.contains("code")
                || editor.contains("nano")
                || editor.contains("vi")
        );
    }

    #[test]
    fn test_editor_command_creation() {
        // Test that we can create the basic structure without running
        let test_editor = "test_editor";
        let test_file = PathBuf::from("test.txt");

        // This tests the internal logic without actually executing
        let mut command = Command::new(test_editor);
        command.arg(&test_file);
        command.stdin(Stdio::inherit());
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());

        // Verify the command was set up correctly
        // We can't easily inspect Command internals, but we can verify it was created
        assert!(format!("{:?}", command).contains("test_editor"));
    }

    #[test]
    fn test_pathbuf_handling() {
        // Test that PathBuf can be created and used as expected
        let paths = [
            PathBuf::from("simple.txt"),
            PathBuf::from("/absolute/path/file.txt"),
            PathBuf::from("./relative/path/file.txt"),
            PathBuf::from("../parent/file.txt"),
            PathBuf::from("file with spaces.txt"),
        ];

        for path in &paths {
            // Verify we can create commands with these paths
            let mut command = Command::new("echo");
            command.arg(path);

            // Just verify the command was created (don't execute)
            assert!(format!("{:?}", command).contains("echo"));
        }
    }

    #[test]
    fn test_get_editor_function_exists() {
        // Simple test to verify the function compiles and can be called
        let _editor = get_editor();

        // Test multiple calls return the same result
        let editor1 = get_editor();
        let editor2 = get_editor();
        assert_eq!(editor1, editor2);
    }

    #[test]
    fn test_open_in_editor_function_signature() {
        // Test that the function exists with the correct signature
        // WITHOUT actually executing it to avoid opening editors
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();

        // Instead of calling the function, we'll just verify it compiles
        // and has the correct type signature by creating a function pointer
        let _fn_ptr: fn(&PathBuf) -> Result<()> = open_in_editor;

        // This test verifies the function signature exists and compiles
        // without actually executing the potentially dangerous function
    }

    #[test]
    fn test_stdio_configuration() {
        // Test that we can configure Stdio as expected
        let mut command = Command::new("echo");
        command.arg("test");
        command.stdin(Stdio::inherit());
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());

        // Verify command was created
        assert!(format!("{:?}", command).contains("echo"));
    }

    // NOTE: `open_in_editor` itself is not executed directly in tests, since
    // that would depend on process-wide $EDITOR/$VISUAL state. Its testable
    // core, `open_in_editor_with`, takes the editor as a plain argument and
    // is exercised directly below.

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
            Some(("subl".to_owned(), vec!["-n".to_owned(), "-w".to_owned()]))
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
}
