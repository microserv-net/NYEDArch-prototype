//! Does this capsule still consist of the bytes it was built from?
//!
//! The capsule already authenticates the package it carries: any edit there is
//! refused. It did not check the **image it runs as**, so editing the binary
//! elsewhere went unnoticed — found by attacking a delivered capsule, not by
//! review, and recorded as a known limitation until now.
//!
//! # Why this is not the confidentiality boundary
//!
//! It is not, and it must not be mistaken for one. The payload key is composed
//! from the authorization factors, so patching a branch yields no key material
//! whatever this check does. What this adds is **detection**: an attacker who
//! modifies the binary is noticed and fails closed, which costs them the
//! specimen rather than giving them a free iteration. That is defence in depth
//! (spec §30), and its value is in raising the cost of experimenting.
//!
//! # The chicken and egg
//!
//! A binary cannot contain a hash of itself — writing the hash changes the
//! bytes being hashed. The usual resolution, used here: reserve a field, hash
//! the image with that field **zeroed**, then write the result into it. The
//! runtime does the same in reverse, so both sides hash identical bytes.
//!
//! The field is found by scanning for a magic prefix rather than by a recorded
//! offset, because the linker decides where static data lands and an offset
//! computed at build time is a promise about layout nobody made.
//!
//! # Unsealed binaries still run
//!
//! If the hash field is all zeroes, no seal was ever applied — a local
//! development build, or a build whose post-processing step did not run. The
//! check reports "not sealed" and the capsule proceeds.
//!
//! That is deliberate. The alternative is a capsule that refuses to run because
//! a build step was skipped, which converts a missing defence-in-depth layer
//! into total loss of access. A missing seal is a weaker capsule; a
//! false-positive seal is a destroyed one.

/// Marks the reserved field. Chosen to be improbable in compiled output and
/// searched for at run time, since the linker chooses the layout.
/// Built at run time rather than stored as a literal.
///
/// A literal would place a *second* copy of the magic in the image — the
/// library's own constant, next to the capsule's reserved field — and the
/// scanner would find whichever came first. It found the library's copy,
/// treated the unrelated bytes after it as a stored digest, and every unsealed
/// capsule refused its own owner. Exactly the false positive this module is
/// supposed to avoid.
pub fn self_seal_magic() -> [u8; 16] {
    let mut m = [0u8; 16];
    // XOR-masked so the plaintext bytes never appear in the library's data.
    const MASKED: [u8; 16] = [0x0c, 0x1b, 0x07, 0x06, 0x03, 0x10, 0x01, 0x0a, 0x6f, 0x11, 0x07, 0x03, 0x0e, 0x34, 0x73, 0x42];
    let mut i = 0;
    while i < 16 {
        m[i] = MASKED[i] ^ 0x42;
        i += 1;
    }
    m
}

/// Magic followed by a 32-byte digest field.
pub const SELF_SEAL_LEN: usize = 16 + 32;

/// What a check concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfSeal {
    /// The image matches the digest written into it.
    Intact,
    /// The image does not match: it has been modified since it was sealed.
    Modified,
    /// No digest was written. The capsule was never sealed.
    NotSealed,
    /// The image could not be read or the field could not be found. Treated as
    /// unsealed rather than as tampering: a capsule that cannot read its own
    /// file (an unusual filesystem, an odd launcher) should not refuse a
    /// legitimate owner over it.
    Unknown,
}

/// Locate the reserved field in an image.
pub fn find_fields(image: &[u8]) -> Vec<usize> {
    let magic = self_seal_magic();
    image
        .windows(SELF_SEAL_LEN)
        .enumerate()
        .filter(|(_, w)| w[..16] == magic)
        .map(|(i, _)| i)
        .collect()
}

/// The first candidate field, for callers that only need one.
pub fn find_field(image: &[u8]) -> Option<usize> {
    find_fields(image).into_iter().next()
}

/// The digest an image should carry: the image itself with the digest field
/// zeroed. Shared by the sealing tool and the runtime so the two cannot
/// disagree about what is being hashed.
pub fn expected_digest(image: &[u8], field_at: usize) -> Option<[u8; 32]> {
    let start = field_at.checked_add(16)?;
    let end = start.checked_add(32)?;
    if end > image.len() {
        return None;
    }
    let mut buf = image.to_vec();
    buf[start..end].fill(0);
    // The project's own domain-separated KDF rather than a bare hash: it is
    // already audited, already versioned, and using it here means one fewer
    // construction to reason about. The domain string keeps this digest from
    // colliding with any other use of the same primitive.
    let key = nyedarch_crypto::kdf::hkdf_key(b"nyedarch:v1:self-seal", &buf, b"image").ok()?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&key[..32]);
    Some(out)
}

/// Write a digest into an image, returning the sealed bytes.
pub fn seal_image(image: &[u8]) -> Option<Vec<u8>> {
    let at = find_field(image)?;
    let digest = expected_digest(image, at)?;
    let mut out = image.to_vec();
    out[at + 16..at + 48].copy_from_slice(&digest);
    Some(out)
}

/// Check an image against the digest it carries.
pub fn check_image(image: &[u8]) -> SelfSeal {
    // Every candidate is considered, not just the first.
    //
    // More than one occurrence of the magic can exist, and picking one by
    // position is a guess. An image is intact if *any* candidate verifies, and
    // unsealed if any candidate is still zeroed - only when neither holds has
    // something actually been modified.
    let candidates = find_fields(image);
    if candidates.is_empty() {
        return SelfSeal::Unknown;
    }
    let mut saw_unsealed = false;
    for at in &candidates {
        let stored = &image[at + 16..at + 48];
        if stored.iter().all(|b| *b == 0) {
            saw_unsealed = true;
            continue;
        }
        if let Some(d) = expected_digest(image, *at) {
            let mut diff = 0u8;
            for (a, b) in d.iter().zip(stored.iter()) {
                diff |= a ^ b;
            }
            if diff == 0 {
                return SelfSeal::Intact;
            }
        }
    }
    if saw_unsealed {
        return SelfSeal::NotSealed;
    }
    SelfSeal::Modified
}

/// Check the running executable.
pub fn check_self() -> SelfSeal {
    let Ok(path) = std::env::current_exe() else {
        return SelfSeal::Unknown;
    };
    match std::fs::read(path) {
        Ok(bytes) => check_image(&bytes),
        Err(_) => SelfSeal::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in image: the field embedded in surrounding noise, as it would
    /// be in a real binary.
    fn image_with_field() -> Vec<u8> {
        let mut v = b"....machine code....".repeat(20);
        v.extend_from_slice(&self_seal_magic());
        v.extend_from_slice(&[0u8; 32]);
        v.extend_from_slice(&b"....more code....".repeat(20));
        v
    }

    #[test]
    fn an_unsealed_image_is_recognised_as_unsealed() {
        assert_eq!(check_image(&image_with_field()), SelfSeal::NotSealed);
    }

    #[test]
    fn a_sealed_image_verifies() {
        let sealed = seal_image(&image_with_field()).expect("seals");
        assert_eq!(check_image(&sealed), SelfSeal::Intact);
    }

    /// The point of the whole module: any edit outside the field is detected.
    #[test]
    fn any_edit_after_sealing_is_detected() {
        let sealed = seal_image(&image_with_field()).expect("seals");
        let at = find_field(&sealed).unwrap();

        // Every byte, one at a time, outside the digest field itself.
        let mut missed = Vec::new();
        for i in 0..sealed.len() {
            if (at + 16..at + 48).contains(&i) {
                continue;
            }
            let mut edited = sealed.clone();
            edited[i] = edited[i].wrapping_add(1);
            if check_image(&edited) == SelfSeal::Intact {
                missed.push(i);
            }
        }
        assert!(
            missed.is_empty(),
            "edits at {missed:?} were not detected; the seal covers less than the image"
        );
    }

    /// Editing the digest itself is also a modification, not a downgrade to
    /// "unsealed" - otherwise an attacker could erase the seal to disable it.
    #[test]
    fn the_digest_cannot_be_edited_to_disable_the_check() {
        let sealed = seal_image(&image_with_field()).expect("seals");
        let at = find_field(&sealed).unwrap();

        let mut flipped = sealed.clone();
        flipped[at + 20] ^= 0xFF;
        assert_eq!(check_image(&flipped), SelfSeal::Modified);

        // Zeroing it claims "never sealed". That is a real downgrade and it is
        // documented: the seal detects modification, it cannot prevent removal.
        // What it costs the attacker is the ability to modify silently.
        let mut zeroed = sealed.clone();
        zeroed[at + 16..at + 48].fill(0);
        assert_eq!(check_image(&zeroed), SelfSeal::NotSealed);
    }

    #[test]
    fn an_image_without_the_field_is_unknown_not_modified() {
        assert_eq!(check_image(b"no field here at all"), SelfSeal::Unknown);
    }
}
