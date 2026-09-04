//! Encrypted source transport for the remote build.
//!
//! # The problem
//!
//! The remote build needs the generated capsule source on GitHub, but that
//! source is not neutral: it contains the sealed package, the per-build
//! bootstrap key, and the runtime commitment. Pushing it as plaintext means a
//! public repository hands all of that to anyone, and even a private repository
//! exposes it to every collaborator and to anyone who later gains read access.
//!
//! # The approach
//!
//! Everything except a small, secret-free unlocker is packed into one archive,
//! encrypted under a fresh random key, and pushed as ciphertext. The key is
//! stored as a **GitHub Actions repository secret**, sealed to the repository's
//! own public key, so only the Actions runner can read it. The workflow builds
//! the unlocker, decrypts the archive in the runner's workspace, and then
//! compiles the capsule.
//!
//! The unlocker is deliberately committed in the clear: it contains no secret,
//! only the ability to open an archive given a key it does not have.
//!
//! # What this does and does not protect
//!
//! It removes the capsule source from the repository's contents, its history,
//! and its clones. Someone with read access sees a blob and a bootstrap crate.
//!
//! It does **not** defend against the repository owner, because the trust model
//! here is explicit: whoever controls the repository controls its workflows, and
//! anything a workflow can decrypt, a modified workflow can print. This raises
//! the bar from "public by default" to "requires control of the build
//! environment", which is the honest description.

use std::io::Write;
use std::path::Path;

/// Archive magic. Versioned so a future format can be rejected rather than
/// misread.
const MAGIC: &[u8; 10] = b"NYARCHIVE2";
/// Domain separation for the archive's authenticated encryption.
pub const ARCHIVE_AAD: &[u8] = b"nyedarch:v1:source-archive";

/// Pack a directory into a flat archive.
///
/// Layout: `MAGIC` then, per entry, `u32` path length, the path in UTF-8 with
/// `/` separators, `u8` flags, `u64` data length, then the bytes. Bit 0 of the
/// flags marks an executable file.
///
/// The flag exists because the first version dropped the executable bit, so an
/// unpacked tree that contained a script could not run it. Simple on purpose:
/// the unlocker that reads this must be small enough to audit at a glance.
pub fn pack(root: &Path, skip: &[&str]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    let mut files = Vec::new();
    collect(root, root, skip, &mut files)?;
    // Deterministic ordering keeps the commitment stable across runs.
    files.sort();
    for rel in files {
        let full = root.join(&rel);
        let data = std::fs::read(&full)?;
        let name = rel.replace('\\', "/");
        let mut flags = 0u8;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(md) = std::fs::metadata(&full) {
                if md.permissions().mode() & 0o111 != 0 {
                    flags |= 1;
                }
            }
        }
        out.write_all(&(name.len() as u32).to_le_bytes())?;
        out.write_all(name.as_bytes())?;
        out.write_all(&[flags])?;
        out.write_all(&(data.len() as u64).to_le_bytes())?;
        out.write_all(&data)?;
    }
    Ok(out)
}

fn collect(root: &Path, dir: &Path, skip: &[&str], out: &mut Vec<String>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "target" || name == ".git" || skip.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, skip, out)?;
        } else {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            out.push(rel);
        }
    }
    Ok(())
}

/// The unlocker crate, written into the build repository in the clear.
///
/// It holds no secret. It reads the key from the environment, which on the
/// runner comes from a repository secret, opens the archive, and writes the
/// files out. Anyone reading it learns the format and nothing else.
pub fn unlocker_sources() -> Vec<(&'static str, String)> {
    let cargo = r#"# Its own workspace on purpose: this crate is unpacked next to the encrypted
# capsule source, and without this cargo would try to treat it as a member of
# whatever workspace happens to sit above it and refuse to build.
[workspace]

[package]
name = "nyedarch-unlock"
version = "0.0.1"
edition = "2021"

[[bin]]
name = "nyedarch-unlock"
path = "src/main.rs"

[dependencies]
chacha20poly1305 = "0.10"

# Hardened like everything else NYEDArch ships. This crate holds no secret, but
# it is compiled in a repository that may be public, and a stripped binary keeps
# the build log and the artifact free of symbol noise.
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
debug = false
incremental = false
"#;

    let main = r#"//! Opens the encrypted capsule source inside the build runner.
//!
//! This program contains no secret. The key arrives in NYEDARCH_SOURCE_KEY,
//! which the workflow populates from a repository secret, so it exists only in
//! the runner's memory for the length of the build.
//!
//! Failure is always fatal: a build must never continue with a partially
//! written or unauthenticated source tree.

use std::io::Read;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};

const MAGIC: &[u8; 10] = b"NYARCHIVE2";
const AAD: &[u8] = b"nyedarch:v1:source-archive";

fn die(msg: &str) -> ! {
    eprintln!("nyedarch-unlock: {msg}");
    std::process::exit(1);
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 { return None; }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i+2], 16).ok()).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        die("usage: nyedarch-unlock <archive> <output-directory>");
    }
    let key_hex = std::env::var("NYEDARCH_SOURCE_KEY")
        .unwrap_or_else(|_| die("NYEDARCH_SOURCE_KEY is not set; the repository secret is missing"));
    let key = unhex(&key_hex).unwrap_or_else(|| die("NYEDARCH_SOURCE_KEY is not valid hex"));
    if key.len() != 32 {
        die("NYEDARCH_SOURCE_KEY must be 32 bytes");
    }

    let mut blob = Vec::new();
    std::fs::File::open(&args[1])
        .unwrap_or_else(|e| die(&format!("cannot open the archive: {e}")))
        .read_to_end(&mut blob)
        .unwrap_or_else(|e| die(&format!("cannot read the archive: {e}")));

    if blob.len() < 24 {
        die("the archive is truncated");
    }
    let (nonce, ct) = blob.split_at(24);

    let cipher = XChaCha20Poly1305::new_from_slice(&key)
        .unwrap_or_else(|_| die("invalid key"));
    // Authenticated: a modified archive fails here rather than producing a
    // subtly wrong source tree.
    let plain = cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad: AAD })
        .unwrap_or_else(|_| die("the archive failed authentication: wrong key or modified content"));

    if plain.len() < MAGIC.len() || &plain[..MAGIC.len()] != MAGIC {
        die("unrecognised archive format");
    }

    let out_root = PathBuf::from(&args[2]);
    let mut i = MAGIC.len();
    let mut count = 0usize;
    while i < plain.len() {
        if i + 4 > plain.len() { die("truncated entry header"); }
        let nlen = u32::from_le_bytes(plain[i..i+4].try_into().unwrap()) as usize;
        i += 4;
        if i + nlen > plain.len() { die("truncated entry name"); }
        let name = String::from_utf8_lossy(&plain[i..i+nlen]).to_string();
        i += nlen;
        if i >= plain.len() { die("truncated entry flags"); }
        let flags = plain[i];
        i += 1;
        if i + 8 > plain.len() { die("truncated entry length"); }
        let dlen = u64::from_le_bytes(plain[i..i+8].try_into().unwrap()) as usize;
        i += 8;
        if i + dlen > plain.len() { die("truncated entry data"); }
        let data = &plain[i..i+dlen];
        i += dlen;

        // Refuse anything that would escape the output directory.
        if name.starts_with('/') || name.split('/').any(|c| c == ".." || c.is_empty()) {
            die(&format!("refusing unsafe path in archive: {name}"));
        }
        let dest = out_root.join(&name);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|e| die(&format!("cannot create {}: {e}", parent.display())));
        }
        std::fs::write(&dest, data)
            .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", dest.display())));
        // Restore the executable bit. Without this an unpacked script cannot
        // run, which is how this was found.
        #[cfg(unix)]
        if flags & 1 != 0 {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
        }
        #[cfg(not(unix))]
        let _ = flags;
        count += 1;
    }
    println!("nyedarch-unlock: restored {count} file(s) into {}", out_root.display());
    let _ = Path::new(&args[1]);
}
"#;

    vec![
        ("unlock/Cargo.toml", cargo.to_string()),
        ("unlock/src/main.rs", main.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("nyedarch-srcpack-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Unpack mirrors the unlocker, so the format is exercised both ways here.
    fn unpack(blob: &[u8]) -> Vec<(String, Vec<u8>)> {
        assert_eq!(&blob[..MAGIC.len()], MAGIC);
        let mut out = Vec::new();
        let mut i = MAGIC.len();
        while i < blob.len() {
            let nlen = u32::from_le_bytes(blob[i..i + 4].try_into().unwrap()) as usize;
            i += 4;
            let name = String::from_utf8(blob[i..i + nlen].to_vec()).unwrap();
            i += nlen;
            let _flags = blob[i];
            i += 1;
            let dlen = u64::from_le_bytes(blob[i..i + 8].try_into().unwrap()) as usize;
            i += 8;
            out.push((name, blob[i..i + dlen].to_vec()));
            i += dlen;
        }
        out
    }

    #[test]
    fn packs_a_tree_and_round_trips() {
        let d = tmp("pack");
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::write(d.join("Cargo.toml"), b"[package]").unwrap();
        std::fs::write(d.join("src/main.rs"), b"fn main(){}").unwrap();
        std::fs::write(d.join("capsule.nyeda"), vec![7u8; 300]).unwrap();

        let blob = pack(&d, &[]).unwrap();
        let entries = unpack(&blob);
        let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"Cargo.toml"));
        assert!(names.contains(&"src/main.rs"));
        assert!(names.contains(&"capsule.nyeda"));
        let payload = entries.iter().find(|(n, _)| n == "capsule.nyeda").unwrap();
        assert_eq!(payload.1.len(), 300);
    }

    #[test]
    fn packing_is_deterministic() {
        let d = tmp("det");
        std::fs::create_dir_all(d.join("a")).unwrap();
        std::fs::write(d.join("a/one"), b"1").unwrap();
        std::fs::write(d.join("a/two"), b"2").unwrap();
        assert_eq!(pack(&d, &[]).unwrap(), pack(&d, &[]).unwrap());
    }

    #[test]
    fn skipped_directories_are_excluded() {
        let d = tmp("skip");
        std::fs::create_dir_all(d.join("unlock/src")).unwrap();
        std::fs::write(d.join("unlock/src/main.rs"), b"x").unwrap();
        std::fs::write(d.join("keep.txt"), b"y").unwrap();
        let names: Vec<String> = unpack(&pack(&d, &["unlock"]).unwrap())
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert!(names.contains(&"keep.txt".to_string()));
        assert!(
            !names.iter().any(|n| n.starts_with("unlock/")),
            "the unlocker must not be packed inside the archive it opens"
        );
    }

    #[test]
    #[cfg(unix)]
    fn the_executable_bit_survives_a_round_trip() {
        use std::os::unix::fs::PermissionsExt;
        let d = tmp("exec");
        std::fs::write(d.join("script.sh"), b"#!/bin/sh\necho hi\n").unwrap();
        std::fs::write(d.join("plain.txt"), b"data").unwrap();
        std::fs::set_permissions(d.join("script.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();

        let blob = pack(&d, &[]).unwrap();
        // Locate the flag byte for each entry by walking the format.
        let mut i = MAGIC.len();
        let mut seen = std::collections::HashMap::new();
        while i < blob.len() {
            let nlen = u32::from_le_bytes(blob[i..i + 4].try_into().unwrap()) as usize;
            i += 4;
            let name = String::from_utf8(blob[i..i + nlen].to_vec()).unwrap();
            i += nlen;
            let flags = blob[i];
            i += 1;
            let dlen = u64::from_le_bytes(blob[i..i + 8].try_into().unwrap()) as usize;
            i += 8 + dlen;
            seen.insert(name, flags);
        }
        assert_eq!(seen["script.sh"] & 1, 1, "an executable file must be marked");
        assert_eq!(seen["plain.txt"] & 1, 0, "a plain file must not be marked");
    }

    #[test]
    fn the_unlocker_carries_no_secret() {
        let src = unlocker_sources();
        let main = &src.iter().find(|(n, _)| n.ends_with("main.rs")).unwrap().1;
        // It must read the key from the environment, never embed one.
        assert!(main.contains("NYEDARCH_SOURCE_KEY"));
        assert!(!main.contains("const KEY"));
        // And it must refuse path traversal.
        assert!(main.contains("refusing unsafe path"));
        assert!(main.contains("failed authentication"));
    }
}
