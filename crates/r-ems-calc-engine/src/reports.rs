//! ---
//! ems_section: "08-energy-models-optimization"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Optimisation and calculation routines for energy planning."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::{fs, path::Path};

use serde::Serialize;
use serde_json::json;
use tracing::info;

use crate::{errors::Result, CalcSummary};

#[derive(Debug)]
pub struct ReportExporter<'a> {
    summary: &'a CalcSummary,
}

impl<'a> ReportExporter<'a> {
    pub fn new(summary: &'a CalcSummary) -> Self {
        Self { summary }
    }

    pub fn export_all(&self, output_dir: &Path) -> Result<()> {
        if !output_dir.exists() {
            fs::create_dir_all(output_dir)?;
        }

        let timestamp = self.summary.timestamp.to_rfc3339();
        let version = self.summary.model_version.clone();

        let short_report = ReportEnvelope::new(
            &timestamp,
            version.clone(),
            short_circuit_schema(),
            &self.summary.short_circuit,
        );
        let load_flow_report = ReportEnvelope::new(
            &timestamp,
            version.clone(),
            load_flow_schema(),
            &self.summary.load_flow,
        );
        let cable_report = ReportEnvelope::new(
            &timestamp,
            version,
            cable_check_schema(),
            &self.summary.cable_check,
        );

        write_json(output_dir.join("short_circuit.json"), &short_report)?;
        write_json(output_dir.join("load_flow.json"), &load_flow_report)?;
        write_json(output_dir.join("cable_check.json"), &cable_report)?;

        info!("Reports exported to {}", output_dir.display());
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct ReportEnvelope<'a, T: Serialize> {
    timestamp: &'a str,
    model_version: Option<String>,
    schema: serde_json::Value,
    data: &'a T,
}

impl<'a, T: Serialize> ReportEnvelope<'a, T> {
    fn new(
        timestamp: &'a str,
        model_version: Option<String>,
        schema: serde_json::Value,
        data: &'a T,
    ) -> Self {
        Self {
            timestamp,
            model_version,
            schema,
            data,
        }
    }
}

fn write_json<T: Serialize>(path: impl AsRef<Path>, value: &T) -> Result<()> {
    let serialized = serde_json::to_string_pretty(value)?;
    fs::write(path, serialized)?;
    Ok(())
}

fn short_circuit_schema() -> serde_json::Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "ShortCircuitReport",
        "type": "object",
        "properties": {
            "fault_location": {"type": "string", "format": "uuid"},
            "ik": {"type": "number"},
            "cable_trip": {"type": ["string", "null"], "format": "uuid"},
            "breaker_trip": {"type": ["string", "null"], "format": "uuid"}
        },
        "required": ["fault_location", "ik"],
    })
}

fn load_flow_schema() -> serde_json::Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "LoadFlowReport",
        "type": "object",
        "properties": {
            "bus_voltages": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "component_id": {"type": "string", "format": "uuid"},
                        "voltage": {"type": "number"}
                    },
                    "required": ["component_id", "voltage"]
                }
            },
            "line_currents": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "connection_id": {"type": "string", "format": "uuid"},
                        "from": {"type": "string", "format": "uuid"},
                        "to": {"type": "string", "format": "uuid"},
                        "current": {"type": "number"}
                    },
                    "required": ["connection_id", "from", "to", "current"]
                }
            },
            "total_losses_kw": {"type": "number"}
        },
        "required": ["bus_voltages", "line_currents", "total_losses_kw"]
    })
}

fn cable_check_schema() -> serde_json::Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "CableCheckReport",
        "type": "object",
        "properties": {
            "undersized": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "connection_id": {"type": "string", "format": "uuid"},
                        "cable_name": {"type": "string"},
                        "reasons": {"type": "array", "items": {"type": "string"}},
                        "measured_current_a": {"type": "number"},
                        "voltage_drop_percent": {"type": "number"}
                    },
                    "required": [
                        "connection_id",
                        "cable_name",
                        "reasons",
                        "measured_current_a",
                        "voltage_drop_percent"
                    ]
                }
            }
        },
        "required": ["undersized"]
    })
}
