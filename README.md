Status Very early alpha stage no official release yet

# R-EMS Orchestrator

R-EMS is a redundant energy management system designed for industrial and edge deployments. This repository hosts the Rust workspace that powers the core runtime, redundancy supervisor, simulation engine, and Balena-ready CLI daemon (`r-emsd`).

## Features
- Multi-grid orchestration with deterministic scheduling and heartbeat-driven health monitoring
- Controller redundancy with promotion, watchdog timers, and snapshot-based replay
- License validation with verifiable payloads (base64 + HMAC) and developer bypass controls
- Semantic versioning with Git commit embedding via `vergen`
- Update subsystem supporting local feeds and GitHub Releases via `octocrab`
- Simulator mode for synthetic or replayed telemetry, including hybrid hardware/sim blending
- Structured logging (`tracing`) with production and developer profiles

## Workspace Layout
```
Cargo.toml              # workspace definition
bin/r-emsd/             # CLI daemon entrypoint
crates/r-ems-common/    # shared types, config, licensing, logging
crates/r-ems-core/      # orchestrator runtime, snapshots, updates
crates/r-ems-redundancy/# heartbeat + failover supervisor
crates/r-ems-sim/       # synthetic telemetry + scenario replay engines
crates/r-ems-rt/        # real-time helpers (rate limiter, deterministic executor)
configs/                # example TOML configs + update feed + scenarios
.github/workflows/      # CI + release pipelines
```

## Quick Start
1. Ensure Rust 1.82+ and Cargo are installed.
2. Clone the repo and enter the workspace.
3. Choose a configuration (`configs/example.dev.toml`, `configs/example.prod.toml`, or `configs/rpi_sim.toml`).
4. Run the daemon:
   ```bash
   cargo run -p r-emsd -- --config configs/example.dev.toml run
   ```
5. Trigger simulator output:
   ```bash
   cargo run -p r-emsd -- --mode simulation --config configs/example.dev.toml run
   ```
6. Check for updates:
   ```bash
   cargo run -p r-emsd -- update-check
   ```

## Versioning & Build Metadata
- Semantic version set in `Cargo.toml`
- `vergen` build script embeds Git SHA, target triple, profile, and timestamp
- `r-emsd --version` prints `SEMVER (GIT_SHA)`

## License Validation
- License payload: base64 JSON containing owner, key id, expiry, signature (HMAC-SHA256)
- License is loaded from file (`license.path`) or env var (`R_EMS_LICENSE` by default)
- Startup aborts if license invalid unless `--dev-allow-license-bypass` or config `allow_bypass`
- Validation metadata is logged on success and stored in `LicenseValidation`

## Update Workflow
- Local feed: `configs/update_feed.json`
- GitHub: owner/repo from config → `octocrab` integration
- CLI commands:
  - `r-emsd update-check` → report latest version
  - `r-emsd update-apply` → (dev only) simulate binary swap
- Simulation writes artifacts to `target/update-simulated/`

## Runtime Modes
| Mode        | Description                                      |
|-------------|--------------------------------------------------|
| production  | Hardware inputs, minimal logging                 |
| simulation  | Synthetic or replayed telemetry replaces inputs |
| hybrid      | Primary hardware with synthetic augmentation     |

Mode can be set via config (`mode = "simulation"`), CLI flag (`--mode hybrid`), or simulation config override.

## Logging & Telemetry
- `tracing` with JSON or pretty formatting
- Env var `R-EMS_LOG` sets filter (`debug` for developer mode)
- Log fields: `grid_id`, `controller_id`, `role`, `mode`, `tick`, `jitter_us`, `failover_event`
- Jitter histograms written to `target/test-logs/<grid>-<controller>-jitter.json`

## Testing
- Unit tests for license validation, redundancy promotion, update detection
- Orchestrator integration test runs simulator mode with multiple controllers
- Property/performance test scaffolding provided via `LoopTimingReporter`

Run the suite:
```bash
cargo fmt
cargo clippy --workspace --all-targets
cargo test --workspace
```

## CI / CD
- `.github/workflows/ci.yml`: `cargo fmt`, `cargo clippy`, `cargo test`, simulator smoke test
- `.github/workflows/release.yml`: version tag build, Docker image for Balena, release notes

## Documentation
- `ARCHITECTURE.md`: system diagrams (control loop, redundancy, simulation)
- `LICENSES.md`, `UPDATES.md`, `SIMULATION.md`
- Detailed docs under `docs/`

## Contributions
- Branch strategy: `main` (stable), `dev` (integration), feature branches via PR
- Tests + docs required for PRs
- Follow coding guidelines in `docs/OPERATIONS.md`

## Messaging Workspace Scaffold
This repository now includes the initial scaffolding for the dedicated messaging workspace covering schema, messaging, transport, and replay crates; the code presently exposes placeholder modules only and will be extended in subsequent iterations.
## Docker Automation
- Run `./scripts/docker-install.sh` (macOS/Linux) or `powershell -File scripts/docker-install.ps1` (Windows) to build the image and launch the daemon.
- Override name/image with `R_EMS_CONTAINER`/`R_EMS_IMAGE` env vars.
- To use Docker Compose: `docker compose -f docker/compose.yml up -d` (creates config/Log/snapshot bind mounts under `target/`).
- Logs stream via `docker logs -f r-emsd`; config file persisted at `target/docker-config/config.toml` for editing.
