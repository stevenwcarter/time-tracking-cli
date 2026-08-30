//! Runtime smoke tests that verify behavior which is invisible to clippy/fmt.
//! E.g., graceful shutdown mechanics, process lifecycle.

mod common;

#[cfg(all(feature = "webapp", not(feature = "tui")))]
#[test]
fn webapp_only_server_shutdown_channel_stays_alive() {
    use std::net::TcpStream;
    use std::process::Stdio;
    use std::thread;
    use std::time::Duration;

    // Use a tempdir for data, not the real data directory.
    // This prevents the test from touching ~/.time-tracking/.
    let data_dir = tempfile::tempdir().expect("failed to create temp dir");

    // Find an available port by binding to port 0 (OS assigns ephemeral port)
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind to ephemeral port");
    let port = listener
        .local_addr()
        .expect("failed to get local addr")
        .port();
    drop(listener); // Release the port for the server to use

    // Spawn ttcli with --serve, --port, --noedit (prevents editor hang),
    // and --data-directory (prevents touching real data).
    // If the oneshot sender is dropped immediately (bare `_` bug), the server
    // will shut down within ~0.8ms and stop accepting connections.
    // `--noedit` already keeps this run away from the editor; going through
    // `common::ttcli` as well means the guarantee does not depend on that flag
    // surviving a future edit to this argument list.
    let mut child = common::ttcli()
        .args([
            "--serve",
            "--port",
            &port.to_string(),
            "--noedit",
            "--data-directory",
        ])
        .arg(data_dir.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn ttcli");

    // Give the server time to start listening on the port.
    // If the shutdown signal fires immediately (the bug), the port will close
    // before we can connect. The delay here is longer than the ~0.8ms bug symptom.
    thread::sleep(Duration::from_millis(500));

    // Probe the port: try to connect. If the connection succeeds, the server
    // is accepting connections and the shutdown channel is still alive.
    // If the shutdown fired immediately, the server closed the listener and
    // connection will fail.
    let server_is_alive = TcpStream::connect(("127.0.0.1", port)).is_ok();

    // Kill the process. kill() signals the direct child; --noedit ensures
    // no editor subprocess is spawned, so there's nothing to orphan.
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        server_is_alive,
        "server did not accept connections; the graceful shutdown channel was closed too early"
    );
}

/// The startup banner used to print to stdout unconditionally whenever the
/// webserver task was spawned — including under `--serve --tui`, where it
/// raced `ratatui::init()`'s alternate-screen entry and could leave text
/// stranded in a frame ratatui's diff renderer does not know changed.
#[cfg(feature = "webapp")]
#[test]
fn serving_prints_no_banner_to_stdout() {
    use std::io::Read;
    use std::process::Stdio;
    use std::thread;
    use std::time::Duration;

    let data_dir = tempfile::tempdir().expect("failed to create temp dir");

    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind to ephemeral port");
    let port = listener
        .local_addr()
        .expect("failed to get local addr")
        .port();
    drop(listener);

    let mut child = common::ttcli()
        .args([
            "--serve",
            "--port",
            &port.to_string(),
            "--noedit",
            "--data-directory",
        ])
        .arg(data_dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn ttcli");

    thread::sleep(Duration::from_millis(500));
    child.kill().expect("failed to kill ttcli");
    child.wait().expect("failed to reap ttcli");

    let mut stdout = String::new();
    child
        .stdout
        .take()
        .expect("piped stdout")
        .read_to_string(&mut stdout)
        .expect("failed to read ttcli stdout");

    assert!(
        !stdout.contains("Other jobs are running"),
        "the background-task banner must not reach stdout — it races the \
         TUI's alternate screen under `--serve --tui`. Got:\n{stdout}"
    );
}
