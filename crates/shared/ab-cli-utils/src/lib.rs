//! Utilities used in various CLI applications

use std::panic;
use std::process::exit;
use tokio::signal;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

/// Install a panic handler which exits on panics, rather than unwinding. Unwinding can hang the
/// tokio runtime waiting for stuck tasks or threads.
pub fn set_exit_on_panic() {
    let default_panic_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        default_panic_hook(panic_info);
        exit(1);
    }));
}

/// Initialize logger with typical settings
pub fn init_logger() {
    tracing_subscriber::registry()
        .with(
            fmt::layer().with_filter(
                EnvFilter::builder()
                    .with_default_directive(LevelFilter::INFO.into())
                    .from_env_lossy(),
            ),
        )
        .init();
}

/// Raise soft file descriptor limit to the hard limit, if possible
pub fn raise_fd_limit() {
    match fdlimit::raise_fd_limit() {
        Ok(fdlimit::Outcome::LimitRaised { from, to }) => {
            tracing::debug!(
                "Increased file descriptor limit from previous (most likely soft) limit {} to \
                new (most likely hard) limit {}",
                from,
                to
            );
        }
        Ok(fdlimit::Outcome::Unsupported) => {
            // Unsupported platform (a platform other than Linux or macOS)
        }
        Err(error) => {
            tracing::warn!(
                "Failed to increase file descriptor limit for the process due to an error: {}.",
                error
            );
        }
    }
}

/// Create a future that waits for `SIGINT` or `SIGTERM` to be sent to the process
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use futures::FutureExt;
        use std::pin::pin;

        let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
            .expect("Setting signal handlers must never fail");
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Setting signal handlers must never fail");

        futures::future::select(
            pin!(sigint.recv().map(|_| {
                tracing::info!("Received SIGINT, shutting down farmer...");
            }),),
            pin!(sigterm.recv().map(|_| {
                tracing::info!("Received SIGTERM, shutting down farmer...");
            }),),
        )
        .await;
    }
    #[cfg(not(unix))]
    {
        signal::ctrl_c()
            .await
            .expect("Setting signal handlers must never fail");

        tracing::info!("Received Ctrl+C, shutting down farmer...");
    }
}
