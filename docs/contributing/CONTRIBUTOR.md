---
ems_section: "13-documentation-contributor-guides"
ems_subsection: "contributor-guide"
ems_type: "doc"
ems_scope: "documentation"
ems_description: "Contributor expectations and workflow guidance."
ems_version: "v0.0.0-prealpha"
ems_owner: "tbd"
---

# Contributor Guide

Thank you for contributing to R-EMS. During the pre-alpha phase we focus on establishing the architecture and ensuring repository hygiene.

## Workflow Expectations

1. Fork the repository and create a `feature/<topic>` branch from `develop`.
2. Keep pull requests small and focused on a single architectural section.
3. Ensure all files include valid frontmatter metadata.
4. Provide tests and simulation coverage where possible.
5. Update the documentation within the relevant section to reflect design decisions.

## Code Review

- Two maintainer approvals are required before merging into `develop`.
- Use descriptive commit messages referencing the impacted sections.
- Capture any follow-up work using TODO comments flagged with the responsible section identifier.

## Communication

- Join the asynchronous design discussions via the tbd engineering forum.
- Release planning occurs fortnightly; ensure feature branches are rebased prior to the planning call.
