//! ---
//! ems_section: "15-testing-qa-runbook"
//! ems_subsection: "integration-tests"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Integration and validation tests for the R-EMS stack."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs;
use std::path::Path;

use egui_extras::RetainedImage;
use once_cell::sync::Lazy;
use r_ems_grid_builder::{icon_loader::get_icon_path, ComponentKind};

static ICON_BYTES: Lazy<Vec<(ComponentKind, Vec<u8>)>> = Lazy::new(|| {
    ComponentKind::all()
        .iter()
        .map(|kind| {
            let path = get_icon_path(kind);
            let bytes = fs::read(&path)
                .unwrap_or_else(|_| panic!("missing icon file for {} at {}", kind.slug(), path));
            (*kind, bytes)
        })
        .collect()
});

#[test]
fn every_component_has_an_icon_file() {
    for (kind, _) in ICON_BYTES.iter() {
        let path = get_icon_path(kind);
        assert!(Path::new(&path).exists(), "{} should exist", path);
    }
}

#[test]
fn svg_icons_load_via_retained_image() {
    for (kind, bytes) in ICON_BYTES.iter() {
        let name = format!("test-{}.svg", kind.slug());
        let image = RetainedImage::from_svg_bytes(name, bytes);
        assert!(image.is_ok(), "{name} should load as svg");
    }
}
