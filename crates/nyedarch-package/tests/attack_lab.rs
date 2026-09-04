//! Attack laboratory (context 5 §8, item B2).
//!
//! Every attack here is **executed**, not proposed. Each one takes a genuinely
//! sealed package, modifies it the way an attacker holding the artifact would,
//! and records what the implementation does.
//!
//! The single property under test throughout:
//!
//! > modifying the artifact must never yield plaintext — it must yield a
//! > refusal, and the refusal must not depend on a branch that could be patched
//! > away.
//!
//! Failures here are authentication failures, which is the point: the AEAD tag
//! covers the header, the policy, the manifest and every chunk, so tampering is
//! detected by cryptography rather than by validation logic an attacker could
//! remove.

use nyedarch_core::Policy;
use nyedarch_crypto::{compose, passphrase_key, version, Argon2Params, Binding, Contributions};
use nyedarch_package::format::*;
use nyedarch_package::format::parse;
use nyedarch_package::{collect, restore, seal_package};

/// Unique per call. Timestamps are not enough: tests run in parallel and two
/// starting in the same nanosecond shared a directory, so one test read
/// another's output and reported a leak that had not happened.
static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn unique(tag: &str) -> String {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}-{}-{}", std::process::id(), tag, n)
}

fn tmpdir(tag: &str) -> std::path::PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!("nyedarch-attack-{}", unique(tag)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn tiny() -> Argon2Params {
    // Small parameters keep the laboratory fast; they do not change the
    // structure being attacked.
    Argon2Params { m_cost: 8, t_cost: 1, p_cost: 1 }
}

struct Specimen {
    package: Vec<u8>,
    key: [u8; 32],
    dir: std::path::PathBuf,
    payload: Vec<u8>,
}

/// Build a real capsule package to attack.
///
/// The identity is derived from `tag`, so two specimens are genuinely different
/// capsules. With fixed constants they would share a key, and a
/// transplantation test would "fail" merely because both halves were the same
/// capsule - which is what happened first time.
fn specimen(tag: &str) -> Specimen {
    let mut seed = [0u8; 32];
    for (i, b) in tag.bytes().enumerate() {
        seed[i % 32] ^= b.wrapping_mul(31).wrapping_add(i as u8);
    }
    let dir = tmpdir(tag);
    let src = dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let payload = b"CLASSIFIED-PAYLOAD-MARKER-0123456789".repeat(40);
    std::fs::write(src.join("secret.bin"), &payload).unwrap();

    let (manifest, files) = collect(&[src.clone()]).unwrap();
    let mut salt = [0x42u8; 32];
    let mut runtime_binding = [0x24u8; 32];
    let mut package_id = *b"attack-lab-pkg01";
    for i in 0..32 {
        salt[i] ^= seed[i];
        runtime_binding[i] ^= seed[(i + 7) % 32];
    }
    for i in 0..16 {
        package_id[i] ^= seed[(i + 3) % 32];
    }
    let binding = Binding {
        package_id,
        runtime_binding,
        crypto_version: version::CRYPTO_VERSION,
    };
    let policy = Policy { location: false, time: false, one_shot: false };
    let flags = policy.to_flags();

    let pass = passphrase_key(b"correct-horse", &salt, tiny()).unwrap();
    let mut machine = [0xAAu8; 32];
    for i in 0..32 {
        machine[i] ^= seed[(i + 11) % 32];
    }
    let contrib = Contributions {
        machine_secret: Some(machine),
        passphrase_key: Some(pass),
        location_cell: None,
        time_window: None,
    };
    let key = *compose::derive_payload_key(&salt, &binding, flags, &contrib).unwrap();

    let header = Header {
        crypto_version: version::CRYPTO_VERSION,
        package_id: binding.package_id,
        runtime_binding: binding.runtime_binding,
        policy,
        argon: tiny().into(),
        package_salt: salt,
        compression: COMPRESSION_ZSTD,
        chunk_size: 4096,
    };
    let package = seal_package(&header, b"", &key, &manifest, &files).unwrap();

    Specimen { package, key, dir, payload }
}

/// Attempt extraction and report whether any plaintext escaped.
///
/// Both halves matter: a refusal that still wrote files would be a failure, so
/// the output directory is inspected rather than trusting the return value.
fn attempt(pkg: &[u8], key: &[u8; 32], dir: &std::path::Path, marker: &[u8]) -> (bool, bool) {
    let out = dir.join(format!("out-{}", unique("attempt")));
    let opened = match parse(pkg) {
        Ok(parsed) => restore(&parsed, key, &out).is_ok(),
        Err(_) => false,
    };
    let mut leaked = false;
    if out.exists() {
        let mut stack = vec![out.clone()];
        while let Some(d) = stack.pop() {
            if let Ok(rd) = std::fs::read_dir(&d) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if let Ok(b) = std::fs::read(&p) {
                        if b.windows(marker.len()).any(|w| w == marker) {
                            leaked = true;
                        }
                    }
                }
            }
        }
    }
    (opened, leaked)
}

const MARKER: &[u8] = b"CLASSIFIED-PAYLOAD-MARKER";

/// A0 — the control. Without it, every result below is meaningless: a package
/// that never opened would "resist" every attack trivially.
#[test]
fn a0_control_the_unmodified_specimen_opens() {
    let s = specimen("a0");
    let (opened, leaked) = attempt(&s.package, &s.key, &s.dir, MARKER);
    assert!(opened, "the control must open, or the laboratory proves nothing");
    assert!(leaked, "the control must produce the payload");
    assert!(s.payload.len() > 100);
}

/// A1 — flip a byte in the ciphertext body.
#[test]
fn a1_ciphertext_modification_is_refused() {
    let s = specimen("a1");
    let mut pkg = s.package.clone();
    let at = pkg.len() * 3 / 4;
    pkg[at] ^= 0x01;
    let (opened, leaked) = attempt(&pkg, &s.key, &s.dir, MARKER);
    assert!(!opened, "a modified chunk must not decrypt");
    assert!(!leaked, "no plaintext may reach disk");
}

/// A2 — truncate the package.
#[test]
fn a2_truncation_is_refused() {
    let s = specimen("a2");
    for frac in [9, 7, 5, 2] {
        let pkg = s.package[..s.package.len() * frac / 10].to_vec();
        let (opened, leaked) = attempt(&pkg, &s.key, &s.dir, MARKER);
        assert!(!opened, "a truncated package must not open (kept {frac}/10)");
        assert!(!leaked);
    }
}

/// A3 — corrupt the header. The header is authenticated data, so changing it
/// invalidates every chunk rather than merely being ignored.
#[test]
fn a3_header_modification_is_refused() {
    let s = specimen("a3");
    for offset in [8usize, 16, 24, 40] {
        let mut pkg = s.package.clone();
        if offset < pkg.len() {
            pkg[offset] ^= 0xFF;
            let (opened, leaked) = attempt(&pkg, &s.key, &s.dir, MARKER);
            assert!(!opened, "header byte {offset} altered but the package still opened");
            assert!(!leaked);
        }
    }
}

/// A4 — flip the policy flags, the classic "turn the protections off" attack.
#[test]
fn a4_policy_downgrade_is_refused() {
    let s = specimen("a4");
    // The flags participate in key derivation, so clearing them changes the key
    // rather than relaxing a check.
    let mut found = false;
    for i in 0..s.package.len().min(96) {
        let mut pkg = s.package.clone();
        let before = pkg[i];
        pkg[i] = before | 0x07; // set location/time/one-shot style bits
        if pkg[i] != before {
            found = true;
            let (opened, leaked) = attempt(&pkg, &s.key, &s.dir, MARKER);
            assert!(!opened, "policy byte {i} altered but the package still opened");
            assert!(!leaked);
        }
    }
    assert!(found, "the header region should contain modifiable bytes");
}

/// A5 — swap two payload chunks. The chunk index is authenticated, so a
/// reordered stream is not merely out of order, it fails to authenticate.
#[test]
fn a5_chunk_reordering_is_refused() {
    let s = specimen("a5");
    let mut pkg = s.package.clone();
    let n = pkg.len();
    if n > 4096 {
        let (a, b) = (n / 3, 2 * n / 3);
        for k in 0..512 {
            pkg.swap(a + k, b + k);
        }
        let (opened, leaked) = attempt(&pkg, &s.key, &s.dir, MARKER);
        assert!(!opened, "reordered chunks must not authenticate");
        assert!(!leaked);
    }
}

/// A6 — the wrong key. This is what an attacker who cannot satisfy a protection
/// actually holds: material of the right shape and the wrong value.
#[test]
fn a6_a_wrong_key_yields_nothing() {
    let s = specimen("a6");
    let mut wrong = s.key;
    wrong[0] ^= 0x01;
    let (opened, leaked) = attempt(&s.package, &wrong, &s.dir, MARKER);
    assert!(!opened, "a key differing in one bit must not open the package");
    assert!(!leaked);
}

/// A7 — splice the ciphertext of one capsule into another's container.
#[test]
fn a7_payload_transplantation_is_refused() {
    let a = specimen("a7a");
    let b = specimen("a7b");
    let cut = a.package.len().min(b.package.len()) / 2;

    // A's container, B's tail.
    let mut hybrid = a.package[..cut].to_vec();
    hybrid.extend_from_slice(&b.package[cut..]);
    let (opened, leaked) = attempt(&hybrid, &a.key, &a.dir, MARKER);
    assert!(!opened, "a spliced package must not open");
    assert!(!leaked);

    // And B's package must not open with A's key.
    let (opened2, leaked2) = attempt(&b.package, &a.key, &a.dir, MARKER);
    assert!(!opened2, "another capsule's package must not open with this key");
    assert!(!leaked2);
}

/// A8 — append trailing data, as a parser-confusion attempt.
#[test]
fn a8_appended_data_does_not_change_the_outcome() {
    let s = specimen("a8");
    let mut pkg = s.package.clone();
    pkg.extend_from_slice(&[0xEE; 4096]);
    let (opened, leaked) = attempt(&pkg, &s.key, &s.dir, MARKER);
    // Either it is rejected outright or the trailer is ignored; what must never
    // happen is the appended bytes influencing what is produced.
    if opened {
        assert!(leaked, "if it opened, it must have produced the real payload");
    } else {
        assert!(!leaked);
    }
}

/// A9 — a package of zeroes, and other degenerate inputs. These must fail
/// cleanly rather than panicking: a panic in a capsule is a denial-of-service
/// and can leak state through its message.
#[test]
fn a9_degenerate_inputs_fail_cleanly() {
    let s = specimen("a9");
    for pkg in [
        vec![],
        vec![0u8; 16],
        vec![0u8; 4096],
        vec![0xFFu8; 1024],
        b"NYARCH".to_vec(),
    ] {
        let (opened, leaked) = attempt(&pkg, &s.key, &s.dir, MARKER);
        assert!(!opened);
        assert!(!leaked);
    }
}

/// A10 — bit-flip sweep across the whole artifact.
///
/// The single strongest statement the laboratory can make: not "these specific
/// edits are caught", but "no single-bit edit anywhere produced plaintext".
#[test]
fn a10_no_single_bit_flip_anywhere_yields_plaintext() {
    let s = specimen("a10");
    let step = (s.package.len() / 60).max(1); // sample across the whole file
    let mut tested = 0;
    let mut i = 0;
    while i < s.package.len() {
        for bit in [0u8, 3, 7] {
            let mut pkg = s.package.clone();
            pkg[i] ^= 1 << bit;
            if pkg == s.package {
                continue;
            }
            let (opened, leaked) = attempt(&pkg, &s.key, &s.dir, MARKER);
            assert!(!leaked, "byte {i} bit {bit}: plaintext escaped");
            assert!(!opened, "byte {i} bit {bit}: a modified package opened");
            tested += 1;
        }
        i += step;
    }
    assert!(tested >= 100, "the sweep should cover the artifact broadly, ran {tested}");
}

/// A11 — a tampered length field must not become an allocation request.
///
/// The bit-flip sweep hit a chunk count that asked for 51 GB and aborted the
/// process. An abort is not a refusal: it is a denial of service, and it
/// bypasses the fail-closed path entirely.
#[test]
fn a11_length_fields_cannot_drive_allocation() {
    let s = specimen("a11");

    // Walk the header/prefix region, where the length fields live, and set each
    // byte to 0xFF - the largest value the field can express.
    for i in 0..s.package.len().min(160) {
        let mut pkg = s.package.clone();
        if pkg[i] == 0xFF {
            // Setting a byte to the value it already holds changes nothing, so
            // the package opens - correctly. Without this guard the test
            // reported a false positive on Windows, where a differing manifest
            // put a 0xFF at offset 148. A test that fails for a reason unrelated
            // to the product is worse than no test.
            continue;
        }
        pkg[i] = 0xFF;
        let (opened, leaked) = attempt(&pkg, &s.key, &s.dir, MARKER);
        assert!(!opened, "byte {i} set to 0xFF still opened the package");
        assert!(!leaked);
    }

    // And an explicitly absurd chunk count, constructed rather than stumbled on.
    let mut pkg = s.package.clone();
    let n = pkg.len();
    for i in (0..n.saturating_sub(4)).rev() {
        // Overwrite the last plausible u32 before the chunk data with a huge
        // count; any position that parses must still refuse.
        pkg[i..i + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let (opened, leaked) = attempt(&pkg, &s.key, &s.dir, MARKER);
        assert!(!opened);
        assert!(!leaked);
        if i < n.saturating_sub(64) {
            break;
        }
    }
}

/// A12 — exhaustive sweep: every byte, not a sample.
///
/// `a10` samples across the artifact and `a11` covers only the first 160 bytes,
/// so between them they can miss an unauthenticated byte — and on Windows they
/// did, at offset 148. This walks the entire package and reports every offset
/// that can be changed without changing the outcome.
///
/// The failure message names the offsets, because "something is wrong" is not
/// actionable and "offsets 148, 149 are unauthenticated" is.
#[test]
fn a12_exhaustive_sweep_finds_no_unauthenticated_byte() {
    let s = specimen("a12");
    let mut unauthenticated = Vec::new();

    for i in 0..s.package.len() {
        let mut pkg = s.package.clone();
        pkg[i] = pkg[i].wrapping_add(1); // any change at all
        let (opened, leaked) = attempt(&pkg, &s.key, &s.dir, MARKER);
        if opened || leaked {
            unauthenticated.push(i);
        }
    }

    assert!(
        unauthenticated.is_empty(),
        "these offsets can be modified without preventing extraction: {:?} \
         (package is {} bytes)",
        unauthenticated,
        s.package.len()
    );
}
