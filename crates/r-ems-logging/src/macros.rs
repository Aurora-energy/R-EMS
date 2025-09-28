//! ---
//! ems_section: "03-persistence-logging"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Structured logging adapters and sinks."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
/// Emit an informational log enriched with R-EMS context.
#[macro_export]
macro_rules! ems_info {
    (context = $ctx:expr, $($arg:tt)+) => {{
        let ctx = &$ctx;
        tracing::event!(
            tracing::Level::INFO,
            grid = ctx.grid.unwrap_or(""),
            controller = ctx.controller.unwrap_or(""),
            tick = ctx.tick.unwrap_or_default(),
            mode = ctx.mode.unwrap_or(""),
            message = %format_args!($($arg)+)
        );
    }};
    ($($arg:tt)+) => {{
        let ctx = &$crate::LogContext::default();
        tracing::event!(
            tracing::Level::INFO,
            grid = ctx.grid.unwrap_or(""),
            controller = ctx.controller.unwrap_or(""),
            tick = ctx.tick.unwrap_or_default(),
            mode = ctx.mode.unwrap_or(""),
            message = %format_args!($($arg)+)
        );
    }};
}

/// Emit a debug log enriched with R-EMS context.
#[macro_export]
macro_rules! ems_debug {
    (context = $ctx:expr, $($arg:tt)+) => {{
        let ctx = &$ctx;
        tracing::event!(
            tracing::Level::DEBUG,
            grid = ctx.grid.unwrap_or(""),
            controller = ctx.controller.unwrap_or(""),
            tick = ctx.tick.unwrap_or_default(),
            mode = ctx.mode.unwrap_or(""),
            message = %format_args!($($arg)+)
        );
    }};
    ($($arg:tt)+) => {{
        let ctx = &$crate::LogContext::default();
        tracing::event!(
            tracing::Level::DEBUG,
            grid = ctx.grid.unwrap_or(""),
            controller = ctx.controller.unwrap_or(""),
            tick = ctx.tick.unwrap_or_default(),
            mode = ctx.mode.unwrap_or(""),
            message = %format_args!($($arg)+)
        );
    }};
}

/// Emit an error log enriched with R-EMS context.
#[macro_export]
macro_rules! ems_error {
    (context = $ctx:expr, $($arg:tt)+) => {{
        let ctx = &$ctx;
        tracing::event!(
            tracing::Level::ERROR,
            grid = ctx.grid.unwrap_or(""),
            controller = ctx.controller.unwrap_or(""),
            tick = ctx.tick.unwrap_or_default(),
            mode = ctx.mode.unwrap_or(""),
            message = %format_args!($($arg)+)
        );
    }};
    ($($arg:tt)+) => {{
        let ctx = &$crate::LogContext::default();
        tracing::event!(
            tracing::Level::ERROR,
            grid = ctx.grid.unwrap_or(""),
            controller = ctx.controller.unwrap_or(""),
            tick = ctx.tick.unwrap_or_default(),
            mode = ctx.mode.unwrap_or(""),
            message = %format_args!($($arg)+)
        );
    }};
}
