//! Structured Logger Skeleton
//!
//! Captures log events from other services and exposes replay endpoints. The
//! bootstrap version initialises tracing and provides the HTTP scaffolding with
//! placeholder responses.

use std::net::SocketAddr;

use axum::{routing::get, Router};
use tokio::signal;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let addr: SocketAddr = std::env::var("REMS_LOGGER_BIND")
        .unwrap_or_else(|_| "0.0.0.0:7400".to_string())
        .parse()?;

    info!(%addr, "starting logger bootstrap");

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/metrics", get(|| async { "metrics stub" }))
        .route("/replay", get(|| async { "replay stub" }));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

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
