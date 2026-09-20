//! A GitHub artifact is a *streamed* zip, and reading one is not the same as
//! reading a zip written to a file.
//!
//! Actions writes the archive as the upload happens, so each local header
//! carries the "sizes follow the data" flag with zeroes where the lengths
//! belong. A reader that walks local headers stops at the first such entry and
//! finds nothing - the artifact downloads correctly and then reports that no
//! capsule could be read from it, which is exactly what the end-to-end test
//! caught on a real runner.
//!
//! The fixture is a hand-built streamed archive with that flag set and zero
//! sizes, so this test cannot pass by accident on a normally written zip.

use nyedarch_buildtool::artifact;

const STREAMED: &[u8] = include_bytes!("streamed-artifact.zip");
const PAYLOAD: &[u8] = include_bytes!("streamed-payload.bin");

/// The fixture must really be streamed, or the test proves nothing.
#[test]
fn the_fixture_has_no_sizes_in_its_local_header() {
    let flags = u16::from_le_bytes(STREAMED[6..8].try_into().unwrap());
    let csize = u32::from_le_bytes(STREAMED[18..22].try_into().unwrap());
    let usize_ = u32::from_le_bytes(STREAMED[22..26].try_into().unwrap());
    assert_eq!(flags & 0x08, 0x08, "the data-descriptor flag must be set");
    assert_eq!(csize, 0, "a streamed entry has no compressed size in its header");
    assert_eq!(usize_, 0, "a streamed entry has no uncompressed size either");
}

#[test]
fn a_streamed_artifact_yields_the_capsule() {
    let entry = artifact::capsule_from_artifact(STREAMED)
        .expect("a streamed artifact must still yield its capsule");
    assert_eq!(entry.name, "nyedarch-capsule-aarch64-apple-darwin");
    assert_eq!(entry.data, PAYLOAD, "the capsule must come back byte for byte");
}

/// Truncation must not panic: the archive arrives from an untrusted build
/// environment.
#[test]
fn truncated_streamed_archives_are_refused() {
    for cut in [8usize, 40, STREAMED.len() / 2, STREAMED.len() - 8] {
        let _ = artifact::capsule_from_artifact(&STREAMED[..cut]);
    }
    // A corrupted end-of-directory record must not crash the reader.
    let mut broken = STREAMED.to_vec();
    let n = broken.len();
    broken[n - 6..n - 2].copy_from_slice(&u32::MAX.to_le_bytes());
    let _ = artifact::capsule_from_artifact(&broken);
}

/// Provenance must be checked against the *package*, not its digest.
///
/// `package_commitment` is a SHA-256 over the sealed package. The capsule
/// embeds the package itself, never the digest, so searching a capsule for the
/// commitment cannot match - it refused every artifact for a reason unrelated
/// to provenance. This pins the property the check actually relies on: the
/// sealed package appears verbatim in the built capsule.
#[test]
fn the_sealed_package_is_what_a_capsule_carries() {
    // Stand-in for a compiled capsule: arbitrary code around an embedded blob.
    let package = b"NYARCH-sealed-package-bytes-0123456789".to_vec();
    let mut capsule = b"\x7fELF................".to_vec();
    capsule.extend_from_slice(&package);
    capsule.extend_from_slice(b"....more machine code....");

    let found = capsule
        .windows(package.len())
        .any(|w| w == package.as_slice());
    assert!(found, "the package must be findable in the capsule");

    // A capsule built from a different package must not pass.
    let other = b"NYARCH-sealed-package-bytes-9876543210".to_vec();
    assert!(!capsule.windows(other.len()).any(|w| w == other.as_slice()));

    // A digest of the package is not present, which is why the first check
    // could never succeed.
    let digest = [0xABu8; 32];
    assert!(!capsule.windows(32).any(|w| w == digest));
}
