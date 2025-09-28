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

fn read(path: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let full = Path::new(manifest_dir).join("..").join(path);
    fs::read_to_string(&full)
        .unwrap_or_else(|err| panic!("failed to read {}: {}", full.display(), err))
}

#[test]
fn dockerfile_supports_multi_architectures() {
    let dockerfile = read("deploy/Dockerfile");
    for arch in ["linux/amd64", "linux/arm64", "linux/arm/v7"] {
        assert!(
            dockerfile.contains(arch),
            "expected Dockerfile to reference target architecture {arch}"
        );
    }
    assert!(
        dockerfile.contains("--platform=$BUILDPLATFORM"),
        "Dockerfile should opt into BuildKit multi-platform support"
    );
}

#[test]
fn prod_compose_mounts_host_paths() {
    let compose = read("deploy/docker-compose.prod.yml");
    assert!(
        compose.contains("type: bind"),
        "production compose must bind mount host directories"
    );
    assert!(
        compose.contains("R_EMS_CONFIG_DIR"),
        "production compose should expose configuration directory interpolation"
    );
}

#[test]
fn systemd_units_use_frontmatter_and_execstart() {
    for unit in [
        "deploy/systemd/r-emsd.service",
        "deploy/systemd/r-ems-api.service",
        "deploy/systemd/r-ems-ui.service",
    ] {
        let content = read(unit);
        assert!(
            content.starts_with("# ---"),
            "{unit} must include frontmatter header"
        );
        assert!(
            content.contains("ExecStart"),
            "{unit} missing ExecStart stanza"
        );
    }
}

#[test]
fn installer_targets_expected_paths() {
    let installer = read("scripts/install.sh");
    for needle in [
        "/etc/r-ems",
        "deploy/docker-compose.prod.yml",
        "deploy/systemd",
    ] {
        assert!(
            installer.contains(needle),
            "installer should reference {needle}"
        );
    }
}
