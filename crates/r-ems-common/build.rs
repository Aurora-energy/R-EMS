//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Shared primitives and utilities for the core runtime."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use vergen::EmitBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    EmitBuilder::builder()
        .fail_on_error()
        .all_cargo()
        .all_git()
        .emit()?;

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=R_EMS_GIT_OVERRIDE");
    Ok(())
}
