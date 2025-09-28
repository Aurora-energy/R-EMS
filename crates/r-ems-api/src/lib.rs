//! ---
//! ems_section: "05-networking-external-interfaces"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Networking API surface for external integrations."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, get_service, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use r_ems_common::config::AppConfig;
use r_ems_common::version::VersionInfo;
use r_ems_common::Mode;
use r_ems_sim::{FaultKind, GridSimulationControl};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use toml;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use uuid::Uuid;

const RECENT_ERROR_LIMIT: usize = 20;

/// Shared API state exposed to handlers.
pub struct ApiState {
    config: RwLock<AppConfig>,
    version: VersionInfo,
    mode: Mode,
    start: Instant,
    features: BTreeMap<String, bool>,
    config_path: PathBuf,
    log_dir: PathBuf,
    simulation: Option<Arc<dyn GridSimulationControl>>,
}

impl ApiState {
    pub fn new(
        config: AppConfig,
        mode: Mode,
        version: VersionInfo,
        features: BTreeMap<String, bool>,
        config_path: PathBuf,
        log_dir: PathBuf,
        simulation: Option<Arc<dyn GridSimulationControl>>,
    ) -> Self {
        Self {
            config: RwLock::new(config),
            version,
            mode,
            start: Instant::now(),
            features,
            config_path,
            log_dir,
            simulation,
        }
    }

    fn status(&self) -> StatusResponse {
        let config = self.config.read();
        let grid_count = config.grids.len();
        let controller_count: usize = config
            .grids
            .values()
            .map(|grid| grid.controllers.len())
            .sum();
        StatusResponse {
            mode: self.mode,
            version: self.version.cli_string(),
            git_commit: self.version.git_sha.clone(),
            uptime_seconds: self.start.elapsed().as_secs(),
            grid_count,
            controller_count,
            features: self.features.clone(),
        }
    }

    fn config_snapshot(&self) -> AppConfig {
        self.config.read().clone()
    }

    fn replace_config(&self, next: AppConfig) -> Result<()> {
        self.persist_config(&next)?;
        let grid_count = next.grids.len();
        *self.config.write() = next;
        info!(grid_count, "api configuration cache replaced");
        Ok(())
    }

    fn simulation(&self) -> Option<Arc<dyn GridSimulationControl>> {
        self.simulation.as_ref().map(Arc::clone)
    }

    fn persist_config(&self, config: &AppConfig) -> Result<()> {
        let serialised = toml::to_string_pretty(config)
            .context("failed to serialise configuration for persistence")?;
        fs::write(&self.config_path, serialised).with_context(|| {
            format!(
                "failed to write configuration file {}",
                self.config_path.display()
            )
        })?;
        Ok(())
    }

    fn log_snapshot(&self, error_limit: usize) -> Result<LogOverview> {
        if !self.log_dir.exists() {
            return Ok(LogOverview {
                files: Vec::new(),
                recent_errors: Vec::new(),
            });
        }

        let mut file_entries = Vec::new();
        for entry in fs::read_dir(&self.log_dir)
            .with_context(|| format!("unable to read log directory {}", self.log_dir.display()))?
        {
            let entry = entry.with_context(|| {
                format!("failed to iterate log directory {}", self.log_dir.display())
            })?;
            if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                let metadata = entry.metadata().with_context(|| {
                    format!("unable to stat log file {}", entry.path().display())
                })?;
                file_entries.push((entry.path(), metadata));
            }
        }

        file_entries.sort_by(|a, b| {
            let a_time = a.1.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let b_time = b.1.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            b_time.cmp(&a_time)
        });

        let files = file_entries
            .iter()
            .map(|(path, metadata)| LogFileSummary {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string()),
                path: path.display().to_string(),
                size_bytes: metadata.len(),
                modified: metadata.modified().ok().map(format_system_time),
            })
            .collect::<Vec<_>>();

        let mut recent_errors = Vec::new();
        for (path, _) in file_entries.iter().take(3) {
            let entries = collect_recent_errors(path, error_limit)?;
            recent_errors.extend(entries);
            if recent_errors.len() >= error_limit {
                break;
            }
        }

        recent_errors.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        recent_errors.reverse();
        recent_errors.truncate(error_limit);

        Ok(LogOverview {
            files,
            recent_errors,
        })
    }
}

impl fmt::Debug for ApiState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiState")
            .field("version", &self.version)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

/// Handle to the running API server.
#[derive(Debug)]
pub struct ApiServer {
    addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<()>>,
}

impl ApiServer {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        match self.task.await {
            Ok(result) => result,
            Err(err) => Err(err.into()),
        }
    }
}

/// Spawn the REST API with optional static asset hosting.
pub fn spawn_api_server(
    state: Arc<ApiState>,
    addr: SocketAddr,
    static_dir: Option<PathBuf>,
) -> Result<ApiServer> {
    let api_routes = Router::new()
        .route("/api/status", get(get_status))
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/logs", get(get_logs))
        .route("/api/sim/fault", post(post_sim_fault))
        .with_state(state);

    let router = if let Some(dir) = static_dir {
        let service = get_service(ServeDir::new(dir).append_index_html_on_directories(true));
        Router::new()
            .merge(api_routes)
            .fallback_service(service)
            .layer(TraceLayer::new_for_http())
    } else {
        api_routes.layer(TraceLayer::new_for_http())
    };

    let listener = StdTcpListener::bind(addr)
        .with_context(|| format!("failed to bind API listener {addr}"))?;
    listener
        .set_nonblocking(true)
        .context("failed to configure API listener as non-blocking")?;
    let tcp_listener =
        TcpListener::from_std(listener).context("failed to create tokio listener")?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let handle: JoinHandle<Result<()>> = tokio::spawn(async move {
        info!(address = %addr, "api server listening");
        if let Err(err) = axum::serve(tcp_listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
        {
            error!(address = %addr, error = %err, "api server exited with error");
            return Err(err.into());
        }
        Ok(())
    });

    Ok(ApiServer {
        addr,
        shutdown: Some(shutdown_tx),
        task: handle,
    })
}

#[derive(Debug, Serialize)]
struct StatusResponse {
    mode: Mode,
    version: String,
    git_commit: String,
    uptime_seconds: u64,
    grid_count: usize,
    controller_count: usize,
    features: BTreeMap<String, bool>,
}

#[derive(Debug, Serialize)]
struct ConfigAck {
    applied: bool,
}

#[derive(Debug, Serialize)]
struct LogFileSummary {
    name: String,
    path: String,
    size_bytes: u64,
    modified: Option<String>,
}

#[derive(Debug, Serialize)]
struct LogErrorEntry {
    timestamp: String,
    level: String,
    message: String,
    file: Option<String>,
    line: Option<u32>,
    target: Option<String>,
    source: String,
    raw: String,
}

#[derive(Debug, Serialize)]
struct LogOverview {
    files: Vec<LogFileSummary>,
    recent_errors: Vec<LogErrorEntry>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    message: String,
}

#[derive(Debug, Serialize)]
struct SimFaultResponse {
    applied: bool,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            message: self.message,
        });
        (self.status, body).into_response()
    }
}

async fn get_status(State(state): State<Arc<ApiState>>) -> Json<StatusResponse> {
    Json(state.status())
}

async fn get_config(State(state): State<Arc<ApiState>>) -> Json<AppConfig> {
    Json(state.config_snapshot())
}

async fn get_logs(State(state): State<Arc<ApiState>>) -> Result<Json<LogOverview>, ApiError> {
    state
        .log_snapshot(RECENT_ERROR_LIMIT)
        .map(Json)
        .map_err(|err| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

async fn put_config(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<AppConfig>,
) -> Result<Json<ConfigAck>, ApiError> {
    payload
        .validate()
        .map_err(|err| ApiError::new(StatusCode::BAD_REQUEST, err.to_string()))?;
    state
        .replace_config(payload)
        .map_err(|err| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(Json(ConfigAck { applied: true }))
}

#[derive(Debug, Deserialize)]
struct SimFaultRequest {
    component_id: Uuid,
    fault: FaultKind,
}

async fn post_sim_fault(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SimFaultRequest>,
) -> Result<(StatusCode, Json<SimFaultResponse>), ApiError> {
    if let Some(simulation) = state.simulation() {
        simulation
            .inject_fault(request.component_id, request.fault)
            .map_err(|err| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("unable to inject fault: {err}"),
                )
            })?;
        Ok((
            StatusCode::ACCEPTED,
            Json(SimFaultResponse { applied: true }),
        ))
    } else {
        Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "simulation control unavailable",
        ))
    }
}

fn collect_recent_errors(path: &Path, limit: usize) -> Result<Vec<LogErrorEntry>> {
    let file =
        File::open(path).with_context(|| format!("failed to open log file {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut ring: VecDeque<LogErrorEntry> = VecDeque::with_capacity(limit);
    for line in reader.lines() {
        let line =
            line.with_context(|| format!("failed to read log entry from {}", path.display()))?;
        if let Some(mut entry) = parse_error_entry(&line) {
            entry.source = path.display().to_string();
            if ring.len() == limit {
                ring.pop_front();
            }
            ring.push_back(entry);
        }
    }
    Ok(ring.into_iter().collect())
}

fn parse_error_entry(line: &str) -> Option<LogErrorEntry> {
    let value: Value = serde_json::from_str(line).ok()?;
    let level = value.get("level")?.as_str()?.to_string();
    if level.to_ascii_uppercase() != "ERROR" {
        return None;
    }

    let timestamp = value
        .get("timestamp")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let fields = value.get("fields").and_then(|fields| fields.as_object());
    let message = fields
        .and_then(|map| map.get("message"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let file = fields
        .and_then(|map| map.get("log.file"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let line_no = fields
        .and_then(|map| map.get("log.line"))
        .and_then(|v| v.as_u64())
        .map(|value| value as u32);
    let target = value
        .get("target")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(LogErrorEntry {
        timestamp,
        level,
        message,
        file,
        line: line_no,
        target,
        source: String::new(),
        raw: line.to_string(),
    })
}

fn format_system_time(time: SystemTime) -> String {
    DateTime::<Utc>::from(time).to_rfc3339()
}
