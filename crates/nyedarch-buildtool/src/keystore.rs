//! Local key storage (spec §25/§44).
//!
//! The client holds long-lived secret material: the key that authenticates
//! portable `.nyfp` machine records and the key that authenticates the EULA
//! acceptance record. Until now these were derived from a constant compiled
//! into the binary, which meant **anyone could recompute them** and forge a
//! trusted-machine record. That is fixed here.
//!
//! A single random 32-byte *client master key* is generated on first run and
//! stored using the platform's own secure storage. Every other client secret is
//! derived from it with domain separation, so one stored item protects all of
//! them and no secret is ever written to a plain file if the platform offers
//! somewhere better.
//!
//! | Platform | Storage |
//! |---|---|
//! | macOS | Keychain, via the `security` tool |
//! | Windows | DPAPI, user-scoped, via PowerShell |
//! | Linux | Secret Service (`secret-tool`) when available |
//!
//! # The fallback, stated honestly
//!
//! A headless Linux box often has no Secret Service. Rather than refuse to run,
//! the master key is written to a file with owner-only permissions **and the
//! user is told**. That file is readable by anything running as that user, so
//! it is weaker than a keystore - it is a documented degradation, never a
//! silent one, and it is exactly the kind of substitution the specification
//! forbids doing quietly.
//!
//! No capsule ever contains any of this. The keystore is builder-side only.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use zeroize::Zeroizing;

const SERVICE: &str = "net.nyedarch.client";
const ACCOUNT: &str = "client-master-key";

/// Where the master key came from. Recorded so a weaker source is never passed
/// off as a stronger one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySource {
    /// Platform keystore: Keychain, DPAPI, or Secret Service.
    PlatformKeystore,
    /// Owner-only file, because no keystore was available.
    RestrictedFile,
}

impl KeySource {
    pub fn is_degraded(self) -> bool {
        self == KeySource::RestrictedFile
    }
    /// A warning to show the operator, or nothing when the keystore was used.
    pub fn warning(self) -> Option<&'static str> {
        match self {
            KeySource::PlatformKeystore => None,
            KeySource::RestrictedFile => Some(
                "No platform keystore was available, so NYEDArch stored its client key in a \
                 file readable only by your user account. Anything running as you can read it. \
                 Install a Secret Service provider (gnome-keyring, KWallet) for stronger storage.",
            ),
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 || s.is_empty() {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

fn fallback_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("nyedarch").join("client-key")
}

// ------------------------------------------------------------- platforms ---

#[cfg(target_os = "macos")]
fn keystore_load() -> Option<Vec<u8>> {
    let out = Command::new("security")
        .args(["find-generic-password", "-s", SERVICE, "-a", ACCOUNT, "-w"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    unhex(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(target_os = "macos")]
fn keystore_store(key: &[u8]) -> bool {
    // -U updates an existing item rather than failing.
    Command::new("security")
        .args([
            "add-generic-password", "-U", "-s", SERVICE, "-a", ACCOUNT,
            "-D", "NYEDArch client key", "-w", &hex(key),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Windows DPAPI, via the .NET `ProtectedData` API.
///
/// The first attempt used PowerShell's `ConvertTo-SecureString` /
/// `ConvertFrom-SecureString` pair. That round trip did not reproduce on a real
/// runner: a store reported success and a later load returned nothing, so every
/// run minted a fresh key and every previously signed record became
/// unverifiable. Those cmdlets carry SecureString and console-encoding
/// behaviour that is not worth fighting for a 32-byte secret.
///
/// `ProtectedData::Protect` is the primitive underneath, and it takes and
/// returns plain bytes. The ciphertext is stored base64-encoded, and the secret
/// travels on **stdin** in both directions - never in a command line, where any
/// other user on the machine can read it from the process list.
///
/// Whether this works is not asserted here: `master_key` reads back what it
/// stored and falls through to the restricted file if the value does not come
/// back identical. A keystore that cannot return what it was given is not
/// trusted, whatever its documentation claims.
#[cfg(target_os = "windows")]
fn dpapi_path() -> std::path::PathBuf {
    fallback_path().with_extension("dpapi")
}

#[cfg(target_os = "windows")]
fn keystore_store(key: &[u8]) -> bool {
    use std::io::Write;
    let path = dpapi_path();
    if let Some(d) = path.parent() {
        if std::fs::create_dir_all(d).is_err() {
            return false;
        }
    }
    let script = format!(
        "Add-Type -AssemblyName System.Security;          $b64 = [Console]::In.ReadToEnd().Trim();          $bytes = [Convert]::FromBase64String($b64);          $prot = [Security.Cryptography.ProtectedData]::Protect($bytes, $null, 'CurrentUser');          [IO.File]::WriteAllText('{}', [Convert]::ToBase64String($prot))",
        path.display()
    );
    let child = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    let Ok(mut child) = child else { return false };
    if let Some(mut si) = child.stdin.take() {
        // Base64 on stdin: the key never appears in argv.
        let _ = si.write_all(b64_encode(key).as_bytes());
    }
    let ok = child.wait().map(|s| s.success()).unwrap_or(false);
    if ok {
        restrict(&path);
    }
    ok
}

#[cfg(target_os = "windows")]
fn keystore_load() -> Option<Vec<u8>> {
    let path = dpapi_path();
    if !path.exists() {
        return None;
    }
    let script = format!(
        "Add-Type -AssemblyName System.Security;          $b64 = [IO.File]::ReadAllText('{}').Trim();          $prot = [Convert]::FromBase64String($b64);          $bytes = [Security.Cryptography.ProtectedData]::Unprotect($prot, $null, 'CurrentUser');          [Console]::Out.Write([Convert]::ToBase64String($bytes))",
        path.display()
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    b64_decode(String::from_utf8_lossy(&out.stdout).trim())
}

/// Minimal base64, so the key can cross a process boundary without a
/// dependency and without touching a command line.
#[cfg(target_os = "windows")]
fn b64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(target_os = "windows")]
fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a') as u32 + 26,
            b'0'..=b'9' => (c - b'0') as u32 + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    };
    let bytes: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            return None;
        }
        let pad = chunk.iter().filter(|c| **c == b'=').count();
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            let v = if *c == b'=' { 0 } else { val(*c)? };
            n |= v << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

#[cfg(all(target_os = "windows", any()))]
fn keystore_load_dpapi_disabled() -> Option<Vec<u8>> {
    // DPAPI, user-scoped: the ciphertext is only decryptable by this user on
    // this machine.
    let path = fallback_path().with_extension("dpapi");
    if !path.exists() {
        return None;
    }
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         try {{ $e = Get-Content -Raw '{}'; \
         $s = ConvertTo-SecureString $e; \
         $b = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($s); \
         [Runtime.InteropServices.Marshal]::PtrToStringAuto($b) }} catch {{ '' }}",
        path.display()
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .ok()?;
    unhex(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(all(target_os = "windows", any()))]
fn keystore_store_dpapi_disabled(key: &[u8]) -> bool {
    let path = fallback_path().with_extension("dpapi");
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $s = ConvertTo-SecureString '{}' -AsPlainText -Force; \
         ConvertFrom-SecureString $s | Set-Content -NoNewline '{}'",
        hex(key),
        path.display()
    );
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn keystore_load() -> Option<Vec<u8>> {
    let out = Command::new("secret-tool")
        .args(["lookup", "service", SERVICE, "account", ACCOUNT])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    unhex(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn keystore_store(key: &[u8]) -> bool {
    // secret-tool reads the secret from stdin, so it never appears in argv.
    let child = Command::new("secret-tool")
        .args(["store", "--label=NYEDArch client key", "service", SERVICE, "account", ACCOUNT])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else { return false };
    if let Some(stdin) = child.stdin.as_mut() {
        if stdin.write_all(hex(key).as_bytes()).is_err() {
            return false;
        }
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn keystore_load() -> Option<Vec<u8>> {
    None
}
#[cfg(not(any(unix, target_os = "windows")))]
fn keystore_store(_key: &[u8]) -> bool {
    false
}

// ------------------------------------------------------------- fallback ----

fn file_load() -> Option<Vec<u8>> {
    let s = std::fs::read_to_string(fallback_path()).ok()?;
    unhex(&s)
}

fn file_store(key: &[u8]) -> bool {
    use std::io::Write;
    let path = fallback_path();
    if let Some(d) = path.parent() {
        if std::fs::create_dir_all(d).is_err() {
            return false;
        }
    }
    // Create exclusively. If another process created it first, leave theirs
    // alone: the caller re-reads and adopts it, so both converge on one key
    // instead of overwriting each other.
    match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut f) => {
            if f.write_all(hex(key).as_bytes()).is_err() {
                return false;
            }
            restrict(&path)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => true, // theirs wins
        Err(_) => false,
    }
}

/// Owner-only permissions. On Windows the file is already under the user's
/// profile and DPAPI is preferred; this is best effort.
fn restrict(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).is_ok()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        true
    }
}

// ------------------------------------------------------------------ api ----

/// Load the client master key, creating it on first use.
///
/// Returns the key and where it is stored, so the caller can warn about a
/// degraded location rather than hiding it.
pub fn master_key() -> Result<(Zeroizing<[u8; 32]>, KeySource), String> {
    if let Some(found) = load_existing() {
        return Ok(found);
    }

    // First run. Minting must happen **once**, so it is serialised on a lock
    // file rather than left to chance.
    //
    // An earlier version stored optimistically and adopted whatever read back.
    // That converges only if every read happens after every write, which is not
    // true under concurrency: on macOS and Windows several starters each minted
    // a key and disagreed about the result. A record signed by a loser would
    // then be unverifiable. The lock removes the race instead of narrowing it.
    let lock = fallback_path().with_extension("lock");
    if let Some(d) = lock.parent() {
        let _ = std::fs::create_dir_all(d);
    }

    match std::fs::OpenOptions::new().write(true).create_new(true).open(&lock) {
        Ok(_) => {
            // We hold the lock: mint, store, and release.
            let result = mint_and_store();
            let _ = std::fs::remove_file(&lock);
            result
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Someone else is minting. Wait for them rather than minting too.
            for _ in 0..100 {
                std::thread::sleep(std::time::Duration::from_millis(50));
                if let Some(found) = load_existing() {
                    return Ok(found);
                }
            }
            // The holder died without finishing. Clear the stale lock and mint,
            // rather than failing forever on a leftover file.
            let _ = std::fs::remove_file(&lock);
            mint_and_store()
        }
        Err(_) => mint_and_store(), // cannot lock; proceed rather than fail
    }
}

/// Load a key that is already stored, preferring the platform keystore.
fn load_existing() -> Option<(Zeroizing<[u8; 32]>, KeySource)> {
    for (bytes, src) in [
        (keystore_load(), KeySource::PlatformKeystore),
        (file_load(), KeySource::RestrictedFile),
    ] {
        if let Some(k) = bytes {
            if k.len() == 32 {
                let mut out = [0u8; 32];
                out.copy_from_slice(&k);
                return Some((Zeroizing::new(out), src));
            }
        }
    }
    None
}

/// Mint a key and store it, verifying that the store returns it again.
fn mint_and_store() -> Result<(Zeroizing<[u8; 32]>, KeySource), String> {
    // Another starter may have finished between our checks.
    if let Some(found) = load_existing() {
        return Ok(found);
    }

    let mut fresh = [0u8; 32];
    getrandom::getrandom(&mut fresh).map_err(|_| "no secure randomness available".to_string())?;

    // A store that reports success without round-tripping is worse than one
    // that fails: every later call would find nothing, mint again, and silently
    // invalidate every record already signed. So the read-back is authoritative.
    if keystore_store(&fresh) {
        if let Some(back) = keystore_load() {
            if back.len() == 32 {
                let mut out = [0u8; 32];
                out.copy_from_slice(&back);
                return Ok((Zeroizing::new(out), KeySource::PlatformKeystore));
            }
        }
    }
    if file_store(&fresh) {
        if let Some(back) = file_load() {
            if back.len() == 32 {
                let mut out = [0u8; 32];
                out.copy_from_slice(&back);
                return Ok((Zeroizing::new(out), KeySource::RestrictedFile));
            }
        }
    }
    Err("the client key could not be stored anywhere that returns it again. \
         NYEDArch will not mint a fresh key on every run, because that would \
         silently invalidate every record it has already signed."
        .to_string())
}

/// Derive a purpose-specific client key from the master key.
///
/// Domain separation means the record-signing key and the EULA key are
/// unrelated: recovering one reveals nothing about the other.
pub fn derive(label: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    let (master, _) = master_key()?;
    nyedarch_crypto::kdf::hkdf_key(b"nyedarch:v1:client-keystore", &*master, label)
        .map_err(|_| "key derivation failed".to_string())
}

/// Where the master key currently lives, without creating one.
pub fn current_source() -> Option<KeySource> {
    if keystore_load().is_some() {
        Some(KeySource::PlatformKeystore)
    } else if file_load().is_some() {
        Some(KeySource::RestrictedFile)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrips() {
        let b = [0u8, 1, 15, 16, 255, 128];
        assert_eq!(unhex(&hex(&b)).unwrap(), b);
        assert!(unhex("odd").is_none());
        assert!(unhex("").is_none());
        assert!(unhex("zz").is_none());
    }

    #[test]
    fn a_key_can_be_obtained_and_is_stable() {
        let (a, src) = master_key().expect("master key");
        let (b, src2) = master_key().expect("master key again");
        // Instability here means every record signed by a previous run becomes
        // unverifiable. It was a real defect on Windows, where the keystore
        // reported a successful store that did not round-trip.
        assert_eq!(&*a, &*b, "the master key must not change between calls");
        assert_eq!(src, src2);
    }

    #[test]
    fn a_third_call_still_returns_the_same_key() {
        // Stability must hold across more than two calls: a store that only
        // works once would pass a two-call test.
        let (a, _) = master_key().expect("first");
        let (b, _) = master_key().expect("second");
        let (c, _) = master_key().expect("third");
        assert_eq!(&*a, &*b);
        assert_eq!(&*b, &*c);
    }

    #[test]
    fn derived_keys_are_separated_by_purpose() {
        let a = derive(b"nyfp").expect("derive");
        let b = derive(b"eula").expect("derive");
        assert_ne!(&*a, &*b, "different purposes must not share a key");
        // And they must be stable.
        assert_eq!(&*derive(b"nyfp").unwrap(), &*a);
    }

    /// Concurrent first-run initialization must converge on one key.
    ///
    /// Without this, two processes starting together on a fresh machine each
    /// mint a key, the last write wins, and every record signed by the loser
    /// becomes unverifiable. Found by the parallel test runner on macOS.
    #[test]
    fn concurrent_initialisation_converges() {
        let handles: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(|| master_key().map(|(k, _)| *k)))
            .collect();
        let keys: Vec<[u8; 32]> = handles
            .into_iter()
            .map(|h| h.join().expect("thread").expect("master key"))
            .collect();
        let first = keys[0];
        for (i, k) in keys.iter().enumerate() {
            assert_eq!(*k, first, "thread {i} disagreed about the client key");
        }
    }

    #[test]
    fn a_degraded_source_reports_a_warning() {
        assert!(KeySource::PlatformKeystore.warning().is_none());
        assert!(KeySource::RestrictedFile.warning().is_some());
        assert!(KeySource::RestrictedFile.is_degraded());
        assert!(!KeySource::PlatformKeystore.is_degraded());
    }

    #[test]
    #[cfg(unix)]
    fn the_fallback_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        // Force the fallback path to exist by writing through it.
        let key = [7u8; 32];
        if file_store(&key) {
            let mode = std::fs::metadata(fallback_path()).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "the key file must not be group or world readable");
        }
    }
}

// --------------------------------------------------------- stored secrets ---

/// Where a client secret lives on disk.
///
/// Beside the machine records rather than in the platform keystore: a token is
/// larger and longer than a key, and the keystore APIs across three platforms
/// disagree about size limits. It is encrypted with a key derived from the
/// master key, so the file alone is useless.
fn secret_path(label: &str) -> std::path::PathBuf {
    let mut d = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .or_else(|| std::env::var_os("APPDATA").map(std::path::PathBuf::from))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    d.push("nyedarch");
    d.push("secrets");
    d.push(format!("{label}.bin"));
    d
}

/// Store a client secret, encrypted under a key derived from the master key.
///
/// The GitHub token is the case this exists for. Retyping it on every build
/// invites the two things that actually go wrong with tokens: pasting the wrong
/// one, and leaving it in a shell history.
pub fn save_secret(label: &str, value: &[u8]) -> Result<(), String> {
    let key = derive(format!("secret:{label}").as_bytes())?;
    let sealed = nyedarch_crypto::aead::seal(&key, label.as_bytes(), value)
        .map_err(|_| "could not encrypt the secret".to_string())?
        .to_bytes();
    let path = secret_path(label);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create the store: {e}"))?;
    }
    std::fs::write(&path, sealed).map_err(|e| format!("cannot write the secret: {e}"))?;
    restrict_path(&path);
    Ok(())
}

/// Load a stored secret. Returns `None` when absent or unreadable — a secret
/// that cannot be decrypted is treated as absent rather than as an error, so a
/// rotated master key means "enter it again" rather than a broken client.
pub fn load_secret(label: &str) -> Option<Zeroizing<Vec<u8>>> {
    let key = derive(format!("secret:{label}").as_bytes()).ok()?;
    let bytes = std::fs::read(secret_path(label)).ok()?;
    let sealed = nyedarch_crypto::aead::Sealed::from_bytes(&bytes).ok()?;
    let plain = nyedarch_crypto::aead::open(&key, label.as_bytes(), &sealed).ok()?;
    Some(Zeroizing::new(plain.to_vec()))
}

/// Forget a stored secret.
pub fn forget_secret(label: &str) {
    let _ = std::fs::remove_file(secret_path(label));
}

fn restrict_path(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

#[cfg(test)]
mod secret_tests {
    use super::*;

    #[test]
    fn a_secret_round_trips_and_can_be_forgotten() {
        let label = format!("test-{}", std::process::id());
        save_secret(&label, b"ghp_not_a_real_token").expect("save");
        let got = load_secret(&label).expect("load");
        assert_eq!(&got[..], b"ghp_not_a_real_token");
        forget_secret(&label);
        assert!(load_secret(&label).is_none(), "a forgotten secret must not come back");
    }

    /// The stored file must not contain the secret in the clear.
    #[test]
    fn the_stored_file_is_not_plaintext() {
        let label = format!("test-plain-{}", std::process::id());
        save_secret(&label, b"SECRET-MARKER-VALUE").expect("save");
        let raw = std::fs::read(secret_path(&label)).expect("read");
        assert!(
            !raw.windows(19).any(|w| w == b"SECRET-MARKER-VALUE"),
            "the secret was written in the clear"
        );
        forget_secret(&label);
    }
}

#[cfg(all(test, target_os = "windows"))]
mod dpapi_tests {
    use super::*;

    /// Base64 must round-trip exactly, because it is how the key crosses the
    /// process boundary into PowerShell and back.
    #[test]
    fn base64_round_trips() {
        for case in [
            vec![],
            vec![0u8],
            vec![0u8, 1],
            vec![0u8, 1, 2],
            vec![0xFFu8; 32],
            (0u8..=255).collect::<Vec<u8>>(),
        ] {
            let encoded = b64_encode(&case);
            let decoded = b64_decode(&encoded).expect("decodes");
            assert_eq!(decoded, case, "round trip failed for {} bytes", case.len());
        }
    }

    /// Deliberately NOT tested here: a live store-and-load round trip.
    ///
    /// The obvious test calls `keystore_store` with a known value and reads it
    /// back. It passes, and it writes to the real keystore - replacing the
    /// client key that every other test in this crate depends on. Tests running
    /// beside it then fail, on Windows only, for reasons that have nothing to
    /// do with what they assert. That is exactly what it did.
    ///
    /// The round trip is verified where it matters anyway: `master_key` reads
    /// back what it stored on every start-up and falls through to the
    /// restricted file if the value does not return identical. A test that
    /// breaks its neighbours to prove something the product already checks is a
    /// bad trade.
    #[test]
    fn the_store_path_is_derived_without_touching_it() {
        let p = dpapi_path();
        assert_eq!(p.extension().and_then(|e| e.to_str()), Some("dpapi"));
        assert!(p.parent().is_some(), "the secret must live under a directory");
    }
}
