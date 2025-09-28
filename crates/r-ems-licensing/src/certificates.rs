//! ---
//! ems_section: "14-versioning-licensing-system"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Licensing enforcement and entitlement checks."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::to_vec;
use std::convert::TryInto;

use crate::core::LicensePayload;

/// Embedded development public key for validating bundled licenses.
pub const DEV_PUBLIC_KEY: [u8; 32] = [
    168, 0, 169, 32, 60, 42, 128, 57, 90, 246, 86, 71, 142, 136, 197, 255, 102, 76, 29, 121, 51,
    29, 142, 59, 79, 67, 201, 133, 11, 56, 13, 229,
];

/// In-memory representation of a signed license certificate.
#[derive(Debug)]
pub struct LicenseCertificate {
    pub(crate) payload: LicensePayload,
    pub(crate) signature: String,
}

/// Verify a license certificate using the provided public key bytes.
pub fn verify_certificate(certificate: &LicenseCertificate, public_key: &[u8; 32]) -> Result<()> {
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|err| anyhow!("invalid public key material: {err}"))?;
    let signature_bytes = general_purpose::STANDARD
        .decode(certificate.signature.trim())
        .with_context(|| "license signature must be base64 encoded")?;
    let signature_array: [u8; 64] = signature_bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("invalid license signature length"))?;
    let signature = Signature::from_bytes(&signature_array);
    let payload = to_vec(&certificate.payload)
        .map_err(|err| anyhow!("failed to serialise license payload: {err}"))?;

    key.verify_strict(&payload, &signature)
        .map_err(|err| anyhow!("license signature verification failed: {err}"))?;
    Ok(())
}
