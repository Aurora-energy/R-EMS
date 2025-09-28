---
ems_section: "01-core-functionality"
ems_subsection: "overview"
ems_type: "doc"
ems_scope: "documentation"
ems_description: "Core runtime primitives, scheduling, and service lifecycle."
ems_version: "v0.0.0-prealpha"
ems_owner: "tbd"
---

# Section 01 · Core Functionality

## Purpose
The core runtime provides the orchestration kernel that boots every grid, supervises
controller state machines, and coordinates persistence, telemetry, and failover logic.
It is implemented primarily in the `r-ems-core` crate, which exposes the orchestrator,
state snapshot, persistence bridge, and update client modules to the rest of the
workspace.【F:crates/r-ems-core/src/lib.rs†L1-L19】

## Installation & Tooling
1. **Install Rust** – The workspace targets the stable Rust toolchain declared in
   `rust-toolchain.toml` and builds with Cargo.
2. **Build and test** – Run the standard build pipeline from the repository root:
   ```bash
   cargo build
   cargo test
   ```
   These commands compile the orchestrator and all supporting crates, ensuring the
   runtime primitives pass their unit and integration coverage.【F:README.md†L57-L66】
3. **Runtime binaries** – Successful builds place the daemon (`r-emsd`) and CLI
   (`r-emsctl`) under `./bin`. Use the CLI to drive lifecycle commands such as
   `r-emsctl status` once the orchestrator is running.【F:README.md†L68-L93】
4. **Installer scripts** – Production installs can be automated with
   `./scripts/install.sh`, which provisions Docker, systemd, or bare-metal layouts and
   bundles the dependencies required by the orchestrator layer.【F:README.md†L123-L134】

## Module Layout
- **`orchestrator.rs`** – Entry point that boots grids, spawns controller tasks, and
  supervises their lifecycle using redundancy and metrics reporting.【F:crates/r-ems-core/src/orchestrator.rs†L12-L157】
- **`state.rs`** – Manages persisted controller snapshots, providing helpers to write,
  restore, and prune state files so controllers can resume after restarts or failovers.【F:crates/r-ems-core/src/state.rs†L10-L97】
- **`integration_persistence.rs`** – Wraps the persistence crate, exposing an event-log
  bridge that records controller ticks, snapshot metadata, and failover events.【F:crates/r-ems-core/src/integration_persistence.rs†L10-L129】
- **`update.rs`** – Configures the workspace update client, mapping repository metadata
  into version-aware update checks and apply operations.【F:crates/r-ems-core/src/update.rs†L1-L48】

## Runtime Lifecycle
1. **Configuration ingestion** – The orchestrator accepts an `AppConfig` struct that
   contains grids, controllers, and snapshot policies, along with optional metrics and
   licensing metadata.【F:crates/r-ems-core/src/orchestrator.rs†L21-L81】
2. **Grid startup** – For each configured grid the orchestrator creates a
   `RedundancySupervisor`, prepares a `SnapshotStore`, and constructs a
   `PersistenceBridge`. It then spawns async tasks for every controller with the
   relevant configuration, simulation options, and metrics handles.【F:crates/r-ems-core/src/orchestrator.rs†L83-L172】
3. **Controller loop** – Each controller registers with the redundancy supervisor,
   restores any persisted snapshot, optionally replays stored events, and enters a
   rate-limited control loop. The loop sends heartbeats, records metrics, and persists
   telemetry snapshots on schedule until a shutdown signal or exit condition is
   met.【F:crates/r-ems-core/src/orchestrator.rs†L187-L318】
4. **Failover supervision** – A dedicated supervisor task evaluates redundancy events
   at fixed intervals. When a failover occurs it records metrics, persists the event, and
   emits a notification so downstream systems can respond.【F:crates/r-ems-core/src/orchestrator.rs†L174-L250】
5. **Shutdown** – The `OrchestratorHandle` can broadcast shutdown, wait for all tasks to
   join, and report orderly termination to the log pipeline.【F:crates/r-ems-core/src/orchestrator.rs†L131-L157】【F:crates/r-ems-core/src/orchestrator.rs†L252-L296】

## Persistence & Replay
- **Snapshot retention** – Snapshot files are stored per grid/controller with retention
  policies enforced through deterministic pruning, guaranteeing only the most recent
  snapshots remain available for fast recovery.【F:crates/r-ems-core/src/state.rs†L38-L97】
- **Event log bridge** – All controller ticks, snapshot status, and failover messages are
  appended to an event log through the `PersistenceBridge`, which can also replay events
  for specific controllers or entire grids during bootstrap.【F:crates/r-ems-core/src/integration_persistence.rs†L45-L169】
- **Auto replay** – When the grid configuration enables auto replay, controller bootstrap
  iterates the event log to rebuild transient state before entering the active loop, a key
  tool for resynchronizing after outages.【F:crates/r-ems-core/src/orchestrator.rs†L209-L246】

## Simulation & Telemetry
- Controllers can initialize a `TelemetrySimulationEngine` when operating in simulation
  mode, allowing synthetic data generation and deterministic replay for testing and lab
  validation.【F:crates/r-ems-core/src/orchestrator.rs†L33-L120】【F:crates/r-ems-core/src/orchestrator.rs†L295-L318】
- Loop timing is measured with `LoopTimingReporter`, and jitter is bounded with a shared
  `RateLimiter`, ensuring predictable scheduling across real and simulated environments.【F:crates/r-ems-core/src/orchestrator.rs†L25-L120】【F:crates/r-ems-core/src/orchestrator.rs†L263-L296】

## Licensing, Metrics & Updates
- **License metadata** – The orchestrator logs validated license information (if
  available) during startup so operational teams can confirm entitlement status.【F:crates/r-ems-core/src/orchestrator.rs†L105-L129】
- **Metrics registry** – When a metrics registry is supplied, orchestrator and persistence
  metrics track controller activity, snapshot health, and failover counts for observability
  dashboards.【F:crates/r-ems-core/src/orchestrator.rs†L58-L102】【F:crates/r-ems-core/src/integration_persistence.rs†L84-L129】
- **Update client** – CLI commands use the `UpdateClient` to perform workspace update
  checks or apply eligible releases based on configured feeds and version metadata.【F:crates/r-ems-core/src/update.rs†L16-L48】【F:crates/r-ems-core/src/orchestrator.rs†L133-L149】

## Operational Interfaces
- **CLI control** – The `r-emsctl` utility can request orchestrator updates, retrieve
  configuration, or trigger shutdown using the exposed handle methods.【F:crates/r-ems-core/src/orchestrator.rs†L133-L167】
- **Automated deployment** – Use the installer scripts or infrastructure automation to
  provision hosts, then start `r-emsd` under your init system. Observability pipelines
  consume the structured logs and metrics emitted by the orchestrator for fleet-wide
  monitoring.【F:README.md†L68-L134】
