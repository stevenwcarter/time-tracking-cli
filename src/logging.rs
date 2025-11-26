use tokio::fs;

use anyhow::{Context, Result};

pub async fn init_tracing() -> Result<()> {
    dotenvy::dotenv().ok();

    let log_path = dirs::data_local_dir().context("Could not get local directory")?;

    let log_path = log_path.join("time-tracking-cli");

    fs::create_dir_all(&log_path)
        .await
        .context("Could not create log directory")?;

    let file_appender = tracing_appender::rolling::never(&log_path, "log.txt");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(true)
        .init();

    std::mem::forget(_guard);

    Ok(())
}
