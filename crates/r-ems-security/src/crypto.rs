//! ---
//! ems_section: "06-security-access-control"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Security policies, identity, and cryptographic utilities."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rand::RngCore;
use rcgen::{Certificate, CertificateParams, DistinguishedName, Error as RcgenError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::info;

/// TLS assets loaded from disk or generated for development.
#[derive(Debug, Clone)]
pub struct TlsAssets {
    /// PEM-encoded certificate chain.
    pub certificate_pem: String,
    /// PEM-encoded private key.
    pub private_key_pem: String,
}

/// Configuration describing how TLS assets should be loaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to certificate PEM file.
    pub cert_path: Option<PathBuf>,
    /// Path to private key PEM file.
    pub key_path: Option<PathBuf>,
    /// When true, a self-signed certificate is generated if assets are missing (dev mode).
    #[serde(default)]
    pub allow_self_signed: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_path: None,
            key_path: None,
            allow_self_signed: true,
        }
    }
}

/// Opaque symmetric key material (32 bytes) for encryption/HMAC purposes.
#[derive(Debug, Clone)]
pub struct KeyMaterial(pub [u8; 32]);

impl KeyMaterial {
    /// Generate random key material.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Render as base64 string.
    pub fn to_base64(&self) -> String {
        BASE64.encode(self.0)
    }

    /// Compute a SHA-256 fingerprint of the key for audit/logging.
    pub fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.0);
        hex::encode(hasher.finalize())
    }
}

/// Load TLS assets based on configuration, falling back to self-signed material.
pub fn load_tls_assets(config: &TlsConfig) -> Result<TlsAssets> {
    match (&config.cert_path, &config.key_path) {
        (Some(cert), Some(key)) => {
            let cert_pem = fs::read_to_string(cert)
                .with_context(|| format!("unable to read certificate at {}", cert.display()))?;
            let key_pem = fs::read_to_string(key)
                .with_context(|| format!("unable to read private key at {}", key.display()))?;
            Ok(TlsAssets {
                certificate_pem: cert_pem,
                private_key_pem: key_pem,
            })
        }
        _ if config.allow_self_signed => {
            let cert = generate_self_signed_cert()?;
            info!("generated self-signed TLS certificate for development");
            Ok(TlsAssets {
                certificate_pem: cert.serialize_pem()?,
                private_key_pem: cert.serialize_private_key_pem(),
            })
        }
        _ => anyhow::bail!("TLS assets not configured and self-signed generation disabled"),
    }
}

fn generate_self_signed_cert() -> Result<Certificate, RcgenError> {
    let mut params = CertificateParams::default();
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "R-EMS Development CA");
    params.alg = &rcgen::PKCS_ECDSA_P256_SHA256;
    Certificate::from_params(params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_key_has_32_bytes() {
        let key = KeyMaterial::generate();
        assert_eq!(key.0.len(), 32);
        assert_eq!(key.to_base64().len(), 44);
    }

    #[test]
    fn self_signed_cert_generation_succeeds() {
        let assets = load_tls_assets(&TlsConfig {
            cert_path: None,
            key_path: None,
            allow_self_signed: true,
        })
        .unwrap();
        assert!(assets.certificate_pem.contains("BEGIN CERTIFICATE"));
        assert!(assets.private_key_pem.contains("PRIVATE KEY"));
    }
}
