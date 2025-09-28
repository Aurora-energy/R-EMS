//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Shared primitives and utilities for the core runtime."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
//! Core shared primitives for the R-EMS orchestrator workspace.
//! This crate exposes configuration loading, licensing, logging, and
//! version metadata utilities consumed across the workspace.

pub mod config;
pub mod license;
pub mod logging;
pub mod metrics;
pub mod time;
pub mod version;

pub use config::{
    AppConfig, ControllerConfig, GridConfig, LoggingConfig, MetricsConfig, Mode, SimulationConfig,
    UpdateConfig,
};
pub use license::{
    Feature, FeatureMatrix, LicenseAuthority, LicenseDetails, LicenseTier, LicenseValidation,
    LicenseValidator, MockLicenseAuthority,
};
pub use logging::{init_tracing, LogFormat};
pub use metrics::{JitterHistogram, LoopTimingReporter};
