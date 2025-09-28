---
ems_section: "14-versioning-licensing-system"
ems_subsection: "versioning"
ems_type: "doc"
ems_scope: "documentation"
ems_description: "Branching and versioning rules for the pre-alpha programme."
ems_version: "v0.0.0-prealpha"
ems_owner: "Ocean Batteries / R-EMS Maintainers"
---

# Versioning & Branch Strategy

R-EMS is currently in **pre-alpha** and follows a lightweight branching process while the architecture stabilises.

## Branch Model

- `main` represents the latest stable baseline for demonstrations and archival builds.
- `develop` aggregates completed features and integration work awaiting promotion to `main`.
- `feature/*` branches capture focused changes for a single capability or fix.
- `release/*` branches prepare a candidate for a future tagged pre-release.

## Version Semantics

- The active version string is `v0.0.0-prealpha`.
- Tags are created on `main` only when significant architectural checkpoints are reached.
- Breaking changes are expected without notice until the first alpha milestone is published.

## Release Cadence

Releases are not yet scheduled. The team may cut experimental builds from `develop` to validate packaging, but these artefacts are considered volatile and should not be deployed to production systems.
