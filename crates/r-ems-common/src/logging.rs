//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Shared primitives and utilities for the core runtime."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use anyhow::Result;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tracing::info;
use tracing_appender::rolling::daily;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::{Layer, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;

use crate::config::LoggingConfig;

const LOG_ENV: &str = "R-EMS_LOG";

static FILE_GUARD: OnceCell<tracing_appender::non_blocking::WorkerGuard> = OnceCell::new();
static STDOUT_GUARD: OnceCell<tracing_appender::non_blocking::WorkerGuard> = OnceCell::new();

/// Available log formats for the daemon.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum LogFormat {
    #[default]
    StructuredJson,
    Pretty,
}

/// Initialize the tracing subscriber based on configuration and environment variables.
///
/// * `R-EMS_LOG` can be set to override the log filter (e.g. `info`, `debug,foo=trace`).
///   When unset the standard `RUST_LOG` variable is honoured, finally defaulting to
///   `debug` to aid troubleshooting.
/// * Structured JSON is emitted to stdout by default which keeps container logs tidy,
///   while a rolling daily log file is created for production post-mortem analysis.
pub fn init_tracing(service_name: &str, config: &LoggingConfig) -> Result<()> {
    std::fs::create_dir_all(&config.directory)?;
    let prefix = config
        .file_prefix
        .clone()
        .unwrap_or_else(|| service_name.to_owned());

    let file_appender = daily(
        &config.directory,
        format!("{}-{}.log", prefix, service_name),
    );
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);
    let (stdout_writer, stdout_guard) = tracing_appender::non_blocking(std::io::stdout());

    let _ = FILE_GUARD.set(file_guard);
    let _ = STDOUT_GUARD.set(stdout_guard);

    // Honour the custom `R-EMS_LOG` directive first. If it is missing we fall back to the
    // standard `RUST_LOG` environment variable. Finally, default to `debug` so that
    // engineers always receive verbose diagnostics during development and early bring-up.
    let filter = match std::env::var(LOG_ENV) {
        Ok(directive) => EnvFilter::try_new(directive).unwrap_or_else(|err| {
            eprintln!(
                "invalid {} directive ({}); defaulting to debug logging",
                LOG_ENV, err
            );
            EnvFilter::new("debug")
        }),
        Err(_) => EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")),
    };

    let fmt_layer = match config.format {
        LogFormat::StructuredJson => fmt::layer()
            .with_target(false)
            .with_timer(fmt::time::UtcTime::rfc_3339())
            .json()
            .with_writer(stdout_writer)
            .boxed(),
        LogFormat::Pretty => fmt::layer()
            .with_target(true)
            .with_timer(fmt::time::UtcTime::rfc_3339())
            .with_writer(stdout_writer)
            .boxed(),
    };

    let file_layer = fmt::layer()
        .with_target(true)
        .with_timer(fmt::time::UtcTime::rfc_3339())
        .json()
        .with_writer(file_writer)
        .boxed();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(file_layer)
        .try_init()
        .ok();

    info!(service = %service_name, log_dir = %config.directory.display(), format = ?config.format, "tracing initialised");
    Ok(())
}
