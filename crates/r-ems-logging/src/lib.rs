//! ---
//! ems_section: "03-persistence-logging"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Structured logging adapters and sinks."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
#![warn(missing_docs)]

use tracing::Level;
use tracing_subscriber::{fmt as subscriber_fmt, prelude::*, EnvFilter, Registry};

pub mod macros;

/// Initialize a baseline tracing subscriber suitable for development.
pub fn init() {
    let _ = Registry::default()
        .with(EnvFilter::from_default_env().add_directive(Level::INFO.into()))
        .with(subscriber_fmt::layer())
        .try_init();
}

/// Structured logging context propagated by the convenience macros.
#[derive(Debug, Default, Clone)]
pub struct LogContext<'a> {
    /// Grid identifier associated with the log event.
    pub grid: Option<&'a str>,
    /// Controller identifier associated with the log event.
    pub controller: Option<&'a str>,
    /// Discrete tick or sequence number.
    pub tick: Option<u64>,
    /// Operating mode (production, simulation, etc.).
    pub mode: Option<&'a str>,
}

impl<'a> LogContext<'a> {
    /// Create an empty logging context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a grid identifier.
    pub fn with_grid(mut self, grid: &'a str) -> Self {
        self.grid = Some(grid);
        self
    }

    /// Attach a controller identifier.
    pub fn with_controller(mut self, controller: &'a str) -> Self {
        self.controller = Some(controller);
        self
    }

    /// Attach a tick value.
    pub fn with_tick(mut self, tick: u64) -> Self {
        self.tick = Some(tick);
        self
    }

    /// Attach an operating mode descriptor.
    pub fn with_mode(mut self, mode: &'a str) -> Self {
        self.mode = Some(mode);
        self
    }
}

/// High-level outcome used when emitting lifecycle log events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemEventOutcome {
    /// The operation completed successfully.
    Success,
    /// The operation failed or was aborted.
    Fault,
}

impl SystemEventOutcome {
    fn as_str(&self) -> &'static str {
        match self {
            SystemEventOutcome::Success => "success",
            SystemEventOutcome::Fault => "fault",
        }
    }

    fn level(&self) -> Level {
        match self {
            SystemEventOutcome::Success => Level::INFO,
            SystemEventOutcome::Fault => Level::ERROR,
        }
    }
}

/// Emit a standardized system event with a success/fault outcome.
pub fn log_system_event(
    context: Option<&LogContext>,
    event: &str,
    message: &str,
    outcome: SystemEventOutcome,
) {
    let ctx = context.unwrap_or(&LogContext::default());
    let level = outcome.level();
    tracing::event!(
        level,
        event,
        outcome = outcome.as_str(),
        grid = ctx.grid.unwrap_or(""),
        controller = ctx.controller.unwrap_or(""),
        tick = ctx.tick.unwrap_or_default(),
        mode = ctx.mode.unwrap_or(""),
        message = %message
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macros_emit_without_panic() {
        init();
        let ctx = LogContext::new()
            .with_grid("grid-a")
            .with_controller("ctrl-a");
        ems_info!(context = ctx.clone(), "controller online");
        ems_debug!("debug message");
        ems_error!(context = ctx, "error code: {}", 42);
    }

    #[test]
    fn init_does_not_panic() {
        init();
    }

    #[test]
    fn system_event_helper_emits() {
        init();
        let ctx = LogContext::new().with_grid("grid-a");
        log_system_event(
            Some(&ctx),
            "test.event",
            "system event helper executed",
            SystemEventOutcome::Success,
        );
        log_system_event(
            None,
            "test.event",
            "system event helper fault",
            SystemEventOutcome::Fault,
        );
    }
}
