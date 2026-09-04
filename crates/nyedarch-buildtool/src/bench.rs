//! Measured performance (spec: benchmark, never claim "fast" without numbers).
//! Run: `nyedarch-buildtool bench`

use std::time::Instant;

use nyedarch_core::Policy;
use nyedarch_crypto::{compose, passphrase_key, version, Argon2Params, Binding, Contributions};
use nyedarch_package::{collect, format::*, restore, seal_package};

fn mib(bytes: u64, secs: f64) -> f64 {
    (bytes as f64 / (1024.0 * 1024.0)) / secs
}

pub fn run() {
    println!("NYEDArch benchmarks - single run, this machine, debug-or-release as built.\n");

    // 1) Fingerprint capture.
    let t = Instant::now();
    let fp = nyedarch_fingerprint::capture();
    let d = t.elapsed();
    println!("  fingerprint capture      {:>9.2?}   ({} signals, {:?})", d, fp.signals.len(), fp.best_strength);

    // 2) Argon2id at interactive parameters — the deliberate cost.
    for (label, p) in [
        ("argon2id 64MiB t=2", Argon2Params { m_cost: 64 * 1024, t_cost: 2, p_cost: 1 }),
        ("argon2id 256MiB t=3", Argon2Params { m_cost: 256 * 1024, t_cost: 3, p_cost: 1 }),
    ] {
        let t = Instant::now();
        let _ = passphrase_key(b"benchmark-passphrase", &[7u8; 32], p);
        println!("  {label:<24} {:>9.2?}", t.elapsed());
    }

    // 3) Seal + restore across payload sizes.
    let salt = [0x11u8; 32];
    let binding = Binding { package_id: [1u8; 16], runtime_binding: [2u8; 32], crypto_version: version::CRYPTO_VERSION };
    let policy = Policy { location: false, time: false, one_shot: false };
    let flags = policy.to_flags();
    let fast = Argon2Params { m_cost: 8, t_cost: 1, p_cost: 1 };
    let contrib = Contributions {
        machine_secret: Some([3u8; 32]),
        passphrase_key: Some(passphrase_key(b"p", &salt, fast).unwrap()),
        location_cell: None,
        time_window: None,
    };
    let key = compose::derive_payload_key(&salt, &binding, flags, &contrib).unwrap();

    println!();
    println!("  {:<8} {:<8} {:>9} {:>11} {:>9} {:>13} {:>7}",
        "codec", "payload", "seal", "seal MiB/s", "restore", "restore MiB/s", "ratio");

    for (codec, cname) in [(COMPRESSION_DEFLATE, "deflate"), (COMPRESSION_ZSTD, "zstd")] {
    for size_mb in [8u64, 32] {
        let dir = std::env::temp_dir().join(format!("nyedarch-bench-{}-{}", std::process::id(), size_mb));
        let src = dir.join("src");
        let _ = std::fs::create_dir_all(&src);
        // Semi-compressible data: neither all-zeros nor pure random.
        let mut blob = Vec::with_capacity((size_mb * 1024 * 1024) as usize);
        let mut x: u32 = 0x1234_5678;
        while blob.len() < (size_mb * 1024 * 1024) as usize {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            let b = (x >> 24) as u8;
            for _ in 0..16 {
                blob.push(b);
            }
            blob.extend_from_slice(b"NYEDArch benchmark filler sequence ");
        }
        blob.truncate((size_mb * 1024 * 1024) as usize);
        std::fs::write(src.join("blob.bin"), &blob).unwrap();

        let (manifest, files) = collect(&[src.clone()]).unwrap();
        let header = Header {
            crypto_version: version::CRYPTO_VERSION,
            package_id: binding.package_id,
            runtime_binding: binding.runtime_binding,
            policy,
            argon: fast.into(),
            package_salt: salt,
            compression: codec,
            chunk_size: 1024 * 1024,
        };

        let t = Instant::now();
        let pkg = seal_package(&header, b"", &key, &manifest, &files).unwrap();
        let seal_t = t.elapsed().as_secs_f64();

        let parsed = parse(&pkg).unwrap();
        let out = dir.join("out");
        let t = Instant::now();
        let _ = restore(&parsed, &key, &out).unwrap();
        let rest_t = t.elapsed().as_secs_f64();

        let raw = size_mb * 1024 * 1024;
        println!(
            "  {:<8} {:<8} {:>8.2}s {:>11.1} {:>8.2}s {:>13.1} {:>6.1}x",
            cname,
            format!("{size_mb}MiB"),
            seal_t,
            mib(raw, seal_t),
            rest_t,
            mib(raw, rest_t),
            raw as f64 / pkg.len() as f64
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
    }

    println!();
    println!("  Notes:");
    println!("   - Argon2id dominates capsule startup by design; it is the memory-hard");
    println!("     passphrase cost, not an inefficiency.");
    println!("   - Seal/restore stream in bounded chunks; peak plaintext memory is one");
    println!("     chunk, independent of payload size.");
    println!("   - Compression is DEFLATE in this build; zstd is the production target");
    println!("     and is expected to improve both ratio and throughput.");
}
