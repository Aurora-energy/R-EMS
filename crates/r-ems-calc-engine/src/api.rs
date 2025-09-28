//! ---
//! ems_section: "08-energy-models-optimization"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Optimisation and calculation routines for energy planning."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use crate::{model::SystemModel, telemetry::TelemetryFrame};

#[cfg(feature = "rest-api")]
pub use rest::router;

#[cfg(feature = "rest-api")]
mod rest {
    use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
    use std::sync::Arc;

    use crate::{
        cable_check::validate_cables, errors::CalcEngineError, load_flow::run_load_flow,
        short_circuit::calculate_short_circuit,
    };

    use super::{AnalysisRequest, SystemModel, TelemetryFrame};

    #[derive(Clone, Default)]
    pub struct CalcEngineState;

    pub fn router() -> Router {
        Router::new()
            .route("/api/calc/shortcircuit", post(short_circuit))
            .route("/api/calc/loadflow", post(load_flow))
            .route("/api/calc/cablecheck", post(cable_check))
            .with_state(Arc::new(CalcEngineState))
    }

    async fn short_circuit(
        State(_): State<Arc<CalcEngineState>>,
        Json(payload): Json<AnalysisRequest>,
    ) -> Result<Json<crate::short_circuit::ShortCircuitReport>, StatusCode> {
        calculate_short_circuit(&payload.model)
            .map(Json)
            .map_err(map_err)
    }

    async fn load_flow(
        State(_): State<Arc<CalcEngineState>>,
        Json(payload): Json<AnalysisRequest>,
    ) -> Result<Json<crate::load_flow::LoadFlowReport>, StatusCode> {
        run_load_flow(&payload.model, &payload.telemetry)
            .map(Json)
            .map_err(map_err)
    }

    async fn cable_check(
        State(_): State<Arc<CalcEngineState>>,
        Json(payload): Json<AnalysisRequest>,
    ) -> Result<Json<crate::cable_check::CableCheckReport>, StatusCode> {
        let load_flow = run_load_flow(&payload.model, &payload.telemetry).map_err(map_err)?;
        validate_cables(&payload.model, &payload.telemetry, &load_flow)
            .map(Json)
            .map_err(map_err)
    }

    fn map_err(err: CalcEngineError) -> StatusCode {
        match err {
            CalcEngineError::MissingFault | CalcEngineError::MissingSlack => {
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnalysisRequest {
    pub model: SystemModel,
    #[serde(default)]
    pub telemetry: Vec<TelemetryFrame>,
}
