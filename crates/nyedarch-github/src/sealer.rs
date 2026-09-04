//! libsodium sealed-box sealer for GitHub Actions secrets.
//!
//! GitHub requires repository secrets to be encrypted client-side with the
//! repository's Curve25519 public key. This is mandatory for any remote build,
//! so it is compiled in by default and pulls only `crypto_box` - no HTTP stack.
//!
//! A secret is never transmitted unsealed. If this is unavailable the client
//! refuses to send one at all rather than sending it in the clear.

#![cfg(feature = "sealer")]

use crate::http::{GhError, SecretSealer};

/// libsodium sealed-box sealer for GitHub Actions secrets. GitHub encrypts
/// repository secrets with the repo's Curve25519 public key; only the runner can
/// decrypt them. We never transmit a secret unsealed (spec §48).
pub struct SodiumSealer;

impl SecretSealer for SodiumSealer {
    fn seal(&self, repo_public_key_b64: &str, plaintext: &[u8]) -> Result<String, GhError> {
        use crypto_box::{aead::OsRng, PublicKey};
        let raw = base64_decode(repo_public_key_b64).ok_or(GhError::Sealer)?;
        let arr: [u8; 32] = raw.try_into().map_err(|_| GhError::Sealer)?;
        let pk = PublicKey::from(arr);
        let sealed = pk.seal(&mut OsRng, plaintext).map_err(|_| GhError::Sealer)?;
        Ok(crate::base64::encode(&sealed))
    }
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    // Standard base64 decode (padding tolerant).
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let s: Vec<u8> = s.bytes().filter(|&b| b != b'=' && !b.is_ascii_whitespace()).collect();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.chunks(4) {
        let mut n = 0u32;
        let mut bits = 0;
        for &c in chunk {
            n = (n << 6) | val(c)? as u32;
            bits += 6;
        }
        n <<= 24 - bits;
        let bytes = bits / 8;
        for i in 0..bytes {
            out.push((n >> (16 - i * 8)) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::SecretSealer;

    /// A sealed secret must decrypt back to the original with the matching
    /// secret key, and must never resemble the plaintext.
    #[test]
    fn sealed_box_roundtrips_and_hides_the_secret() {
        use crypto_box::{aead::OsRng, SecretKey};

        let sk = SecretKey::generate(&mut OsRng);
        let pk = sk.public_key();
        let pk_b64 = crate::base64::encode(pk.as_bytes());

        let secret = b"NYEDARCH_GH_TO_CLIENT_PUB=super-secret-value";
        let sealed_b64 = SodiumSealer.seal(&pk_b64, secret).expect("seal");

        // The ciphertext must not contain the plaintext.
        assert!(!sealed_b64.contains("super-secret"));

        // And it must open with the private half.
        let raw = base64_decode(&sealed_b64).expect("b64");
        let opened = sk.unseal(&raw).expect("unseal");
        assert_eq!(opened, secret);
    }

    #[test]
    fn a_malformed_public_key_is_refused() {
        assert!(SodiumSealer.seal("not-base64!!", b"x").is_err());
        assert!(SodiumSealer.seal("c2hvcnQ=", b"x").is_err()); // right encoding, wrong length
    }
}
