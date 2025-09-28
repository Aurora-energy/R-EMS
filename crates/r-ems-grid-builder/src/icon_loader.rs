//! ---
//! ems_section: "09-integration-interoperability"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Grid modelling helpers for partner integrations."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::component::ComponentKind;

const ICON_DIR: &str = "assets/icons";
const ICON_MAPPING: &str = "assets/icons/components.yaml";

static ICON_MAP: OnceLock<HashMap<String, String>> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct IconMapping(HashMap<String, String>);

fn load_mapping() -> HashMap<String, String> {
    let path = Path::new(ICON_MAPPING);
    let yaml = fs::read_to_string(path).unwrap_or_default();
    if yaml.trim().is_empty() {
        return HashMap::new();
    }
    serde_yaml::from_str::<HashMap<String, String>>(&yaml)
        .or_else(|_| serde_yaml::from_str::<IconMapping>(&yaml).map(|m| m.0))
        .unwrap_or_default()
}

fn mapping() -> &'static HashMap<String, String> {
    ICON_MAP.get_or_init(load_mapping)
}

/// Resolve the filesystem path to the icon associated with a component kind.
///
/// The returned path is relative to the workspace root and points into the
/// `assets/icons` directory. If the component kind is missing from the mapping
/// file we fall back to `{slug}.svg` in the same directory so that downstream
/// code can decide how to handle the missing asset.
pub fn get_icon_path(kind: &ComponentKind) -> String {
    let slug = kind.slug();
    let file = mapping()
        .get(slug)
        .cloned()
        .unwrap_or_else(|| format!("{slug}.svg"));
    format!("{ICON_DIR}/{file}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_is_loaded() {
        let map = mapping();
        assert!(!map.is_empty(), "icon mapping should not be empty");
        for kind in ComponentKind::all() {
            assert!(map.contains_key(kind.slug()));
        }
    }
}
