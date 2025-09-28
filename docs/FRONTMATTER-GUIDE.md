---
ems_section: "13-documentation-contributor-guides"
ems_subsection: "frontmatter-guide"
ems_type: "doc"
ems_scope: "documentation"
ems_description: "Guide for applying frontmatter metadata across the repository."
ems_version: "v0.0.0-prealpha"
ems_owner: "Ocean Batteries / R-EMS Maintainers"
---

# Frontmatter Guide

All documentation, source files, and configuration artefacts must begin with frontmatter metadata. Use the YAML form shown below and adapt it to the file type by prefixing each line with the appropriate comment marker where necessary.

```yaml
---
ems_section: "<section-number>-<section-name>"
ems_subsection: "<sub-step>"
ems_type: "doc"
ems_scope: "documentation"
ems_description: "<short description>"
ems_version: "v0.0.0-prealpha"
ems_owner: "Ocean Batteries / R-EMS Maintainers"
---
```

## Section Taxonomy

The repository is organised into the following fifteen sections. Each subsection may be expanded as the platform matures.

1. **01-core-functionality**
   - runtime-kernel
   - service-registry
   - command-processing
2. **02-messaging-ipc-data-model**
   - protocols
   - schema-governance
   - queue-brokers
3. **03-persistence-logging**
   - storage-adapters
   - event-journals
   - observability-pipelines
4. **04-configuration-orchestration**
   - configuration-loader
   - orchestration-graph
   - rollout-strategy
5. **05-networking-external-interfaces**
   - edge-connectivity
   - api-gateway
   - remote-clients
6. **06-security-access-control**
   - identity
   - cryptography
   - policy-enforcement
7. **07-resilience-fault-tolerance**
   - redundancy
   - degradation-handling
   - recovery-playbooks
8. **08-energy-models-optimization**
   - forecasting
   - optimisation-algorithms
   - model-validation
9. **09-integration-interoperability**
   - partner-adapters
   - certification
   - interoperability-testing
10. **10-deployment-ci-cd-enhancements**
    - packaging
    - automation
    - environments
11. **11-simulation-test-harness**
    - sim-runtime
    - replay-tools
    - telemetry-sinks
12. **12-gui-setup-wizard**
    - ui-shell
    - onboarding-flow
    - accessibility
13. **13-documentation-contributor-guides**
    - standards
    - templates
    - contributor-experience
14. **14-versioning-licensing-system**
    - semantic-governance
    - entitlement
    - audits
15. **15-testing-qa-runbook**
    - quality-gates
    - incident-response
    - regression

When adding new content choose the most relevant section and subsection slug. If a document spans multiple sections, favour the most dominant concern and reference additional sections inside the body text.

## Applying Frontmatter to Non-Markdown Files

- **Rust source**: prefix each frontmatter line with `//!`.
- **Shell scripts**: prefix each frontmatter line with `#`.
- **TOML/YAML**: prefix each line with `#` and maintain valid comments.
- **Systemd units**: prefix each line with `#` before the `[Unit]` header.

The metadata keeps the repository searchable and ensures all artefacts trace back to the relevant architectural section.
