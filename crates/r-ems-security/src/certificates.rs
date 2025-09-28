//! ---
//! ems_section: "06-security-access-control"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Security policies, identity, and cryptographic utilities."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
use rcgen::{Certificate, CertificateParams, DistinguishedName};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Configuration for certificate handling.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CertificateConfig {
    /// Optional directory to store generated certificates.
    pub output_dir: Option<PathBuf>,
    /// Whether to allow generating self-signed certificates in development.
    #[serde(default)]
    pub allow_self_signed: bool,
}

/// Status result when verifying certificates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateStatus {
    /// Certificate trusted (not revoked).
    Valid,
    /// Certificate has been revoked.
    Revoked,
}

/// Simple certificate authority for issuing development certificates.
pub struct CertificateAuthority {
    ca: Certificate,
    revoked_fingerprints: HashSet<String>,
}

impl CertificateAuthority {
    /// Create a new development CA.
    pub fn dev_ca() -> Result<Self> {
        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "R-EMS Dev CA");
        params.alg = &rcgen::PKCS_ECDSA_P256_SHA256;
        let ca = Certificate::from_params(params)?;
        Ok(Self {
            ca,
            revoked_fingerprints: HashSet::new(),
        })
    }

    /// Issue a leaf certificate for the provided common name.
    pub fn issue_certificate(&self, common_name: &str) -> Result<(String, String)> {
        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, common_name);
        params.alg = &rcgen::PKCS_ECDSA_P256_SHA256;
        let cert = Certificate::from_params(params)?;
        let pem = cert.serialize_pem_with_signer(&self.ca)?;
        let key = cert.serialize_private_key_pem();
        Ok((pem, key))
    }

    /// Mark a certificate (by SHA-256 fingerprint) as revoked.
    pub fn revoke(&mut self, certificate_pem: &str) {
        let fingerprint = fingerprint_pem(certificate_pem);
        self.revoked_fingerprints.insert(fingerprint);
    }

    /// Check whether a certificate has been revoked.
    pub fn verify(&self, certificate_pem: &str) -> CertificateStatus {
        let fingerprint = fingerprint_pem(certificate_pem);
        if self.revoked_fingerprints.contains(&fingerprint) {
            CertificateStatus::Revoked
        } else {
            CertificateStatus::Valid
        }
    }
}

fn fingerprint_pem(pem: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pem.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_and_revoke_certificate() {
        let mut ca = CertificateAuthority::dev_ca().unwrap();
        let (cert, _key) = ca.issue_certificate("device-1").unwrap();
        assert_eq!(ca.verify(&cert), CertificateStatus::Valid);
        ca.revoke(&cert);
        assert_eq!(ca.verify(&cert), CertificateStatus::Revoked);
    }
}
