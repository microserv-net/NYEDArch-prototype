#![cfg(feature = "builder")]

//! Full package round-trip: files on disk -> sealed package -> restored files.
//! Also asserts the payload is inaccessible with a wrong factor (wrong key).

use std::fs;
use std::path::PathBuf;

/// Unique per call.
///
/// A timestamp is not enough. Windows' clock granularity is coarse, so two
/// calls in a tight loop can return the same value and share a directory - and
/// then one iteration reads another's output. An atomic counter cannot collide.
static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn tmpdir() -> PathBuf {
    let mut d = std::env::temp_dir();
    let n = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let uniq = format!("nyeda-rt-{}-{}-{}", std::process::id(), n, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    d.push(uniq);
    std::fs::create_dir_all(&d).unwrap();
    d
}

use nyedarch_core::Policy;
use nyedarch_crypto::{compose, passphrase_key, version, Argon2Params, Binding, Contributions};
use nyedarch_package::{collect, format::*, restore, seal_package};


fn tiny_params() -> Argon2Params {
    Argon2Params { m_cost: 8, t_cost: 1, p_cost: 1 }
}

fn make_key(salt: &[u8], b: &Binding, policy: Policy, machine: [u8; 32], pass: &[u8]) -> [u8; 32] {
    let flags = policy.to_flags();
    let c = Contributions {
        machine_secret: Some(machine),
        passphrase_key: Some(passphrase_key(pass, salt, tiny_params()).unwrap()),
        location_cell: None,
        time_window: None,
    };
    *compose::derive_payload_key(salt, b, flags, &c).unwrap()
}

#[test]
fn seal_then_restore_reproduces_tree() {
    let dir = tmpdir();
    let src = dir.join("src");
    fs::create_dir_all(src.join("nested")).unwrap();
    fs::write(src.join("a.txt"), b"hello world").unwrap();
    fs::write(src.join("nested/b.bin"), vec![7u8; 5000]).unwrap();
    fs::write(src.join("empty.txt"), b"").unwrap();

    let (manifest, files) = collect(&[src.clone()]).unwrap();

    let salt = [0x21u8; 32];
    let binding = Binding { package_id: *b"pkg-roundtrip-01", runtime_binding: [0x33; 32], crypto_version: version::CRYPTO_VERSION };
    let policy = Policy { location: false, time: false, one_shot: false };
    let machine = [0xAAu8; 32];
    let key = make_key(&salt, &binding, policy, machine, b"passw0rd");

    let header = Header {
        crypto_version: version::CRYPTO_VERSION,
        package_id: binding.package_id,
        runtime_binding: binding.runtime_binding,
        policy,
        argon: tiny_params().into(),
        package_salt: salt,
        compression: COMPRESSION_DEFLATE,
        chunk_size: 1024, // small chunk to exercise multi-chunk + file-straddling
    };
    // Empty sealed policy for this format test (bootstrap map exercised in runtime tests).
    let pkg = seal_package(&header, b"", &key, &manifest, &files).unwrap();

    let parsed = parse(&pkg).unwrap();
    let out = dir.join("out");
    let report = restore(&parsed, &key, &out).unwrap();
    assert_eq!(report.files, 3);

    assert_eq!(fs::read(out.join("src/a.txt")).unwrap(), b"hello world");
    assert_eq!(fs::read(out.join("src/nested/b.bin")).unwrap(), vec![7u8; 5000]);
    assert_eq!(fs::read(out.join("src/empty.txt")).unwrap(), b"");

    // Wrong passphrase -> wrong key -> restore fails (no plaintext leaks).
    let wrong = make_key(&salt, &binding, policy, machine, b"WRONG");
    let out2 = dir.join("out2");
    assert!(restore(&parsed, &wrong, &out2).is_err());
}


/// Both codecs must round-trip, and a package sealed with one codec must be
/// readable regardless of which is currently the default (spec §59: the format
/// is versioned and self-describing).
#[test]
fn both_codecs_roundtrip_and_stay_readable() {
    for codec in [COMPRESSION_DEFLATE, COMPRESSION_ZSTD] {
        let dir = tmpdir();
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        let payload = b"NYEDArch codec test payload, repeated. ".repeat(500);
        fs::write(src.join("a.bin"), &payload).unwrap();

        let (manifest, files) = collect(&[src.clone()]).unwrap();
        let salt = [0x44u8; 32];
        let binding = Binding {
            package_id: *b"pkg-codec-test01",
            runtime_binding: [0x55; 32],
            crypto_version: version::CRYPTO_VERSION,
        };
        let policy = Policy { location: false, time: false, one_shot: false };
        let key = make_key(&salt, &binding, policy, [0xAAu8; 32], b"pw");

        let header = Header {
            crypto_version: version::CRYPTO_VERSION,
            package_id: binding.package_id,
            runtime_binding: binding.runtime_binding,
            policy,
            argon: tiny_params().into(),
            package_salt: salt,
            compression: codec,
            chunk_size: 2048,
        };
        let pkg = seal_package(&header, b"", &key, &manifest, &files).unwrap();
        let parsed = parse(&pkg).unwrap();
        assert_eq!(parsed.header.compression, codec, "codec must be recorded in the header");

        let out = dir.join("out");
        restore(&parsed, &key, &out).unwrap();
        assert_eq!(fs::read(out.join("src/a.bin")).unwrap(), payload);
    }
}

/// An unknown codec must fail closed rather than guessing.
#[test]
fn unknown_codec_fails_closed() {
    assert!(nyedarch_package::pipeline::decompress_block_with(99, b"anything").is_err());
}

/// Every compression mode must produce a capsule that opens identically.
/// The level changes size and build time, never correctness.
#[test]
fn all_compression_modes_round_trip() {
    use nyedarch_package::pipeline::CompressionMode;

    let payload = b"NYEDArch compression mode test payload. ".repeat(400);
    let mut sizes = Vec::new();

    for mode in [
        CompressionMode::Fast,
        CompressionMode::Balanced,
        CompressionMode::Maximum,
        CompressionMode::Automatic,
    ] {
        let dir = tmpdir();
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.bin"), &payload).unwrap();

        let (manifest, files) = collect(&[src.clone()]).unwrap();
        let salt = [0x66u8; 32];
        let binding = Binding {
            package_id: *b"pkg-compression1",
            runtime_binding: [0x77; 32],
            crypto_version: version::CRYPTO_VERSION,
        };
        let policy = Policy { location: false, time: false, one_shot: false };
        let key = make_key(&salt, &binding, policy, [0xAAu8; 32], b"pw");
        let header = Header {
            crypto_version: version::CRYPTO_VERSION,
            package_id: binding.package_id,
            runtime_binding: binding.runtime_binding,
            policy,
            argon: tiny_params().into(),
            package_salt: salt,
            compression: COMPRESSION_ZSTD,
            chunk_size: 4096,
        };
        let level = mode.level(payload.len() as u64);
        let pkg = nyedarch_package::build::seal_package_cancellable(
            &header, b"", &key, &manifest, &files, level, &|| false,
        )
        .unwrap();

        let parsed = parse(&pkg).unwrap();
        let out = dir.join("out");
        restore(&parsed, &key, &out).unwrap();
        assert_eq!(fs::read(out.join("src/a.bin")).unwrap(), payload, "{}", mode.label());
        sizes.push((mode.label(), pkg.len()));
    }

    // Maximum must not be larger than Fast on compressible input; if it were,
    // the level was not reaching the compressor.
    let fast = sizes.iter().find(|(m, _)| *m == "fast").unwrap().1;
    let max = sizes.iter().find(|(m, _)| *m == "maximum").unwrap().1;
    assert!(max <= fast, "maximum ({max}) should not exceed fast ({fast})");
}

/// Cancelling mid-seal must abandon the work rather than return a partial
/// package that could later be mistaken for a complete one.
#[test]
fn cancellation_abandons_the_seal() {
    let dir = tmpdir();
    let src = dir.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("big.bin"), vec![3u8; 200_000]).unwrap();

    let (manifest, files) = collect(&[src.clone()]).unwrap();
    let salt = [0x88u8; 32];
    let binding = Binding {
        package_id: *b"pkg-cancellati01",
        runtime_binding: [0x99; 32],
        crypto_version: version::CRYPTO_VERSION,
    };
    let policy = Policy { location: false, time: false, one_shot: false };
    let key = make_key(&salt, &binding, policy, [0xAAu8; 32], b"pw");
    let header = Header {
        crypto_version: version::CRYPTO_VERSION,
        package_id: binding.package_id,
        runtime_binding: binding.runtime_binding,
        policy,
        argon: tiny_params().into(),
        package_salt: salt,
        compression: COMPRESSION_ZSTD,
        chunk_size: 4096,
    };
    let result = nyedarch_package::build::seal_package_cancellable(
        &header, b"", &key, &manifest, &files, 3, &|| true,
    );
    assert!(result.is_err(), "a cancelled seal must not return a package");
}
