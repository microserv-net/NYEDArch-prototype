//! EULA gating (spec §13). NYEDArch cannot be used until the agreement is
//! explicitly accepted. The acceptance record is stored locally with an
//! integrity tag so it cannot be silently forged or back-dated by editing a
//! config file.

use std::io::Write;
use std::path::PathBuf;

use nyedarch_fingerprint::nyfp;

pub const EULA_VERSION: &str = "1.0";

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Acceptance {
    pub eula_version: String,
    pub accepted_unix: i64,
    pub app_version: String,
    /// Opaque local machine reference (digest, never raw identifiers).
    pub machine_ref: String,
}

pub fn record_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("nyedarch").join("eula-acceptance.bin")
}

/// The acceptance record is authenticated with the same local key class used
/// for `.nyfp` records, so tampering is detectable (spec §13).
/// Key that authenticates the acceptance record, derived from the client master
/// key in the platform keystore and domain-separated from the record key.
fn key() -> [u8; 32] {
    match crate::keystore::derive(b"nyedarch:v1:eula") {
        Ok(k) => *k,
        Err(_) => {
            let fp = nyedarch_fingerprint::capture();
            *nyedarch_crypto::kdf::hkdf_key(
                b"nyedarch:v1:eula:degraded",
                &fp.id,
                b"nyedarch:v1:eula",
            )
            .expect("hkdf")
        }
    }
}

/// Returns true when a valid, current acceptance record exists.
pub fn already_accepted() -> bool {
    let p = record_path();
    let Ok(bytes) = std::fs::read(&p) else { return false };
    // Reuse the authenticated-record container.
    match nyfp::open_bytes(&key(), &bytes) {
        Ok(body) => match bincode::deserialize::<Acceptance>(&body) {
            Ok(a) => a.eula_version == EULA_VERSION,
            Err(_) => false,
        },
        Err(_) => false, // tampered or unreadable => treat as not accepted
    }
}

pub fn summary() -> &'static str {
    "\
  NYEDArch collects system information (firmware/platform identifiers, OS install
  identity, CPU/host details) to build a machine fingerprint. Some of this is
  sensitive. Only salted digests are stored; raw identifiers never are.

  If you use the remote build feature, NYEDArch operates a repository in YOUR
  GitHub account. Private is recommended and default; a PUBLIC repository
  exposes your generated runtime source and build logs to anyone.

  Private key material is stored in your platform's secure storage. LOSS OF KEY
  MATERIAL MAY MAKE PREVIOUSLY GENERATED CAPSULES PERMANENTLY UNRECOVERABLE.
  There is no recovery mechanism and no backdoor.

  The location protection requests your location with permission and rejects readings
  less accurate than your tolerance. The time protection uses the LOCAL SYSTEM CLOCK,
  which whoever controls the machine can change; it is a recurring policy
  control, not a tamper-proof expiry.

  One-shot destruction is best effort. SOFTWARE CANNOT GUARANTEE SECURE
  DELETION on SSDs or copy-on-write filesystems.

  NYEDArch raises the cost of unauthorized access. It is NOT unbreakable and does
  NOT provide absolute security. You are responsible for lawful use, for your
  passphrase, and for keeping independent backups.

  Full text: docs/EULA.md"
}

/// The disclosures, as a list of paragraphs, for an interface to render.
pub fn disclosures() -> Vec<&'static str> {
    summary().split("\n\n").map(|s| s.trim()).filter(|s| !s.is_empty()).collect()
}

/// Record acceptance. Shared by every interface so there is one record and one
/// authentication scheme, not a per-client variation.
pub fn record_acceptance() -> Result<(), String> {
    let fp = nyedarch_fingerprint::capture();
    let record = Acceptance {
        eula_version: EULA_VERSION.to_string(),
        accepted_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        machine_ref: fp.id_hex()[..16].to_string(),
    };
    let body = bincode::serialize(&record).map_err(|_| "serialize acceptance".to_string())?;
    let sealed = nyfp::seal_bytes(&key(), &body).map_err(|_| "authenticate acceptance".to_string())?;
    let p = record_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&p, sealed).map_err(|e| format!("could not persist acceptance: {e}"))
}

/// Present the EULA and require explicit acceptance. Exits if declined.
pub fn require_acceptance() {
    if already_accepted() {
        return;
    }
    println!();
    println!("  -- NYEDArch END USER LICENCE AGREEMENT (v{EULA_VERSION}) --");
    println!();
    println!("{}", summary());
    println!();
    print!("  Type 'accept' to agree, anything else to exit: ");
    let _ = std::io::stdout().flush();

    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        eprintln!("\n  Could not read a response. Not accepted.");
        std::process::exit(1);
    }
    if line.trim().to_ascii_lowercase() != "accept" {
        println!("\n  Not accepted. NYEDArch will not run.");
        std::process::exit(1);
    }

    let fp = nyedarch_fingerprint::capture();
    let record = Acceptance {
        eula_version: EULA_VERSION.to_string(),
        accepted_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        machine_ref: fp.id_hex()[..16].to_string(),
    };
    let body = bincode::serialize(&record).expect("serialize acceptance");
    let sealed = nyfp::seal_bytes(&key(), &body).expect("authenticate acceptance");
    let p = record_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::write(&p, sealed).is_err() {
        eprintln!("  Warning: acceptance could not be persisted; you will be asked again.");
    }
    println!("  Accepted and recorded.\n");
}
