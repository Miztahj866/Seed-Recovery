//! license.rs — Offline license key validation via Ed25519 signatures.
//!
//! Design goal: the app should be able to verify a license key entirely
//! locally, with no network call, while still making it computationally
//! infeasible for someone to mint their own valid keys just by inspecting
//! the shipped binary.
//!
//! This is achieved with asymmetric signing:
//!   - The SELLER holds a private signing key, generated once and kept
//!     secret (never committed to this repo, never shipped in the app).
//!   - The APP only ever embeds the corresponding PUBLIC key (see
//!     `PUBLIC_KEY_BYTES` below) — a public key can verify signatures but
//!     cannot be used to create new ones.
//!   - After a sale, the seller runs `cargo run --bin gen_license -- issue
//!     <some-id>` to produce a license key string, using the private key
//!     that never leaves their machine.
//!
//! IMPORTANT LIMITATION (state this honestly, don't hide it): no client-side
//! license check — cryptographic or otherwise — can stop someone willing to
//! patch the compiled binary itself to skip the check entirely. This scheme
//! only stops casual key-guessing/forging; it does not and cannot stop
//! binary patching. That's a limitation of ALL offline license schemes, not
//! specific to this implementation, and is a known, accepted tradeoff for
//! keeping the app fully offline-capable rather than requiring a
//! phone-home activation server (which would reintroduce a network
//! dependency this project deliberately avoids).

use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// The seller's PUBLIC key, embedded at compile time. This is safe to be
/// public — it can only verify signatures, never create them.
///
/// PLACEHOLDER: all zeros. Replace this with your real public key (32
/// bytes) after running `gen_license genkey` — see docs/LICENSING.md for
/// the full walkthrough. Until replaced, license validation will always
/// fail closed (reject every key), which is the safe default: better to
/// have paid-tier features refuse to unlock than to accidentally ship a
/// placeholder that accepts everything.
const PUBLIC_KEY_BYTES: [u8; 32] = [7, 186, 27, 13, 93, 92, 178, 33, 79, 252, 173, 127, 130, 14, 23, 171, 190, 36, 104, 88, 133, 211, 4, 104, 137, 254, 157, 225, 138, 240, 116, 159];

#[derive(Debug, Clone)]
pub struct LicenseInfo {
    pub license_id: String,
}

/// Validate a license key string of the form "<payload_b64>.<signature_b64>".
/// Returns Ok(LicenseInfo) only if the signature verifies against the
/// embedded public key. Never makes a network call.
pub fn validate_license_key(key: &str) -> Result<LicenseInfo, String> {
    let key = key.trim();
    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() != 2 {
        return Err("Malformed license key (expected 'payload.signature').".into());
    }

    let payload_bytes = STANDARD
        .decode(parts[0])
        .map_err(|e| format!("Invalid license key encoding: {e}"))?;
    let signature_bytes = STANDARD
        .decode(parts[1])
        .map_err(|e| format!("Invalid license key encoding: {e}"))?;

    let signature_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| "Invalid signature length in license key.".to_string())?;
    let signature = Signature::from_bytes(&signature_array);

    let verifying_key = VerifyingKey::from_bytes(&PUBLIC_KEY_BYTES)
        .map_err(|e| format!("Invalid embedded public key (this is a bug, not a bad license): {e}"))?;

    verifying_key
        .verify(&payload_bytes, &signature)
        .map_err(|_| "License key signature is invalid.".to_string())?;

    let license_id = String::from_utf8(payload_bytes)
        .map_err(|_| "License payload is not valid UTF-8.".to_string())?;

    Ok(LicenseInfo { license_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    /// Generates a throwaway keypair and signs a test payload, purely to
    /// exercise the malformed-input and verification-failure paths. This
    /// does NOT test against the real embedded PUBLIC_KEY_BYTES (which is
    /// the all-zero placeholder) — that's intentional, see the two
    /// dedicated tests below for placeholder-specific behavior.
    fn make_test_keypair_and_key(payload: &str) -> (SigningKey, String) {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let signature = signing_key.sign(payload.as_bytes());
        let key_str = format!(
            "{}.{}",
            STANDARD.encode(payload.as_bytes()),
            STANDARD.encode(signature.to_bytes())
        );
        (signing_key, key_str)
    }

    #[test]
    fn malformed_key_rejected() {
        assert!(validate_license_key("not-a-valid-key").is_err());
        assert!(validate_license_key("too.many.dots.here").is_err());
        assert!(validate_license_key("").is_err());
    }

    #[test]
    fn placeholder_public_key_rejects_everything() {
        // With PUBLIC_KEY_BYTES still at its all-zero placeholder, ANY
        // signature (even one signed by a real, freshly generated keypair)
        // must fail to verify, since it wasn't signed by the all-zero key.
        // This confirms the fail-closed default actually fails closed.
        let (_signing_key, key_str) = make_test_keypair_and_key("test-license-001");
        assert!(
            validate_license_key(&key_str).is_err(),
            "placeholder public key should reject all keys until replaced with a real one"
        );
    }

    #[test]
    fn tampered_payload_is_rejected() {
        // Even against a real keypair (not the embedded placeholder), a
        // payload edited after signing must fail verification. We can't
        // test this against the real embedded key without replacing the
        // placeholder, so this test verifies the underlying crypto
        // primitive behaves correctly in isolation.
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        let signature = signing_key.sign(b"original-payload");

        assert!(verifying_key.verify(b"original-payload", &signature).is_ok());
        assert!(
            verifying_key.verify(b"tampered-payload", &signature).is_err(),
            "signature must not verify against a different payload"
        );
    }
}
