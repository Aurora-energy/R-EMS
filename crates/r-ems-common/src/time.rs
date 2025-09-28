//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Shared primitives and utilities for the core runtime."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::time::{Duration, Instant};

/// Capture an instant suitable for scheduler comparisons.
pub fn monotonic_now() -> Instant {
    Instant::now()
}

/// Convert a duration into microseconds, saturating at `u64::MAX`.
pub fn duration_to_micros(duration: Duration) -> u64 {
    duration.as_secs().saturating_mul(1_000_000) + u64::from(duration.subsec_micros())
}

/// Convert to human-friendly jitter units.
pub fn jitter_us(actual: Duration, expected: Duration) -> i64 {
    let actual_us = actual.as_secs_f64() * 1_000_000.0;
    let expected_us = expected.as_secs_f64() * 1_000_000.0;
    (actual_us - expected_us).round() as i64
}
