# R-EMS GUI Service

> **Purpose:** Provides the browser-based operational dashboard for the R-EMS Core platform using Axum and Askama.

## HTTP Endpoints

- `GET /` — Overview dashboard summarising the health of other core services.
- `GET /api/overview` — JSON payload consumed by HTMX widgets for live updates.
- `GET /plugins` — Placeholder plugins management page (to be wired to registry).
- `GET /config` — Read-only configuration view (stubs until configd integration).
- `GET /ha` — High-availability status page (stub).
- `GET /help` — Lists documentation files mounted into the container.
- `GET /help/file?path=<file>` — Renders a single Markdown help file.
- `GET /healthz` — Health check for probes.
- `GET /metrics` — Placeholder metrics endpoint.

All handlers are implemented in `src/main.rs` with detailed commentary to
illustrate the intended architecture and operational guidance for future
contributors.
