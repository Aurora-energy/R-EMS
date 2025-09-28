//! ---
//! ems_section: "03-persistence-logging"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Metrics collection and export utilities."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::http::{header, HeaderValue, StatusCode};
use axum::routing::get;
use axum::{response::IntoResponse, Router};
use prometheus::{
    GaugeVec, Histogram, HistogramOpts, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts,
    Registry, TextEncoder,
};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::{error, info};

/// Shared registry type used across services.
pub type SharedRegistry = Arc<Registry>;

/// Produce a new shared registry.
pub fn new_registry() -> SharedRegistry {
    Arc::new(Registry::new())
}

/// Spawn an HTTP server that exposes the registry at `/metrics`.
pub fn spawn_http_server(registry: SharedRegistry, addr: SocketAddr) -> Result<MetricsServer> {
    let app = Router::new().route(
        "/metrics",
        get({
            let registry = registry.clone();
            move || metrics_handler(registry.clone())
        }),
    );

    let std_listener = StdTcpListener::bind(addr)
        .with_context(|| format!("failed to bind metrics listener {}", addr))?;
    std_listener
        .set_nonblocking(true)
        .with_context(|| "failed to configure metrics listener as non-blocking")?;
    let listener = TcpListener::from_std(std_listener)
        .with_context(|| "failed to convert std listener into tokio listener")?;

    info!(address = %addr, "metrics server starting");

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let service = app.into_make_service();
    let handle: JoinHandle<Result<()>> = tokio::spawn(async move {
        axum::serve(listener, service)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
            .context("metrics server encountered an error")?;
        Ok(())
    });

    Ok(MetricsServer {
        addr,
        shutdown: Some(shutdown_tx),
        task: handle,
    })
}

/// Prometheus scrape endpoint. Returns `text/plain` metrics even on large registries.
async fn metrics_handler(registry: SharedRegistry) -> impl IntoResponse {
    let families = registry.gather();
    let encoder = TextEncoder::new();
    match encoder.encode_to_string(&families) {
        Ok(body) => (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(encoder.format_type()),
            )],
            body,
        ),
        Err(err) => {
            error!(error = %err, "failed to encode metrics");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                String::from("metrics encoding error"),
            )
        }
    }
}

/// Handle to the running HTTP exporter.
#[derive(Debug)]
pub struct MetricsServer {
    addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<()>>,
}

impl MetricsServer {
    /// Return the bound address for convenience.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Signal shutdown and await task completion.
    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        match self.task.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(err),
            Err(join_err) => Err(anyhow::Error::new(join_err)),
        }
    }
}

/// Metrics recorded by the daemon process itself.
#[derive(Clone)]
pub struct DaemonMetrics {
    registry: SharedRegistry,
    starts_total: IntCounter,
    config_load_seconds: Histogram,
    build_info: GaugeVec,
}

impl DaemonMetrics {
    pub fn new(registry: SharedRegistry) -> Result<Self> {
        let starts_total = IntCounter::with_opts(Opts::new(
            "r_emsd_starts_total",
            "Total number of times the R-EMS daemon has initialised",
        ))?;
        registry.register(Box::new(starts_total.clone()))?;

        let buckets = prometheus::exponential_buckets(0.001, 2.0, 16)
            .context("failed to construct histogram buckets")?;
        let config_load_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "r_emsd_config_load_seconds",
                "Time spent loading and validating configuration",
            )
            .buckets(buckets),
        )?;
        registry.register(Box::new(config_load_seconds.clone()))?;

        let build_info = GaugeVec::new(
            Opts::new(
                "r_emsd_build_info",
                "Build metadata for the running daemon binary",
            ),
            &["version", "git_sha", "profile"],
        )?;
        registry.register(Box::new(build_info.clone()))?;

        Ok(Self {
            registry,
            starts_total,
            config_load_seconds,
            build_info,
        })
    }

    pub fn registry(&self) -> SharedRegistry {
        self.registry.clone()
    }

    pub fn inc_start(&self) {
        self.starts_total.inc();
    }

    pub fn observe_config_load(&self, seconds: f64) {
        self.config_load_seconds.observe(seconds);
    }

    pub fn set_build_info(&self, version: &str, git_sha: &str, profile: &str) {
        self.build_info
            .with_label_values(&[version, git_sha, profile])
            .set(1.0);
    }
}
#[derive(Clone, Debug)]
pub struct OrchestratorMetrics {
    registry: SharedRegistry,
    grid_total: IntGauge,
    controller_active: IntGaugeVec,
    failovers: IntCounterVec,
}

impl OrchestratorMetrics {
    pub fn new(registry: SharedRegistry) -> Result<Self> {
        let grid_total = IntGauge::with_opts(Opts::new(
            "r_ems_grids_total",
            "Number of grids managed by the orchestrator",
        ))?;
        registry.register(Box::new(grid_total.clone()))?;

        let controller_active = IntGaugeVec::new(
            Opts::new(
                "r_ems_controller_active",
                "Indicator (0/1) whether a controller currently holds the primary role",
            ),
            &["grid", "controller"],
        )?;
        registry.register(Box::new(controller_active.clone()))?;

        let failovers = IntCounterVec::new(
            Opts::new(
                "r_ems_failovers_total",
                "Count of controller promotions by grid and reason",
            ),
            &["grid", "controller", "reason"],
        )?;
        registry.register(Box::new(failovers.clone()))?;

        Ok(Self {
            registry,
            grid_total,
            controller_active,
            failovers,
        })
    }

    pub fn registry(&self) -> SharedRegistry {
        self.registry.clone()
    }

    pub fn set_grid_count(&self, count: usize) {
        self.grid_total.set(count as i64);
    }

    pub fn set_active(&self, grid: &str, controller: &str, active: bool) {
        let gauge = self
            .controller_active
            .with_label_values(&[grid, controller]);
        gauge.set(if active { 1 } else { 0 });
    }

    pub fn record_failover(&self, grid: &str, controller: &str, reason: &str) {
        let counter = self
            .failovers
            .with_label_values(&[grid, controller, reason]);
        counter.inc();
    }
}

pub use prometheus;
