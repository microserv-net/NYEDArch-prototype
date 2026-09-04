//! Artifact provenance (correction pass §6).
//!
//! GitHub is an UNTRUSTED external build environment. A matching hash proves
//! only "these bytes are those bytes" — it does not establish that the artifact
//! is *the expected output for this specific build*. The client must be able to
//! answer:
//!
//! > Is this artifact the output for THIS build id, THIS target, THIS package
//! > commitment, and THIS runtime configuration?
//!
//! So the client commits to the build request BEFORE dispatch, and verifies the
//! returned artifact against that commitment. Anything unexpected fails closed
//! (spec §29) — the client never trusts an artifact merely because GitHub
//! returned it (spec §47).

use crate::workflow::Target;

/// What the client commits to before dispatching a build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildManifest {
    /// ULID string identifying this build (spec §39).
    pub build_id: String,
    /// SHA-256 of the sealed package embedded in the runtime.
    pub package_commitment: [u8; 32],
    /// The runtime's identity commitment (also embedded in the binary, §4).
    pub runtime_commitment: [u8; 32],
    /// SHA-256 over the generated runtime source tree.
    pub source_commitment: [u8; 32],
    pub target: Target,
    pub crypto_version: u16,
}

/// What the client observes after the build.
#[derive(Clone, Debug)]
pub struct ArtifactClaim {
    pub build_id: String,
    pub target: Target,
    /// SHA-256 of the downloaded artifact bytes.
    pub artifact_digest: [u8; 32],
    /// Signature over `expected_digest_input`, made with the GitHub→Client
    /// signing key whose public half the client generated and stored as a
    /// repository secret (spec §42: directional keypairs, never reused).
    pub signature: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProvenanceError {
    BuildIdMismatch,
    TargetMismatch,
    DigestMismatch,
    SignatureInvalid,
}

/// The exact byte string a valid signature must cover. Binding every field
/// means a signature captured from one build cannot be replayed onto another
/// (different build id, target, or package ⇒ different signed input).
pub fn signed_input(m: &BuildManifest, artifact_digest: &[u8; 32]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"nyedarch:v1:artifact-provenance");
    v.extend_from_slice(&m.crypto_version.to_le_bytes());
    v.extend_from_slice(m.build_id.as_bytes());
    v.push(0);
    v.extend_from_slice(m.target.rust_target().as_bytes());
    v.push(0);
    v.extend_from_slice(&m.package_commitment);
    v.extend_from_slice(&m.runtime_commitment);
    v.extend_from_slice(&m.source_commitment);
    v.extend_from_slice(artifact_digest);
    v
}

/// Verifier for the GitHub→Client direction. Implemented on the client with the
/// verifying key it holds locally; kept as a trait so the signature scheme can
/// be swapped without touching orchestration (spec §54 crypto agility).
pub trait ArtifactVerifier {
    fn verify(&self, signed_input: &[u8], signature: &[u8]) -> bool;
}

/// Establish provenance. Every check must pass; the digest comparison is
/// constant-time. Returns `Ok(())` only when the artifact is provably the
/// expected output of the committed build.
pub fn verify_artifact<V: ArtifactVerifier + ?Sized>(
    verifier: &V,
    manifest: &BuildManifest,
    claim: &ArtifactClaim,
    expected_artifact_digest: &[u8; 32],
) -> Result<(), ProvenanceError> {
    if manifest.build_id != claim.build_id {
        return Err(ProvenanceError::BuildIdMismatch);
    }
    if manifest.target != claim.target {
        return Err(ProvenanceError::TargetMismatch);
    }
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= claim.artifact_digest[i] ^ expected_artifact_digest[i];
    }
    if diff != 0 {
        return Err(ProvenanceError::DigestMismatch);
    }
    let input = signed_input(manifest, &claim.artifact_digest);
    if !verifier.verify(&input, &claim.signature) {
        return Err(ProvenanceError::SignatureInvalid);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AcceptIf(Vec<u8>);
    impl ArtifactVerifier for AcceptIf {
        fn verify(&self, input: &[u8], sig: &[u8]) -> bool {
            sig == self.0.as_slice() && !input.is_empty()
        }
    }

    fn manifest() -> BuildManifest {
        BuildManifest {
            build_id: "01HZY0000000000000000000AA".into(),
            package_commitment: [1u8; 32],
            runtime_commitment: [2u8; 32],
            source_commitment: [3u8; 32],
            target: Target::LinuxGnu,
            crypto_version: 1,
        }
    }

    #[test]
    fn accepts_expected_artifact() {
        let m = manifest();
        let d = [7u8; 32];
        let sig = b"good-sig".to_vec();
        let claim = ArtifactClaim { build_id: m.build_id.clone(), target: m.target, artifact_digest: d, signature: sig.clone() };
        assert_eq!(verify_artifact(&AcceptIf(sig), &m, &claim, &d), Ok(()));
    }

    #[test]
    fn rejects_wrong_build_target_or_digest() {
        let m = manifest();
        let d = [7u8; 32];
        let sig = b"good-sig".to_vec();
        let v = AcceptIf(sig.clone());

        let mut c = ArtifactClaim { build_id: "OTHER".into(), target: m.target, artifact_digest: d, signature: sig.clone() };
        assert_eq!(verify_artifact(&v, &m, &c, &d), Err(ProvenanceError::BuildIdMismatch));

        c = ArtifactClaim { build_id: m.build_id.clone(), target: Target::WindowsMsvc, artifact_digest: d, signature: sig.clone() };
        assert_eq!(verify_artifact(&v, &m, &c, &d), Err(ProvenanceError::TargetMismatch));

        c = ArtifactClaim { build_id: m.build_id.clone(), target: m.target, artifact_digest: [9u8; 32], signature: sig.clone() };
        assert_eq!(verify_artifact(&v, &m, &c, &d), Err(ProvenanceError::DigestMismatch));
    }

    #[test]
    fn rejects_unsigned_or_replayed_signature() {
        let m = manifest();
        let d = [7u8; 32];
        // A signature from a *different* build cannot be replayed: the signed
        // input covers build id, target, and all three commitments.
        let claim = ArtifactClaim { build_id: m.build_id.clone(), target: m.target, artifact_digest: d, signature: b"sig-from-another-build".to_vec() };
        assert_eq!(verify_artifact(&AcceptIf(b"good-sig".to_vec()), &m, &claim, &d), Err(ProvenanceError::SignatureInvalid));

        // Signed input is build-specific.
        let mut other = manifest();
        other.build_id = "01HZY0000000000000000000BB".into();
        assert_ne!(signed_input(&m, &d), signed_input(&other, &d));
    }
}
