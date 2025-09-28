---
ems_section: "11-simulation"
ems_subsection: "01-bootstrap"
ems_type: "doc"
ems_scope: "simulation"
ems_description: "Overview of simulation and test harness subsystems in R-EMS."
ems_version: "v0.1.0"
ems_owner: "tbd"
---

# Simulation & Telemetry Subsystem

The simulation subsystem provides deterministic and reproducible telemetry
streams for validating the R-EMS orchestration pipeline. This bootstrap
milestone documents the scope and expectations for upcoming simulation
features.

> Release versioning and compatibility guarantees follow the guidance in
> [`/docs/VERSIONING.md`](./VERSIONING.md).

## Simulation Setup

The `r-ems-sim` crate hosts the runtime responsible for synthesising telemetry
frames during simulation and hybrid modes. Controllers opt into the simulator
via `SimulationConfig` where randomised inputs or explicit scenario files can be
enabled. The configuration also provides control over tick timing, random seed
selection, and optional replay speed-up parameters for accelerated tests.

### Scenario Data Sets (`.json` / `.csv`)

Scenario data can be authored in either JSON or CSV form. Example fixtures live
under `configs/scenarios/` including `dev_sine.json` and the newly added
`dev_sine.csv`. Each record maps to a `TelemetryFrame` describing the grid and
controller identifiers, timestamp, voltage, frequency, load, and an optional
scenario label. Scenario files double as the "DBS" datasets referenced in the
bootstrap plan and can be produced through the CLI detailed below.

## Value Limits & Alarm Functions

The visual simulation engine applies realistic bounds when synthesising
component states. Voltage, current, temperature, and power outputs are clamped
according to component metadata, while alarm conditions are derived from the
`apply_fault` helper that models overloads, short circuits, and disconnections.
These rules flow into the `GridSimulationEngine` and power both manual tests and
dashboard visualisations.

## Replay Support

Deterministic replay is handled by `ReplayEngine`, which ingests either CSV or
JSON scenario files and cycles through their frames. Hybrid mode layers Gaussian
noise over replay data, yielding reproducible yet non-static telemetry streams.
When scenario inputs are unavailable the engine falls back to synthetic waveform
generation ensuring controllers always receive usable telemetry.

## Data Generator CLI (`r-ems-simgen`)

The `r-ems-simgen` binary streamlines fixture authoring. It supports CSV or JSON
output, stdout redirection, configurable sample counts or durations, and the
ability to hybridise existing scenarios with additional noise. The generator can
stamp frames with scenario labels and emits friendly progress messages when
writing to disk.

## Next Steps

Future iterations will hook the simulator into automated regression harnesses
and expand the telemetry catalogue with richer equipment models.
