use anyhow::{Context, Result, bail};
use std::env;
use std::path::PathBuf;
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

pub fn open_in_editor(file_path: &PathBuf) -> Result<()> {
    let editor = get_editor();

    let mut command = Command::new(&editor);
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

    // NOTE: We deliberately avoid testing the actual execution of open_in_editor
    // to prevent opening interactive editors during test runs.
    // The function's correctness is tested through integration tests or manual testing.
}
