//! ---
//! ems_section: "14-versioning-licensing-system"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Version metadata and release governance helpers."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
#![warn(missing_docs)]

//! Core crate exposing version metadata helpers, update primitives, and
//! integration glue for embedding build information across the workspace.

pub mod semver;
pub mod update;
