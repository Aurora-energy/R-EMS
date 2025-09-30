# R-EMS Registry

> **Purpose:** Validates plugin manifests and coordinates ACL negotiation.

## HTTP Endpoints (bootstrap)

- `GET /api/plugins` — Stubbed list of available plugins.
- `GET /api/plugins/toggle` — Placeholder toggle endpoint.
- `GET /healthz` — Health probe.

Further development will add schema negotiation and topic ACL enforcement.
