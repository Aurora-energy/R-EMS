# R-EMS Schemas

> **Purpose:** Houses protobuf definitions and generated Rust modules shared across services.

## Layout

- `proto/ems/core/v1/` — Source `.proto` files.
- `build.rs` — Invokes `tonic-build` to generate Rust during compilation.
- `src/lib.rs` — Re-exports the generated modules for downstream crates.

The placeholder definitions will be replaced in Phase 1 when the actual schema
is authored.
