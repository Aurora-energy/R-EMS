---
ems_section: "15-testing-runbook"
ems_subsection: "troubleshooting"
ems_type: "doc"
ems_scope: "operations"
ems_description: "Troubleshooting guide for diagnosing common installation and runtime issues."
ems_version: "v0.1.0"
ems_owner: "platform"
---

# Troubleshooting Guide

Use this guide to diagnose common issues encountered during installation,
configuration, or runtime operation. Each section includes quick checks and the
recommended corrective actions.

## Installation Failures

### Installer exits with missing dependencies

- **Symptom:** `scripts/install.sh` reports that Docker or sudo is unavailable.
- **Resolution:** Install Docker Engine 24+ using your distribution packages and
  ensure the current user has permission to run `docker`. When running as a
  non-root user, add yourself to the `docker` group or rerun the installer with
  elevated privileges.

### Rust toolchain not found

- **Symptom:** `cargo build` fails with `command not found`.
- **Resolution:** Install Rust using `rustup` (`curl https://sh.rustup.rs -sSf | sh`) or
  rerun the installer with `--install-rust` to allow automatic provisioning.

### Systemd unit not created

- **Symptom:** After running the installer the `r-emsd.service` unit is missing.
- **Resolution:** Ensure you did not pass `--no-systemd`. Rerun the installer
  without that flag, or manually copy the generated unit from the installer
  output directory into `/etc/systemd/system/` before reloading systemd.

## Runtime Bring-up

### Daemon fails to start

- **Symptom:** `./bin/r-emsd` exits immediately.
- **Checks:**
  - Inspect the JSON logs printed to stdout for configuration errors.
  - Verify that the manifest under `/etc/r-ems` is valid via `./bin/r-emsctl setup verify`.
  - Ensure required ports are free (default metrics exporter and RPC listeners).
- **Resolution:** Correct the configuration, then restart the daemon. Use
  `./bin/r-emsctl status` to confirm healthy controllers.

### Controller missing telemetry

- **Symptom:** Dashboards do not show updates while the daemon is running.
- **Checks:**
  - Confirm the simulator is enabled in the manifest or that hardware telemetry
    endpoints are reachable.
  - Tail the rotating log files under `/var/log/r-ems` (or `target/dev-logs` in
    development) to look for connectivity warnings.
- **Resolution:** Restart the simulation via `./bin/r-emsctl simulation start` with a
  known-good scenario file from `configs/scenarios/`. For hardware connections,
  verify network routes and credentials.

### CLI cannot connect to daemon

- **Symptom:** `./bin/r-emsctl status` reports connection refused.
- **Checks:**
  - Ensure `r-emsd` is running (`systemctl status r-emsd` or `ps aux | grep r-emsd`).
  - Confirm both processes reference the same configuration root. Override with
    `R_EMS_CONFIG_ROOT` when necessary.
- **Resolution:** Restart the daemon, then rerun the CLI command. If using
  Docker compose, ensure the control plane container is healthy via
  `docker compose ps`.

## Simulation Issues

### Scenario fails to load

- **Symptom:** `r-emsctl simulation start` prints a parse error.
- **Resolution:** Validate the JSON/CSV file under `configs/scenarios/` and ensure
  it matches the `TelemetryFrame` schema. Use `r-ems-simgen` to regenerate sample
  datasets when in doubt.

### Replay not deterministic

- **Symptom:** Replays produce inconsistent results between runs.
- **Resolution:** Confirm the scenario is not running in hybrid mode. For
  deterministic playback disable additional noise in the manifest and set the
  simulator seed explicitly.

## Observability & Diagnostics

### Locating logs

- Runtime logs stream to stdout and to daily rotated files under the configured
  logging directory (`target/dev-logs` by default, `/var/log/r-ems` in production).
  Search for error-level records when diagnosing failures.

### Collecting metrics

- Enable the Prometheus exporter in the manifest (`[metrics].enabled = true`).
  Scrape the `/metrics` endpoint exposed by `r-emsd` to capture controller
  health, failover counters, and build metadata.

### Enabling verbose output

- Override log filtering with `R-EMS_LOG=<module>=trace` (falls back to
  `RUST_LOG`). This is useful when troubleshooting orchestration or simulator
  behaviour during bring-up.

## Support Checklist

1. Capture the failing command and full terminal output.
2. Export the latest log bundle from the logging directory.
3. Note the git commit, installer version, and configuration root in use.
4. Include scenario files or controller IDs when filing an issue.

