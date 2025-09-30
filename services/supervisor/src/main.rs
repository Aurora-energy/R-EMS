//! R-EMS Supervisor
//!
//! This executable will eventually manage plugin lifecycles via the Docker API.
//! The current bootstrap stage implements only the HTTP surface and logging
//! setup to keep the repository consistent while subsequent phases fill in the
//! operational logic.

use std::net::SocketAddr;

use axum::{routing::get, Json, Router};
use serde::Serialize;
use tokio::{net::TcpListener, signal};
use tracing::{info, warn};

/// Default bind address for the supervisor API.
const DEFAULT_ADDR: &str = "0.0.0.0:7100";

/// Basic health response payload shared across services.
#[derive(Serialize)]
struct Health {
    status: &'static str,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let addr: SocketAddr = std::env::var("REMS_SUPERVISOR_BIND")
        .unwrap_or_else(|_| DEFAULT_ADDR.to_string())
        .parse()?;

    info!(%addr, "starting supervisor skeleton");

    let app = Router::new()
        .route(
            "/api/health",
            get(|| async { Json(Health { status: "ok" }) }),
        )
        .route("/healthz", get(|| async { "ok" }));

    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        tokio::select! {
            _ = ctrl_c() => {},
            _ = terminate() => {},
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c().await;
    }
}

async fn ctrl_c() {
    if let Err(err) = signal::ctrl_c().await {
        warn!(?err, "failed to install Ctrl+C handler");
    }
}

#[cfg(unix)]
async fn terminate() {
    use tokio::signal::unix::{signal, SignalKind};

    match signal(SignalKind::terminate()) {
        Ok(mut term) => {
            term.recv().await;
        }
        Err(err) => warn!(?err, "failed to install SIGTERM handler"),
    }
}

#[cfg(not(unix))]
async fn terminate() {}
