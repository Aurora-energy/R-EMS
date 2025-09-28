//! ---
//! ems_section: "14-versioning-licensing-system"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Version metadata and release governance helpers."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use serde::Serialize;

/// Compile-time version metadata captured via `vergen`.
#[derive(Debug, Clone, Serialize)]
pub struct VersionInfo {
    /// Workspace semantic version.
    pub semver: String,
    /// Git commit hash captured at build time.
    pub git_sha: String,
    /// Build timestamp from the compilation environment.
    pub build_timestamp: String,
    /// Target triple used for the build.
    pub target: String,
    /// Cargo profile used during compilation.
    pub profile: String,
}

impl VersionInfo {
    /// Construct a new [`VersionInfo`] instance using environment metadata.
    #[must_use]
    pub fn current() -> Self {
        Self {
            semver: env!("CARGO_PKG_VERSION").to_owned(),
            git_sha: option_env!("VERGEN_GIT_SHA")
                .unwrap_or("UNKNOWN")
                .to_owned(),
            build_timestamp: option_env!("VERGEN_BUILD_TIMESTAMP")
                .unwrap_or("UNKNOWN")
                .to_owned(),
            target: option_env!("VERGEN_CARGO_TARGET_TRIPLE")
                .unwrap_or("UNKNOWN")
                .to_owned(),
            profile: option_env!("VERGEN_CARGO_PROFILE")
                .unwrap_or("UNKNOWN")
                .to_owned(),
        }
    }

    /// Returns a concise CLI string combining semantic version and git hash.
    #[must_use]
    pub fn cli_string(&self) -> String {
        format!("{} ({})", self.semver, self.git_sha)
    }

    /// Human readable banner used in logging surfaces.
    #[must_use]
    pub fn banner(&self) -> String {
        format!("R-EMS v{} (git {})", self.semver, self.git_sha)
    }

    /// Extended string containing build metadata suitable for `--version` flags.
    #[must_use]
    pub fn extended(&self) -> String {
        format!(
            "{banner}\nBuilt: {built}\nTarget: {target}\nProfile: {profile}",
            banner = self.banner(),
            built = self.build_timestamp,
            target = self.target,
            profile = self.profile
        )
    }
}

/// Helper for Clap commands to print the extended version string.
#[must_use]
pub fn clap_long_version() -> String {
    VersionInfo::current().extended()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_contains_semver() {
        let info = VersionInfo::current();
        let extended = info.extended();
        assert!(extended.contains(&info.semver));
    }
}
