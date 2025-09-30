# R-EMS ConfigD

The configuration distribution service ingests the declarative system topology,
validates redundancy guarantees, and exposes the resulting snapshot over a
simple HTTP API. Configuration documents are expressed in YAML using the
structure defined below.

## Data Model Overview

```yaml
system:
  grids:
    - id: grid_a
      allow_interop: true
      controllers:
        - id: grid_a_ctrl_primary
          role: primary
          redundancy_group: grid_a_cluster
          heartbeat_interval_ms: 500
          failover_timeout_ms: 1500
        - id: grid_a_ctrl_backup
          role: backup
          redundancy_group: grid_a_cluster
          heartbeat_interval_ms: 500
          failover_timeout_ms: 1500
      devices:
        - id: inverter_main
          bus: can
          address: "0x12"
          protocol:
            dbc_file: dbcs/inverter.dbc
          telemetry:
            - name: voltage
              unit: volts
          commands:
            - name: set_voltage
```

- **Redundancy:** Controllers assigned the `primary` or `backup` role must
  share a `redundancy_group`, declare heartbeat and failover timers, and each
  group must have exactly one primary and at least one backup.
- **Devices:** Every device lists the bus type (`can` or `rs485`), the bus
  address, telemetry values to expect, available commands, and protocol
  metadata. CAN devices require a `dbc_file`; RS-485 devices require a
  `register_map` entry.

## HTTP Endpoints

- `GET /api/config` — Full configuration snapshot.
- `GET /api/config/summary` — Count of grids, controllers, and devices that
  passed validation.
- `GET /healthz` — Liveness endpoint for infrastructure probes.

## CLI Usage

```
cargo run -p r-ems-configd -- --config configs/system.yaml validate
```

- `serve` (default) — Validates configuration and starts the HTTP API.
- `validate` — Performs validation only and exits with an error code on failure.

Logs are emitted to both STDOUT and `logs/configd.log` (overridable via the
`--log-dir` flag or `REMS_LOG_DIR`).
