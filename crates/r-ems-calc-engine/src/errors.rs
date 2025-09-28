//! ---
//! ems_section: "08-energy-models-optimization"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Optimisation and calculation routines for energy planning."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use thiserror::Error;

pub type Result<T> = std::result::Result<T, CalcEngineError>;

#[derive(Debug, Error)]
pub enum CalcEngineError {
    #[error("system model missing faulted component")]
    MissingFault,
    #[error("component {0} not found in system model")]
    UnknownComponent(uuid::Uuid),
    #[error("no slack/source component found for load flow analysis")]
    MissingSlack,
    #[error("load flow solver failed to converge")]
    LoadFlowDidNotConverge,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    SerializationFailed(#[from] serde_json::Error),
    #[error("yaml serialization error: {0}")]
    YamlSerializationFailed(#[from] serde_yaml::Error),
}
