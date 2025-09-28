//! ---
//! ems_section: "05-networking-external-interfaces"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Network connectivity and edge adapters."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use prometheus::{Registry, TextEncoder};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Snapshot of system health returned by the status endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusSnapshot {
    /// Current orchestrator mode (production, simulation, hybrid).
    pub mode: String,
    /// Workspace revision or build hash associated with the runtime.
    pub revision: String,
    /// Aggregated grid statuses.
    pub grids: Vec<GridStatus>,
    /// Optional URL pointing to the metrics endpoint.
    pub metrics_endpoint: Option<String>,
}

/// Per-grid summary information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GridStatus {
    /// Grid identifier.
    pub id: String,
    /// Controllers belonging to this grid.
    pub controllers: Vec<ControllerStatus>,
}

/// Per-controller health record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ControllerStatus {
    /// Controller identifier.
    pub id: String,
    /// Role within redundancy strategy.
    pub role: String,
    /// Whether the controller is currently healthy.
    pub healthy: bool,
    /// Milliseconds since the latest heartbeat.
    pub last_heartbeat_ms: u64,
}

/// REST command payload accepted by `/commands`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandRequest {
    /// Target resource (grid/controller path).
    pub target: String,
    /// Command verb to execute.
    pub command: String,
    #[serde(default)]
    /// Optional structured payload forwarded to the handler.
    pub parameters: serde_json::Value,
}

/// Response emitted after processing a command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandResponse {
    /// True when the command has been accepted for processing.
    pub accepted: bool,
    /// Human readable feedback about the decision or outcome.
    pub message: String,
}

/// Error wrapper for command handling failures.
/// Errors that can be returned by the [`CommandHandler`].
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// Raised when the authoriser rejects the request.
    #[error("command not authorised")]
    NotAuthorised,
    /// Indicates the payload failed validation.
    #[error("invalid command payload: {0}")]
    InvalidPayload(String),
    /// Represents downstream execution failure.
    #[error("command execution failed: {0}")]
    ExecutionFailed(String),
}

/// Provides snapshots for the status endpoint.
pub trait StatusProvider: Send + Sync + 'static {
    /// Return the snapshot that should be emitted by the `/status` endpoint.
    fn snapshot(&self) -> StatusSnapshot;
}

/// Handles command execution requests.
#[async_trait]
pub trait CommandHandler: Send + Sync + 'static {
    /// Execute a command request scoped to the provided principal/API key.
    async fn handle_command(
        &self,
        principal: &str,
        request: CommandRequest,
    ) -> Result<CommandResponse, CommandError>;
}

/// Authorises command requests based on an API key.
pub trait CommandAuthoriser: Send + Sync + 'static {
    /// Determine whether the API key may issues the supplied request.
    fn authorise(&self, api_key: &str, request: &CommandRequest) -> bool;
}

/// Shared state injected into the axum handlers.
struct RestState {
    provider: Arc<dyn StatusProvider>,
    handler: Arc<dyn CommandHandler>,
    authoriser: Arc<dyn CommandAuthoriser>,
    metrics: Option<Arc<Registry>>,
}

/// Builder used to configure and spawn the REST API server.
#[derive(Clone)]
pub struct RestApiBuilder {
    listen: SocketAddr,
    provider: Arc<dyn StatusProvider>,
    handler: Arc<dyn CommandHandler>,
    authoriser: Arc<dyn CommandAuthoriser>,
    metrics: Option<Arc<Registry>>,
}

impl RestApiBuilder {
    /// Construct a new builder from mandatory components.
    pub fn new(
        listen: SocketAddr,
        provider: Arc<dyn StatusProvider>,
        handler: Arc<dyn CommandHandler>,
        authoriser: Arc<dyn CommandAuthoriser>,
    ) -> Self {
        Self {
            listen,
            provider,
            handler,
            authoriser,
            metrics: None,
        }
    }

    /// Attach a Prometheus registry exposed at `/metrics`.
    pub fn with_metrics_registry(mut self, registry: Arc<Registry>) -> Self {
        self.metrics = Some(registry);
        self
    }

    /// Spawn the REST API server and return a handle that can be awaited for shutdown.
    pub async fn spawn(self) -> anyhow::Result<RestApiHandle> {
        let listener = TcpListener::bind(self.listen).await?;
        let local_addr = listener.local_addr()?;
        info!(address = %local_addr, "rest api listening");

        let state = RestState {
            provider: self.provider,
            handler: self.handler,
            authoriser: self.authoriser,
            metrics: self.metrics,
        };
        let router = Router::new()
            .route("/status", get(get_status))
            .route("/metrics", get(get_metrics))
            .route("/commands", post(post_command))
            .with_state(Arc::new(state));

        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let server = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        });
        let task = tokio::spawn(async move {
            if let Err(err) = server.await {
                warn!(error = %err, "rest api server exited with error");
            }
        });

        Ok(RestApiHandle {
            address: local_addr,
            task,
            shutdown: shutdown_tx,
        })
    }
}

/// Handle returned from [`RestApiBuilder::spawn`] allowing the caller to await server completion.
pub struct RestApiHandle {
    address: SocketAddr,
    task: JoinHandle<()>,
    shutdown: watch::Sender<bool>,
}

impl RestApiHandle {
    /// Retrieve the socket address the server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }

    /// Request graceful shutdown and wait for the server task to finish.
    pub async fn shutdown(self) -> anyhow::Result<()> {
        let _ = self.shutdown.send(true);
        match self.task.await {
            Ok(()) => Ok(()),
            Err(join) => Err(anyhow::anyhow!(join)),
        }
    }
}

async fn get_status(State(state): State<Arc<RestState>>) -> Json<StatusSnapshot> {
    let snapshot = state.provider.snapshot();
    Json(snapshot)
}

async fn get_metrics(State(state): State<Arc<RestState>>) -> Response {
    let Some(registry) = &state.metrics else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "metrics registry unavailable",
        )
            .into_response();
    };

    let encoder = TextEncoder::new();
    let families = registry.gather();
    match encoder.encode_to_string(&families) {
        Ok(body) => (StatusCode::OK, body).into_response(),
        Err(err) => {
            warn!(error = %err, "failed to encode metrics");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn post_command(
    State(state): State<Arc<RestState>>,
    headers: axum::http::HeaderMap,
    Json(request): Json<CommandRequest>,
) -> Response {
    let Some(api_key) = extract_api_key(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    if !state.authoriser.authorise(&api_key, &request) {
        return StatusCode::FORBIDDEN.into_response();
    }

    match state.handler.handle_command(&api_key, request).await {
        Ok(response) => (StatusCode::ACCEPTED, Json(response)).into_response(),
        Err(CommandError::NotAuthorised) => StatusCode::FORBIDDEN.into_response(),
        Err(CommandError::InvalidPayload(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
        Err(CommandError::ExecutionFailed(msg)) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
    }
}

fn extract_api_key(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get("x-api-key")
        .or_else(|| headers.get(axum::http::header::AUTHORIZATION))
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().trim_start_matches("Bearer ").to_owned())
}

/// Simple in-memory authoriser that validates API keys against an allow list.
/// Fixed API key authoriser backed by an in-memory map.
#[derive(Debug, Clone)]
pub struct StaticApiKeyAuthoriser {
    keys: Arc<std::collections::HashMap<String, Vec<String>>>,
}

impl StaticApiKeyAuthoriser {
    /// Create an authoriser from a mapping of API key to allowed commands.
    pub fn new(entries: impl IntoIterator<Item = (String, Vec<String>)>) -> Self {
        Self {
            keys: Arc::new(entries.into_iter().collect()),
        }
    }
}

impl CommandAuthoriser for StaticApiKeyAuthoriser {
    fn authorise(&self, api_key: &str, request: &CommandRequest) -> bool {
        let Some(permissions) = self.keys.get(api_key) else {
            debug!(api_key, "api key rejected");
            return false;
        };
        let allowed = permissions
            .iter()
            .any(|perm| perm == "*" || perm == &request.command);
        if !allowed {
            debug!(api_key, command = %request.command, "api key lacks permission for command");
        }
        allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus::{IntCounter, Opts, Registry};
    use reqwest::Client;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{sleep, Duration};

    struct TestStatus;
    impl StatusProvider for TestStatus {
        fn snapshot(&self) -> StatusSnapshot {
            StatusSnapshot {
                mode: "simulation".into(),
                revision: "abc123".into(),
                grids: vec![GridStatus {
                    id: "grid-a".into(),
                    controllers: vec![ControllerStatus {
                        id: "a1".into(),
                        role: "primary".into(),
                        healthy: true,
                        last_heartbeat_ms: 10,
                    }],
                }],
                metrics_endpoint: None,
            }
        }
    }

    struct TestHandler {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl CommandHandler for TestHandler {
        async fn handle_command(
            &self,
            principal: &str,
            request: CommandRequest,
        ) -> Result<CommandResponse, CommandError> {
            assert_eq!(principal, "secret");
            assert_eq!(request.command, "restart-controller");
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(CommandResponse {
                accepted: true,
                message: "restart initiated".into(),
            })
        }
    }

    #[tokio::test]
    async fn status_endpoint_returns_snapshot() {
        let registry = Registry::new();
        let counter = IntCounter::with_opts(Opts::new("test_metric", "demo")).unwrap();
        registry.register(Box::new(counter.clone())).unwrap();
        counter.inc();

        let builder = RestApiBuilder::new(
            "127.0.0.1:0".parse().unwrap(),
            Arc::new(TestStatus),
            Arc::new(TestHandler {
                calls: AtomicUsize::new(0),
            }),
            Arc::new(StaticApiKeyAuthoriser::new([(
                "secret".into(),
                vec!["*".into()],
            )])),
        )
        .with_metrics_registry(Arc::new(registry));

        let handle = builder.spawn().await.unwrap();
        let client = Client::new();
        let base = format!("http://{}", handle.local_addr());

        let status: StatusSnapshot = client
            .get(format!("{base}/status"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        assert_eq!(status.mode, "simulation");

        let metrics = client
            .get(format!("{base}/metrics"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(metrics.contains("test_metric"));

        let command_resp = client
            .post(format!("{base}/commands"))
            .header("x-api-key", "secret")
            .json(&json!({
                "target": "grid-a:a1",
                "command": "restart-controller",
                "parameters": {
                    "delay_ms": 100
                }
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(command_resp.status(), StatusCode::ACCEPTED);

        let forbidden = client
            .post(format!("{base}/commands"))
            .header("x-api-key", "invalid")
            .json(&json!({
                "target": "grid-a:a1",
                "command": "restart-controller",
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        sleep(Duration::from_millis(20)).await;
        drop(client);
        handle.shutdown().await.unwrap();
    }

    #[test]
    fn static_authoriser_controls_permissions() {
        let mut perms = HashMap::new();
        perms.insert("key-a".to_string(), vec!["restart".to_string()]);
        perms.insert("key-b".to_string(), vec!["*".to_string()]);

        let auth = StaticApiKeyAuthoriser::new(perms);
        let request = CommandRequest {
            target: "grid-a".into(),
            command: "restart".into(),
            parameters: serde_json::Value::Null,
        };
        assert!(auth.authorise("key-a", &request));
        assert!(auth.authorise("key-b", &request));

        let other = CommandRequest {
            target: "grid-a".into(),
            command: "shutdown".into(),
            parameters: serde_json::Value::Null,
        };
        assert!(!auth.authorise("key-a", &other));
        assert!(auth.authorise("key-b", &other));
    }
}
