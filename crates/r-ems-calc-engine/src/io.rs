//! ---
//! ems_section: "08-energy-models-optimization"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Optimisation and calculation routines for energy planning."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::{fs, io::BufRead, path::Path};

use crate::{
    errors::{CalcEngineError, Result},
    model::SystemModel,
    telemetry::TelemetryFrame,
};

pub fn load_system_model_from_file(path: impl AsRef<Path>) -> Result<SystemModel> {
    let data = fs::read_to_string(path)?;
    let model = if data.trim_start().starts_with('{') {
        serde_json::from_str(&data)?
    } else {
        serde_yaml::from_str(&data).map_err(CalcEngineError::YamlSerializationFailed)?
    };
    Ok(model)
}

pub fn load_telemetry_from_jsonl(path: impl AsRef<Path>) -> Result<Vec<TelemetryFrame>> {
    let file = fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut frames = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        frames.push(serde_json::from_str(&line)?);
    }
    Ok(frames)
}

pub fn load_telemetry_from_json(path: impl AsRef<Path>) -> Result<Vec<TelemetryFrame>> {
    let data = fs::read_to_string(path)?;
    let frames = serde_json::from_str(&data)?;
    Ok(frames)
}
