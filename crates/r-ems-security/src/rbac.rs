//! ---
//! ems_section: "06-security-access-control"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Security policies, identity, and cryptographic utilities."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Permission enumerates actions that can be performed in the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// Read status/metrics information.
    ReadStatus,
    /// Execute control commands.
    ExecuteCommand,
    /// Reload or update configuration.
    ManageConfiguration,
    /// Manage user accounts and API keys.
    ManageUsers,
}

/// Role describes a named set of permissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    /// Role identifier.
    pub name: String,
    /// Permissions attached to the role.
    pub permissions: HashSet<Permission>,
}

impl Role {
    /// Built-in admin role with full access.
    pub fn admin() -> Self {
        Self {
            name: "admin".into(),
            permissions: HashSet::from([
                Permission::ReadStatus,
                Permission::ExecuteCommand,
                Permission::ManageConfiguration,
                Permission::ManageUsers,
            ]),
        }
    }

    /// Operator role with command + status access.
    pub fn operator() -> Self {
        Self {
            name: "operator".into(),
            permissions: HashSet::from([Permission::ReadStatus, Permission::ExecuteCommand]),
        }
    }

    /// Viewer role with read-only access.
    pub fn viewer() -> Self {
        Self {
            name: "viewer".into(),
            permissions: HashSet::from([Permission::ReadStatus]),
        }
    }
}

/// Association between a user and roles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleAssignment {
    /// User identifier.
    pub user_id: String,
    /// Roles assigned to the user.
    pub roles: Vec<String>,
}

/// Errors occurring during RBAC evaluation.
#[derive(Debug, Error)]
pub enum RbacError {
    /// Role not defined in the engine.
    #[error("role not found: {0}")]
    UnknownRole(String),
}

/// RBAC engine that stores role definitions and can evaluate permissions.
#[derive(Debug, Clone)]
pub struct RbacEngine {
    roles: HashMap<String, Role>,
}

impl Default for RbacEngine {
    fn default() -> Self {
        let mut engine = Self {
            roles: HashMap::new(),
        };
        engine.insert_role(Role::admin());
        engine.insert_role(Role::operator());
        engine.insert_role(Role::viewer());
        engine
    }
}

impl RbacEngine {
    /// Create an empty RBAC store (pre-populated with defaults by `Default`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a role definition.
    pub fn insert_role(&mut self, role: Role) {
        self.roles.insert(role.name.clone(), role);
    }

    /// Lookup a role by name.
    pub fn role(&self, name: &str) -> Option<&Role> {
        self.roles.get(name)
    }

    /// Determine whether any of the provided role names grant the permission.
    pub fn is_authorized(
        &self,
        roles: &[String],
        permission: Permission,
    ) -> Result<bool, RbacError> {
        for role_name in roles {
            let role = self
                .roles
                .get(role_name)
                .ok_or_else(|| RbacError::UnknownRole(role_name.clone()))?;
            if role.permissions.contains(&permission) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_roles_cover_core_permissions() {
        let engine = RbacEngine::new();
        assert!(engine
            .is_authorized(&["admin".into()], Permission::ManageUsers)
            .unwrap());
        assert!(engine
            .is_authorized(&["operator".into()], Permission::ExecuteCommand)
            .unwrap());
        assert!(!engine
            .is_authorized(&["viewer".into()], Permission::ExecuteCommand)
            .unwrap());
    }
}
