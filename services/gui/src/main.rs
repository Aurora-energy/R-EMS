//! ---------------------------------------------------------------------------
//! R-EMS Core GUI Service
//! ---------------------------------------------------------------------------
//! This binary provides the web-based management interface for the R-EMS Core
//! platform. It is intentionally implemented as an all-Rust stack using Axum
//! for HTTP routing and Askama for server-side rendered templates so that no
//! separate Node.js toolchain is required for building or running the GUI.
//!
//! The service exposes several routes that interact with the rest of the R-EMS
//! platform via HTTP calls. Each handler is extensively documented to clarify
//! the reasoning behind the control flow and the security decisions, which is
//! especially important for system operators reviewing the code prior to
//! deployment in high-availability energy environments.
//!
//! The module also demonstrates the overall commenting style required across
//! the repository: every non-trivial line is annotated with context so that new
//! contributors (and future auditors) can understand the intent without
//! reverse-engineering control flow.

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use askama::Template;
use axum::{
    extract::{Extension, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use pulldown_cmark::{html, Parser};
use reqwest::Client;
use serde::Deserialize;
use thiserror::Error;
use tokio::{fs, net::TcpListener, signal};
use tower::ServiceBuilder;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use tracing::{error, info, instrument};

/// Environment variable that controls which address the server binds to.
/// Using an environment variable rather than a config file keeps the
/// container image generic while still allowing operators to customize the
/// port at runtime.
const ENV_BIND_ADDR: &str = "REMS_GUI_BIND";

/// Environment variable containing a comma-separated list of internal service
/// base URLs that expose health endpoints. The GUI pings these to display the
/// overview status dashboard.
const ENV_HEALTH_ENDPOINTS: &str = "REMS_GUI_HEALTH_ENDPOINTS";

/// Environment variable pointing to the directory where Markdown help docs
/// are mounted. The directory is served read-only to avoid operators editing
/// documentation through the web UI.
const ENV_DOCS_ROOT: &str = "REMS_GUI_DOCS_ROOT";

/// Default listen address used if the `REMS_GUI_BIND` environment variable is
/// not supplied. Binding to `0.0.0.0:8080` matches the compose file defaults
/// and works out of the box inside containers.
const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";

/// Default documentation directory that is used when the env var is absent.
const DEFAULT_DOCS_ROOT: &str = "/srv/r-ems/docs";

/// Placeholder token used in the rendered template where the Markdown HTML body
/// should be injected. Askama escapes all template variables by default, so we
/// replace this token with the rendered Markdown after the template is
/// rendered to avoid double-escaping while still keeping titles escaped.
const MARKDOWN_SENTINEL: &str = "__REMS_MARKDOWN_BODY__";

/// Structure representing configuration derived from environment variables.
/// Using a dedicated struct keeps the handler code clean and allows the config
/// to be shared via `Arc`.
#[derive(Clone, Debug)]
struct AppConfig {
    /// Address the Axum server listens on.
    bind_addr: SocketAddr,
    /// Collection of health endpoints to poll.
    health_endpoints: Vec<String>,
    /// Root directory for Markdown documentation files.
    docs_root: PathBuf,
}

/// Shared application state stored in an `Arc` so it can be cloned cheaply and
/// injected into each handler through Axum's `Extension` extractor.
#[derive(Clone)]
struct AppState {
    /// Static configuration loaded at startup.
    config: Arc<AppConfig>,
    /// Reusable HTTP client used to reach sibling services. Sharing a client
    /// is important because it keeps TCP connections pooled and reduces load
    /// on the other services.
    client: Client,
}

/// Custom error type used across the module. We implement `IntoResponse` so
/// handlers can propagate errors via the `?` operator while still returning a
/// well-formed HTTP response to the browser.
#[derive(Debug, Error)]
enum AppError {
    /// Wraps lower-level errors with context for easier debugging. Using
    /// `anyhow::Error` gives us automatic backtraces when enabled.
    #[error("internal server error: {0}")]
    Internal(#[from] anyhow::Error),

    /// Represents attempts to access files outside of the documentation root.
    #[error("forbidden path access")]
    Forbidden,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            // Internal errors are mapped to HTTP 500 while logging the full
            // context. We avoid exposing detailed error messages to the client
            // to prevent information leaks.
            AppError::Internal(err) => {
                error!(error = %err, "serving 500 due to internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
            }
            // Forbidden file access attempts result in HTTP 403 so that the
            // browser understands the request was understood but disallowed.
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden").into_response(),
        }
    }
}

/// Minimal JSON response for the `/healthz` endpoint.
#[derive(serde::Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// Query parameters accepted by the `/help/file` route. We only allow a single
/// `path` value to keep the API surface small.
#[derive(Deserialize)]
struct HelpFileQuery {
    path: String,
}

/// Askama template describing the overview page. Templates are defined using
/// Rust structs so that the compiler verifies that all variables used within
/// the HTML exist and have the expected types.
#[derive(Template)]
#[template(path = "overview.html")]
struct OverviewTemplate<'a> {
    /// Collection of service health results displayed in a table.
    services: &'a [ServiceHealth],
}

/// Serializable structure describing the health of a single service. The
/// structure doubles as the model passed into the Askama template and the JSON
/// payload returned to HTMX partial requests.
#[derive(Clone, Debug, serde::Serialize)]
struct ServiceHealth {
    name: String,
    status: String,
}

/// Template for the help index page.
#[derive(Template)]
#[template(path = "help_index.html")]
struct HelpIndexTemplate {
    entries: Vec<HelpEntry>,
}

/// Template for rendering a specific help file.
#[derive(Template)]
#[template(path = "help_file.html")]
struct HelpFileTemplate {
    title: String,
    body_placeholder: &'static str,
}

/// Description of a single help file, used by the help index template.
#[derive(Clone)]
struct HelpEntry {
    title: String,
    href: String,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Install a default tracing subscriber so logs emitted with `tracing::info`
    // and friends show up on stdout. We keep the configuration minimal (only
    // INFO level) because operators can override it by setting the
    // `RUST_LOG` environment variable when starting the container.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    // Load configuration from environment variables and wrap it in an `Arc`
    // so we can share it across request handlers.
    let config = Arc::new(load_config()?);

    info!(?config, "starting R-EMS GUI service");

    // Build the reusable HTTP client with sane defaults. We set a relatively
    // short timeout so that slow downstream services do not cause the GUI to
    // hang indefinitely.
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .context("failed to build reqwest client")?;

    // Construct the Axum router that wires routes to handlers. We use
    // middleware layers for tracing and compression so that all responses are
    // gzip-compressed automatically when the client supports it.
    let app_state = AppState {
        config: config.clone(),
        client,
    };

    let router = Router::new()
        // Dashboard view listing the health of core services.
        .route("/", get(overview))
        // JSON endpoint returning the same data consumed by HTMX components.
        .route("/api/overview", get(overview_json))
        // Plugin management surface, currently rendered statically. The actual
        // plugin operations are stubbed until the registry API is implemented.
        .route("/plugins", get(plugins))
        .route("/config", get(config_view))
        .route("/ha", get(ha_status))
        .route("/help", get(help_index))
        .route("/help/file", get(help_file))
        .route("/healthz", get(healthz))
        // Expose a placeholder metrics endpoint that can later be wired into a
        // real metrics registry.
        .route("/metrics", get(metrics))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new()),
        )
        .with_state(app_state.clone())
        .layer(Extension(app_state));

    // Extract the socket address from the configuration for binding.
    let addr = config.bind_addr;

    info!(%addr, "listening for HTTP traffic");

    // Start the Axum server and also listen for termination signals so that we
    // can gracefully shut down when the container receives SIGTERM.
    let listener = TcpListener::bind(addr)
        .await
        .context("binding TCP listener")?;

    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum server error")
}

/// Waits for either CTRL+C (when running locally) or SIGTERM (inside Docker)
/// to trigger graceful shutdown. Using `tokio::select!` avoids missing a signal
/// on platforms that expose only one of these mechanisms.
#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{signal as unix_signal, SignalKind};

    let mut sigterm = unix_signal(SignalKind::terminate()).expect("install SIGTERM handler");

    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("received ctrl_c; shutting down");
        }
        _ = sigterm.recv() => {
            info!("received SIGTERM; shutting down");
        }
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = signal::ctrl_c().await;
    info!("received ctrl_c; shutting down");
}

/// Loads configuration from environment variables with sensible defaults.
fn load_config() -> Result<AppConfig, anyhow::Error> {
    // Read the bind address from the environment and parse it into a
    // `SocketAddr`. We use `with_context` to provide the original string in the
    // error message if parsing fails.
    let bind_addr = std::env::var(ENV_BIND_ADDR)
        .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string())
        .parse::<SocketAddr>()
        .with_context(|| format!("invalid {ENV_BIND_ADDR} value"))?;

    // Parse the list of health endpoints by splitting on commas, trimming
    // whitespace, and discarding empty strings. This allows operators to leave
    // the variable unset without breaking the GUI.
    let health_endpoints = std::env::var(ENV_HEALTH_ENDPOINTS)
        .unwrap_or_else(|_| String::new())
        .split(',')
        .filter_map(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect::<Vec<_>>();

    // Determine where the documentation files live. We resolve the path to an
    // absolute form so that the path traversal checks later can work with a
    // canonical base directory.
    let docs_root = PathBuf::from(
        std::env::var(ENV_DOCS_ROOT).unwrap_or_else(|_| DEFAULT_DOCS_ROOT.to_string()),
    )
    .canonicalize()
    .unwrap_or_else(|_| PathBuf::from(DEFAULT_DOCS_ROOT));

    Ok(AppConfig {
        bind_addr,
        health_endpoints,
        docs_root,
    })
}

/// Handler serving the overview dashboard.
#[instrument(skip_all, fields(num_endpoints = state.config.health_endpoints.len()))]
async fn overview(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    // Gather the health status of core services and render the HTML template.
    let services = gather_health(&state).await?;
    let template = OverviewTemplate {
        services: &services,
    };

    let body = template.render().context("render overview template")?;

    Ok(Html(body))
}

/// JSON variant of the overview endpoint used by HTMX to periodically refresh
/// status tables without reloading the entire page.
#[instrument(skip_all)]
async fn overview_json(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let services = gather_health(&state).await?;
    Ok(Json(services))
}

/// Helper function that queries each configured health endpoint and returns a
/// vector describing their status. The function intentionally never fails hard
/// on individual endpoints; it logs errors and marks services as unhealthy so
/// the UI can still render partial data.
async fn gather_health(state: &AppState) -> Result<Vec<ServiceHealth>, AppError> {
    let mut results = Vec::new();

    for endpoint in &state.config.health_endpoints {
        // Each endpoint is polled sequentially to keep the implementation
        // simple. If needed, this can be upgraded to concurrent requests via
        // `futures::future::join_all` without changing the observable API.
        match state.client.get(endpoint).send().await {
            Ok(resp) => {
                let status = if resp.status().is_success() {
                    "healthy"
                } else {
                    "unhealthy"
                };
                results.push(ServiceHealth {
                    name: endpoint.clone(),
                    status: status.to_string(),
                });
            }
            Err(err) => {
                error!(?err, endpoint, "failed to query health endpoint");
                results.push(ServiceHealth {
                    name: endpoint.clone(),
                    status: "error".to_string(),
                });
            }
        }
    }

    Ok(results)
}

/// Static placeholder for the plugins page. Once the registry API is
/// available this handler will call it to fetch real plugin data.
#[instrument]
async fn plugins() -> Result<impl IntoResponse, AppError> {
    let body = "<html><body><!-- Plugin page stub -->
        <h1>Plugins</h1>
        <p>The plugin registry integration will populate this page.</p>
    </body></html>";
    Ok(Html(body))
}

/// Handler rendering the configuration page. The page currently displays a
/// placeholder because the configuration service is not yet wired in.
#[instrument]
async fn config_view() -> Result<impl IntoResponse, AppError> {
    let body = "<html><body><!-- Config page stub -->
        <h1>Configuration</h1>
        <p>The configd service will provide live configuration snapshots.</p>
    </body></html>";
    Ok(Html(body))
}

/// Handler for the high-availability status page.
#[instrument]
async fn ha_status() -> Result<impl IntoResponse, AppError> {
    let body = "<html><body><!-- HA page stub -->
        <h1>High Availability</h1>
        <p>HA orchestration details will appear here once implemented.</p>
    </body></html>";
    Ok(Html(body))
}

/// Renders the help index by listing Markdown files from the docs directory.
#[instrument(skip_all)]
async fn help_index(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let entries = list_help_entries(&state).await?;
    let template = HelpIndexTemplate { entries };
    let body = template.render().context("render help index")?;
    Ok(Html(body))
}

/// Reads and renders a specific help file requested by the browser.
#[instrument(skip_all, fields(path = %query.path))]
async fn help_file(
    State(state): State<AppState>,
    Query(query): Query<HelpFileQuery>,
) -> Result<impl IntoResponse, AppError> {
    // Sanitize the requested path to prevent directory traversal attacks where
    // a user might try `../../etc/passwd`. The sanitizer returns the canonical
    // path only if it remains within the docs root.
    let path = sanitize_path(&state.config.docs_root, &query.path)?;

    // Read the markdown file contents asynchronously.
    let markdown = fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read help file at {path:?}"))?;

    // Render the Markdown into HTML before sending it to the browser. Using a
    // local renderer avoids exposing raw Markdown (which might include HTML
    // tags) to the client.
    let mut html_output = String::new();
    let parser = Parser::new(&markdown);
    html::push_html(&mut html_output, parser);

    let title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Help");

    let template = HelpFileTemplate {
        title: title.to_string(),
        body_placeholder: MARKDOWN_SENTINEL,
    };

    let rendered = template.render().context("render help file")?;
    let body = rendered.replace(MARKDOWN_SENTINEL, &html_output);

    Ok(Html(body))
}

/// Simple health check endpoint returning JSON for ease of monitoring.
#[instrument]
async fn healthz() -> Result<impl IntoResponse, AppError> {
    Ok(Json(HealthResponse { status: "ok" }))
}

/// Placeholder metrics endpoint. Once metrics are wired in this handler will
/// expose the Prometheus scrape output.
#[instrument]
async fn metrics() -> Result<impl IntoResponse, AppError> {
    Ok(Html("metrics not yet implemented"))
}

/// Enumerates all Markdown files in the documentation directory.
#[instrument(skip_all)]
async fn list_help_entries(state: &AppState) -> Result<Vec<HelpEntry>, AppError> {
    let mut entries = Vec::new();
    let mut dir = fs::read_dir(&state.config.docs_root)
        .await
        .with_context(|| format!("list docs dir {:#?}", state.config.docs_root))?;

    while let Some(entry) = dir
        .next_entry()
        .await
        .map_err(|err| AppError::Internal(err.into()))?
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            if let Ok(rel) = path.strip_prefix(&state.config.docs_root) {
                let relative_path = rel.to_string_lossy().replace('\\', "/");
                let href = format!("/help/file?path={}", urlencoding::encode(&relative_path));
                entries.push(HelpEntry {
                    title: entry
                        .file_name()
                        .to_string_lossy()
                        .trim_end_matches(".md")
                        .replace('_', " "),
                    href,
                });
            }
        }
    }

    entries.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(entries)
}

/// Sanitizes a requested help file path by ensuring it resolves within the
/// documentation root. Returning an error protects against attempts to read
/// arbitrary files on the filesystem.
fn sanitize_path(root: &Path, requested: &str) -> Result<PathBuf, AppError> {
    // Percent-decode the requested path so that encoded sequences like `%2e`
    // (which represents `.`) are correctly interpreted before normalization.
    let decoded = urlencoding::decode(requested)
        .map_err(|err| {
            error!(?err, requested, "failed to decode requested path");
            anyhow::anyhow!(err)
        })
        .map_err(AppError::Internal)?;

    // Join the decoded path onto the root. Using `join` ensures that even if
    // the user includes leading slashes, the resulting path remains rooted in
    // `root` because `PathBuf::join` discards the root component from the
    // argument when it is absolute.
    let candidate = root.join(decoded.as_ref());

    // Canonicalize the path to resolve `..` segments. If canonicalization
    // fails (e.g., the file doesn't exist), we bubble up the error so the
    // handler can respond with a 500 while logging context for operators.
    let canonical = candidate
        .canonicalize()
        .with_context(|| format!("canonicalize candidate path {candidate:?}"))
        .map_err(AppError::Internal)?;

    // Ensure the canonical path still starts with the root directory. If it
    // doesn't, a traversal attempt was made.
    if canonical.starts_with(root) {
        Ok(canonical)
    } else {
        Err(AppError::Forbidden)
    }
}
