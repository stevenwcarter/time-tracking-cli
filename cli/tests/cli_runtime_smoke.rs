//! Runtime smoke tests that verify behavior which is invisible to clippy/fmt.
//! E.g., graceful shutdown mechanics, process lifecycle.

#[cfg(feature = "webapp")]
#[test]
fn webapp_only_server_stays_running() {
    use std::process::Command;
    use std::time::Duration;
    use std::thread;

    // Find an available port by binding to port 0 (OS assigns ephemeral port)
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind to ephemeral port");
    let port = listener
        .local_addr()
        .expect("failed to get local addr")
        .port();
    drop(listener); // Release the port for the server to use

    // Spawn ttcli with --serve and --port, capturing output
    let mut child = Command::new(env!("CARGO_BIN_EXE_ttcli"))
        .args(&["--serve", "--port"])
        .arg(port.to_string())
        .env("RUST_LOG", "info")
        .spawn()
        .expect("failed to spawn ttcli");

    // Give the server time to start and (crucially) NOT shut down immediately.
    // If the sender drops early (bare `_` bug), the server will shut down within ~0.8ms.
    // 1.5 seconds is far longer than needed to detect that regression.
    thread::sleep(Duration::from_millis(1500));

    // Check that the process is still alive.
    // try_wait() returns Ok(None) if the process is still running.
    let status = child
        .try_wait()
        .expect("failed to call try_wait on child process");
    assert!(
        status.is_none(),
        "ttcli --serve exited prematurely; the graceful shutdown signal is being sent too early"
    );

    // Clean up: kill the process.
    let _ = child.kill();
    let _ = child.wait();
}
