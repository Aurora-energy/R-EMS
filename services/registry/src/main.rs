//! Plugin Registry Skeleton
//!
//! The registry service will validate plugin manifests and enforce topic ACLs.
//! For the bootstrap stage we provide a small Axum server with placeholder
//! routes so that documentation and tooling can begin referencing them.

use std::net::SocketAddr;

use axum::{routing::get, Json, Router};
use serde::Serialize;
use tokio::{net::TcpListener, signal};
use tracing::info;

/// Placeholder plugin listing response.
#[derive(Serialize)]
struct PluginList {
    plugins: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let addr: SocketAddr = std::env::var("REMS_REGISTRY_BIND")
        .unwrap_or_else(|_| "0.0.0.0:7200".to_string())
        .parse()?;

    info!(%addr, "starting registry bootstrap server");

    let app = Router::new()
        .route("/api/plugins", get(list_plugins))
        .route("/api/plugins/toggle", get(toggle_placeholder))
        .route("/healthz", get(|| async { "ok" }));

    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn list_plugins() -> Json<PluginList> {
    Json(PluginList {
        plugins: vec!["example-plugin".into()],
    })
}

async fn toggle_placeholder() -> Json<PluginList> {
    Json(PluginList { plugins: vec![] })
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{signal as unix_signal, SignalKind};

    let mut term = unix_signal(SignalKind::terminate()).expect("install SIGTERM handler");

    tokio::select! {
        _ = signal::ctrl_c() => {},
        _ = term.recv() => {},
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = signal::ctrl_c().await;
}
