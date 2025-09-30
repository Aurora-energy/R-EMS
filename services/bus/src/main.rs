//! R-EMS Event Bus
//!
//! This file contains a minimal yet well-documented skeleton for the tonic-based
//! event bus. Later phases will flesh out the publish/subscribe logic; for now
//! we provide a fully-commented placeholder server that demonstrates logging,
//! configuration loading, and graceful shutdown patterns.

use std::net::SocketAddr;

use axum::{routing::get, Router};
use tokio::signal;
use tracing::info;

/// Default address used for the gRPC and HTTP servers. Future iterations will
/// split these if required by operations.
const DEFAULT_ADDR: &str = "0.0.0.0:7000";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise tracing so logs are structured and consistent across services.
    tracing_subscriber::fmt().with_env_filter("info").init();

    // Parse the listen address from an environment variable. Operators can
    // override this during deployment without rebuilding the container image.
    let addr: SocketAddr = std::env::var("REMS_BUS_BIND")
        .unwrap_or_else(|_| DEFAULT_ADDR.to_string())
        .parse()?;

    info!(%addr, "starting bus HTTP control plane");

    // Build a placeholder Axum router with only a health check. The gRPC
    // server will be added in later phases.
    let app = Router::new().route("/healthz", get(|| async { "ok" }));

    // Launch the HTTP server and shut down cleanly on signal.
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Shared graceful shutdown helper used throughout the workspace. Keeping this
/// logic identical across binaries makes operational behaviour predictable.
async fn shutdown_signal() {
    tokio::select! {
        _ = signal::ctrl_c() => {},
        #[cfg(unix)]
        _ = async {
            use tokio::signal::unix::{signal, SignalKind};
            let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
            term.recv().await;
        } => {},
    }
}
