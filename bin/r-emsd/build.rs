//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "binary"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Binary entrypoint for the R-EMS daemon."
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
    Ok(())
}
