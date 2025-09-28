//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Primary orchestration and lifecycle management."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs;
use std::path::PathBuf;

use r_ems_common::config::UpdateConfig;
use r_ems_common::version::VersionInfo;
use r_ems_core::update::UpdateClient;
use serde_json::json;

fn temp_feed_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(name);
    path
}

#[tokio::test]
async fn update_feed_detects_newer_version() {
    let path = temp_feed_path("update_feed.json");
    let feed = json!({
        "releases": [
            {"version": "0.2.0", "commit": "abc", "download_url": null}
        ]
    });
    fs::write(&path, feed.to_string()).unwrap();

    let config = UpdateConfig {
        feed_path: path.clone(),
        allow_apply_in_dev: true,
        ..Default::default()
    };
    let version = VersionInfo {
        semver: "0.1.0".into(),
        git_sha: "deadbeef".into(),
        build_timestamp: "now".into(),
        target: "test".into(),
        profile: "debug".into(),
    };
    let client = UpdateClient::from_config(&config, version).unwrap();

    let result = client.check().await.unwrap();
    assert!(result.update_available());

    client.apply(&result).await.unwrap();
}
