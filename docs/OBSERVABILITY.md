---
ems_section: "03-persistence-logging"
ems_subsection: "guide"
ems_type: "doc"
ems_scope: "observability"
ems_description: "How to configure logging and metrics across the workspace."
ems_version: "v0.1.0"
ems_owner: "platform"
---

# Observability Guide

This guide summarises how the core daemon exposes diagnostics for operators and developers.

## Logging

- The daemon initialises [`tracing`][tracing] with a JSON formatter by default. The
  formatter timestamps each record using UTC and writes to stdout, which makes it suitable
  for container platforms.
- Log events are simultaneously written to a daily rotating file under the directory
  configured in `[logging]` (defaults to `target/dev-logs` in development and `/var/log/r-ems`
  in production). This satisfies the rotation requirement for long lived installations.
- Configure verbosity by setting `R-EMS_LOG`. When unset, `RUST_LOG` is honoured. If neither
  is specified the system defaults to `debug`, ensuring detailed diagnostics are always
  available during bring-up.

## Metrics

- Prometheus metrics are backed by the shared registry from `r-ems-metrics`.
- When `[metrics].enabled = true`, the daemon exposes `/metrics` on the configured socket.
  The handler advertises the canonical `text/plain; version=0.0.4` content type and exports
  counters and histograms for daemon lifecycle, configuration loading and orchestrator
  activity.
- The `DaemonMetrics` helper records startup counts, configuration load timings and build
  metadata. `OrchestratorMetrics` surfaces controller status and failover counters per grid.

[tracing]: https://docs.rs/tracing
