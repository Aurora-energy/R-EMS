# R-EMS Event Bus

> **Purpose:** Serves as the tonic-based publish/subscribe backbone with HTTP health endpoints.

## HTTP Endpoints

- `GET /healthz` — Liveness probe for container orchestrators.

## gRPC (planned)

- `Publish` — Accepts envelopes and fans them out to subscribers.
- `Subscribe` — Opens a streaming response for deterministic topic partitions.

Detailed implementation will be added in later phases. The current skeleton
focuses on bootstrapping the Rust binary and operational scaffolding.
