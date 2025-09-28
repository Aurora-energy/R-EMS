---
ems_section: "12-gui-setup-wizard"
ems_subsection: "overview"
ems_type: "doc"
ems_scope: "documentation"
ems_description: "User interface and setup wizard guidance."
ems_version: "v0.0.0-prealpha"
ems_owner: "tbd"
---

# Section 12 · GUI & Setup Wizard

This section now covers both the **browser-based onboarding experience** and
the new **interactive CLI wizard** introduced for first-run installations. The
two entry points share the same manifest format and validation rules so that an
operator can freely switch between GUI and terminal workflows.

## 12.1 · First-Run CLI Wizard (`r-emsctl setup wizard`)

- The CLI wizard guides the operator through:
  - Naming the installation and deriving a filesystem-safe slug.
  - Selecting the operating mode (production, simulation, hybrid) with inline
    guidance describing each option.
  - Choosing the logging directory and output format (structured JSON for
    aggregators or human-friendly pretty logs).
  - Defining one or more grids and, for each grid, adding controllers with
    explicit roles and deterministic failover order.
- The wizard enforces schema invariants on-the-fly: grid identifiers must be
  unique, each grid needs at least one controller, and at least one controller
  must be marked as `primary` to establish redundancy.
- Before writing to disk, the wizard renders the effective configuration as a
  YAML preview so changes can be reviewed or aborted safely. If the operator
  cancels, no files are written and a structured "fault" event is logged.

## 12.2 · Configuration Persistence & Editing

- Persisted manifests live under `/etc/r-ems` (override with
  `R_EMS_CONFIG_ROOT`). The active manifest is symlinked via
  `/etc/r-ems/current.toml` for easy discovery by services and tooling.
- Both the GUI wizard and the CLI (`r-emsctl setup wizard` or
  `r-emsctl setup new`) operate on this shared storage layout, enabling edits in
  either interface without additional synchronization steps.
- Manifest schema validation is shared with the runtime (`AppConfig::validate`),
  so malformed configurations are rejected early with human-readable errors.

## 12.3 · Logging & Diagnostics

- The logging crate exposes `log_system_event`, a helper that records success or
  fault outcomes together with contextual metadata (grid/controller identifiers,
  tick, mode). The CLI wizard uses this helper to capture setup successes and
  aborted runs for audit trails.
- Use structured JSON logging to forward events into OpenBalena, SurrealDB, or
  other observability stacks shipped in the Docker image.

## 12.4 · Docker & First-Run Experience

- `scripts/docker-install.sh` builds and launches the runtime image with the
  required services (OpenBalena supervisor, SurrealDB, static setup wizard
  assets). After the container starts, the browser-based wizard is available on
  port `8080`, while the CLI wizard can be executed inside or outside the
  container to edit the same manifests.
