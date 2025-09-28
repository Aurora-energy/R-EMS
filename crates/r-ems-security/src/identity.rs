//! ---
//! ems_section: "06-security-access-control"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Security policies, identity, and cryptographic utilities."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::collections::HashMap;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::rbac::Role;

/// Identifier for a user account.
pub type UserId = String;

/// Identifier for an API key.
pub type ApiKeyId = String;

/// Representation of a user within the identity store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserAccount {
    /// Stable identifier (UUID or slug).
    pub id: UserId,
    /// Human readable username.
    pub username: String,
    /// Display name for UI rendering.
    pub display_name: String,
    /// Assigned roles.
    pub roles: Vec<Role>,
    /// Timestamp of creation.
    pub created_at: DateTime<Utc>,
    /// Whether the user can authenticate.
    pub active: bool,
}

impl UserAccount {
    /// Short helper for constructing a new account.
    pub fn new(id: impl Into<UserId>, username: impl Into<String>, roles: Vec<Role>) -> Self {
        Self {
            id: id.into(),
            username: username.into(),
            display_name: String::new(),
            roles,
            created_at: Utc::now(),
            active: true,
        }
    }
}

/// Stored representation of an API key (hashed on disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredApiKey {
    user_id: UserId,
    hash: String,
    issued_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    scopes: Vec<String>,
}

/// Claims returned when an API key is validated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenClaims {
    /// User identifier bound to the key.
    pub subject: UserId,
    /// Role names granted to the principal.
    pub roles: Vec<String>,
    /// Scope strings attached to the key.
    pub scopes: Vec<String>,
    /// Issued timestamp.
    pub issued_at: DateTime<Utc>,
    /// Optional expiry.
    pub expires_at: Option<DateTime<Utc>>,
}

/// API key returned to the caller (secret string plus metadata).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiKey {
    /// Database identifier.
    pub id: ApiKeyId,
    /// Plaintext secret (base64 encoded) the caller must store securely.
    pub secret: String,
    /// Expiry if configured.
    pub expires_at: Option<DateTime<Utc>>,
    /// Associated scopes.
    pub scopes: Vec<String>,
}

/// Errors returned by the identity subsystem.
#[derive(Debug, Error)]
pub enum IdentityError {
    /// Attempted to create a user that already exists.
    #[error("user already exists")]
    UserExists,
    /// User not found.
    #[error("user not found")]
    UserNotFound,
    /// API key lookup failure.
    #[error("api key invalid")]
    InvalidApiKey,
    /// API key expired.
    #[error("api key expired")]
    ApiKeyExpired,
}

/// In-memory identity provider suitable for development/testing.
#[derive(Debug, Default, Clone)]
pub struct IdentityProvider {
    users: Arc<RwLock<HashMap<UserId, UserAccount>>>,
    api_keys: Arc<RwLock<HashMap<ApiKeyId, StoredApiKey>>>,
}

impl IdentityProvider {
    /// Create an empty identity provider.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create or update a user account.
    pub fn upsert_user(&self, user: UserAccount) {
        self.users.write().insert(user.id.clone(), user);
    }

    /// Retrieve a user by id.
    pub fn get_user(&self, user_id: &str) -> Option<UserAccount> {
        self.users.read().get(user_id).cloned()
    }

    /// Issue a new API key associated with the user.
    pub fn issue_api_key(
        &self,
        user_id: &str,
        scopes: &[String],
        ttl: Option<Duration>,
    ) -> Result<ApiKey, IdentityError> {
        let user = self
            .users
            .read()
            .get(user_id)
            .cloned()
            .ok_or(IdentityError::UserNotFound)?;
        if !user.active {
            return Err(IdentityError::InvalidApiKey);
        }

        let mut secret_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret_bytes);
        let secret = BASE64.encode(secret_bytes);
        let hash = hash_secret(&secret);
        let now = Utc::now();
        let expires_at = ttl.map(|delta| now + delta);
        let id = uuid::Uuid::new_v4().to_string();

        let stored = StoredApiKey {
            user_id: user.id.clone(),
            hash,
            issued_at: now,
            expires_at,
            scopes: scopes.to_vec(),
        };

        self.api_keys.write().insert(id.clone(), stored);

        Ok(ApiKey {
            id,
            secret,
            expires_at,
            scopes: scopes.to_vec(),
        })
    }

    /// Validate an API key secret and return claims for downstream services.
    pub fn authenticate_api_key(&self, secret: &str) -> Result<TokenClaims, IdentityError> {
        let hash = hash_secret(secret);
        let store = self.api_keys.read();
        let (_key_id, key) = store
            .iter()
            .find(|(_id, stored)| stored.hash == hash)
            .ok_or(IdentityError::InvalidApiKey)?;

        if let Some(expiry) = key.expires_at {
            if Utc::now() > expiry {
                return Err(IdentityError::ApiKeyExpired);
            }
        }

        let user = self
            .users
            .read()
            .get(&key.user_id)
            .cloned()
            .ok_or(IdentityError::UserNotFound)?;

        Ok(TokenClaims {
            subject: user.id,
            roles: user.roles.iter().map(|role| role.name.clone()).collect(),
            scopes: key.scopes.clone(),
            issued_at: key.issued_at,
            expires_at: key.expires_at,
        })
    }

    /// Revoke an API key by identifier.
    pub fn revoke_api_key(&self, id: &str) -> bool {
        self.api_keys.write().remove(id).is_some()
    }

    /// Enumerate API keys for a user (metadata only).
    pub fn list_api_keys(
        &self,
        user_id: &str,
    ) -> Vec<(ApiKeyId, DateTime<Utc>, Option<DateTime<Utc>>)> {
        self.api_keys
            .read()
            .iter()
            .filter(|(_id, key)| key.user_id == user_id)
            .map(|(id, key)| (id.clone(), key.issued_at, key.expires_at))
            .collect()
    }
}

fn hash_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbac::Role;

    #[tokio::test]
    async fn issue_and_authenticate_key() {
        let provider = IdentityProvider::new();
        provider.upsert_user(UserAccount::new("user-1", "alice", vec![Role::admin()]));

        let key = provider
            .issue_api_key(
                "user-1",
                &["commands".to_string()],
                Some(Duration::minutes(10)),
            )
            .unwrap();
        let claims = provider.authenticate_api_key(&key.secret).unwrap();
        assert_eq!(claims.subject, "user-1");
        assert_eq!(claims.roles, vec!["admin".to_string()]);
        assert_eq!(claims.scopes, vec!["commands".to_string()]);
    }

    #[test]
    fn listing_keys_returns_metadata() {
        let provider = IdentityProvider::new();
        provider.upsert_user(UserAccount::new("user-1", "alice", vec![Role::viewer()]));
        provider
            .issue_api_key("user-1", &[], Some(Duration::minutes(1)))
            .unwrap();
        let keys = provider.list_api_keys("user-1");
        assert_eq!(keys.len(), 1);
    }
}
