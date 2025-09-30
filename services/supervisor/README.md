# R-EMS Supervisor

> **Purpose:** Orchestrates plugin lifecycle and health reporting via Docker.

## HTTP Endpoints (bootstrap)

- `GET /api/health` — JSON health summary.
- `GET /healthz` — Simple liveness probe used by Compose/HA.

Future revisions will expose plugin listing and toggle endpoints as specified in
later project phases.
