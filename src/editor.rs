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

pub fn open_in_editor(file_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let editor = get_editor();

    let mut command = Command::new(&editor);
    command.arg(file_path);

    // For some editors like vim/nano, we need to inherit stdio
    command.stdin(Stdio::inherit());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let status = command.status()?;

    if !status.success() {
        return Err(format!("Editor '{}' exited with non-zero status", editor).into());
    }

    Ok(())
}
