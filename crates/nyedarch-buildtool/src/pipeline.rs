//! The NYEDArch builder pipeline.
//!
//! This is the single implementation of "seal a source into a capsule project".
//! The command-line client and the desktop client both call it, so there is no
//! second code path that could drift from the real one, and no interface can
//! show progress for work that did not happen.
//!
//! It performs no I/O of its own beyond reading the source and writing the
//! generated project, and it reports progress through a callback so a caller
//! can drive a progress bar from genuine stages.

use std::path::{Path, PathBuf};

use nyedarch_core::{Policy, PolicyEntry, PolicyRecord};
use nyedarch_crypto::timewin::DailySchedule;
use nyedarch_crypto::{compose, passphrase_key, version, Argon2Params, Binding, Contributions};
use nyedarch_package::format::*;
use nyedarch_package::collect;

use crate::generator;

/// A stage of real work, reported as it happens.
#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    CapturingFingerprint,
    LoadingTrustedMachines,
    AcquiringLocation,
    DerivingKeys,
    CollectingFiles,
    SealingPayload,
    GeneratingProject,
    Done,
}

impl Stage {
    /// Fraction complete, for progress reporting. These are the real stage
    /// boundaries, not a timer.
    pub fn progress(&self) -> f32 {
        match self {
            Stage::CapturingFingerprint => 0.05,
            Stage::LoadingTrustedMachines => 0.12,
            Stage::AcquiringLocation => 0.20,
            Stage::DerivingKeys => 0.35,
            Stage::CollectingFiles => 0.45,
            Stage::SealingPayload => 0.70,
            Stage::GeneratingProject => 0.90,
            Stage::Done => 1.0,
        }
    }
    pub fn message(&self) -> &'static str {
        match self {
            Stage::CapturingFingerprint => "Reading this machine's identity, so the capsule can recognise it later.",
            Stage::LoadingTrustedMachines => "Checking the trusted machine records. An edited record is refused.",
            Stage::AcquiringLocation => "Requesting location from the operating system.",
            Stage::DerivingKeys => "Deriving the payload key. Argon2id is deliberately slow - this is the step that makes guessing expensive.",
            Stage::CollectingFiles => "Recording what is being protected: paths, permissions and symlinks.",
            Stage::SealingPayload => "Compressing and sealing chunks.",
            Stage::GeneratingProject => "Generating capsule source and vendoring runtime crates.",
            Stage::Done => "Capsule project ready.",
        }
    }
}

/// Everything needed to seal a capsule.
pub struct SealRequest {
    pub source: PathBuf,
    /// Where the generated capsule project is written.
    pub project_dir: PathBuf,
    pub passphrase: String,
    /// Enabled only when `Some`; the value is the acceptance radius in metres.
    pub location_tolerance_m: Option<u32>,
    pub schedule: Option<DailySchedule>,
    pub one_shot: bool,
    /// Additional trusted machines, as authenticated `.nyfp` files.
    pub trust_files: Vec<PathBuf>,
    pub argon: Argon2Params,
    /// How hard to compress (spec §16). Build-time only; it never changes the
    /// format or the cryptography.
    pub compression: nyedarch_package::pipeline::CompressionMode,
    /// Consulted between stages and between chunks. A cancelled build leaves no
    /// partial capsule project behind.
    pub cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Creator mode (spec §50): extra diagnostics for the operator.
    ///
    /// It is **diagnostic only**. It grants no additional authority, skips no
    /// check, and weakens no capsule it produces; the only difference is how
    /// much the client tells you about its own work.
    pub creator_mode: bool,
    /// Hardware-backed machine protection policy (context 5 §14).
    pub hardware: nyedarch_platform::hardware::HardwarePolicy,
}

/// What the caller gets back. Every field describes something that actually
/// exists on disk or was actually computed.
pub struct SealOutcome {
    /// Diagnostics, populated only in creator mode.
    pub diagnostics: Vec<String>,
    /// Security downgrades that the operator must be shown, in any mode.
    pub warnings: Vec<String>,
    pub project_dir: PathBuf,
    pub package_bytes: usize,
    pub trusted_machines: usize,
    pub creator_fingerprint: String,
    pub build_nonce: u64,
    pub package_commitment: [u8; 32],
    pub runtime_commitment: [u8; 32],
    pub entries: usize,
}

#[derive(Debug)]
pub enum SealError {
    /// The operator cancelled the build.
    Cancelled,
    /// Hardware-backed machine protection was required and is not usable.
    HardwareRequired(String),
    SourceMissing(PathBuf),
    TrustFileUnreadable(PathBuf),
    TrustFileUnauthentic(PathBuf),
    /// Neither the native provider nor the browser flow produced a reading.
    /// There is deliberately no manual fallback.
    LocationFailed(String),
    /// A reading arrived, but it was too coarse to prove the region.
    LocationTooCoarse { reported_m: f64, required_m: u32 },
    Crypto(&'static str),
    Io(String),
}

impl std::fmt::Display for SealError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SealError::Cancelled => write!(f, "the build was cancelled; nothing was written"),
            SealError::HardwareRequired(m) => write!(f, "{m}"),
            SealError::SourceMissing(p) => write!(f, "source not found: {}", p.display()),
            SealError::TrustFileUnreadable(p) => write!(f, "cannot read {}", p.display()),
            SealError::TrustFileUnauthentic(p) => {
                write!(f, "{} failed authentication and was refused", p.display())
            }
            SealError::LocationFailed(m) => write!(
                f,
                "could not obtain a location: {m}\n\nNYEDArch tried the operating system's \
                 location service and then your browser. It never falls back to typed \
                 coordinates or to an IP lookup, because both are trivially spoofed and would \
                 make the protection meaningless."
            ),
            SealError::LocationTooCoarse { reported_m, required_m } => write!(
                f,
                "the location fix is accurate to about {reported_m:.0} m, which is coarser than \
                 the {required_m} m you required. Refusing to seal a region that cannot be \
                 reproduced."
            ),
            SealError::Crypto(m) => write!(f, "cryptographic operation failed: {m}"),
            SealError::Io(m) => write!(f, "{m}"),
        }
    }
}

fn rnd<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("csprng");
    b
}

fn sha256(data: &[u8], domain: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(domain);
    h.update(data);
    h.finalize().into()
}

/// Key used to authenticate portable `.nyfp` records. In the shipped client
/// this lives in OS-native secure storage; the prototype derives a stable local
/// key so import and export round-trip.
pub fn nyfp_key() -> [u8; 32] {
    match crate::keystore::derive(b"nyedarch:v1:nyfp") {
        Ok(k) => *k,
        Err(_) => {
            // Refusing outright would make the client unusable for a reason the
            // user cannot act on, so this degrades to a per-machine value.
            // `keystore::current_source()` reports the degradation; it is never
            // silent, and it is still not a constant compiled into the binary.
            let fp = nyedarch_fingerprint::capture();
            *nyedarch_crypto::kdf::hkdf_key(
                b"nyedarch:v1:nyfp:degraded",
                &fp.id,
                b"nyedarch:v1:nyfp",
            )
            .expect("hkdf")
        }
    }
}

/// Seal a source into a capsule project.
///
/// `on_stage` is called before each stage begins, so a caller reports progress
/// that corresponds to work actually starting.
pub fn seal_and_generate(
    req: &SealRequest,
    crates_root: &Path,
    mut on_stage: impl FnMut(Stage),
) -> Result<SealOutcome, SealError> {
    if !req.source.exists() {
        return Err(SealError::SourceMissing(req.source.clone()));
    }
    let is_cancelled = || req.cancelled.load(std::sync::atomic::Ordering::Relaxed);
    let mut diagnostics: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Hardware-backed machine protection is decided before any work begins, so
    // a refusal costs nothing and a downgrade is never silent.
    let backing = nyedarch_platform::hardware::detect();
    match nyedarch_platform::hardware::decide(req.hardware, backing) {
        nyedarch_platform::hardware::HardwareDecision::UseHardware => {
            diagnostics.push("machine factor: hardware-backed path selected".into());
        }
        nyedarch_platform::hardware::HardwareDecision::UseSoftware { warning } => {
            // Always recorded, not only in creator mode: this is a security
            // downgrade and the operator has to see it.
            warnings.push(warning);
        }
        nyedarch_platform::hardware::HardwareDecision::Refuse { reason } => {
            return Err(SealError::HardwareRequired(reason));
        }
    }

    // The project is assembled under a temporary name and moved into place only
    // once it is complete. A cancelled or failed build therefore never leaves
    // something that looks like a usable capsule project.
    let staging = req.project_dir.with_extension("partial");
    let _ = std::fs::remove_dir_all(&staging);

    // --- trusted machines -------------------------------------------------
    on_stage(Stage::CapturingFingerprint);
    if is_cancelled() {
        return Err(SealError::Cancelled);
    }
    let fp = nyedarch_fingerprint::capture();
    if req.creator_mode {
        diagnostics.push(format!(
            "fingerprint {} from {} signal(s), best strength {:?}",
            &fp.id_hex()[..16],
            fp.signals.len(),
            fp.best_strength
        ));
        for s in &fp.signals {
            // Names and strengths only: the digests stay opaque and raw
            // identifiers are never held in the first place.
            diagnostics.push(format!("  signal {} ({:?})", s.name, s.strength));
        }
        diagnostics.push(format!(
            "client key storage: {:?}",
            crate::keystore::current_source()
        ));
    }
    let creator_secret = rnd::<32>();
    let mut entries = vec![PolicyEntry {
        fingerprint_id: fp.id,
        factor_secret: creator_secret,
        labels: vec!["creator".into(), "this-machine".into()],
    }];

    on_stage(Stage::LoadingTrustedMachines);
    for path in &req.trust_files {
        let bytes =
            std::fs::read(path).map_err(|_| SealError::TrustFileUnreadable(path.clone()))?;
        let rec = nyedarch_fingerprint::nyfp::open(&nyfp_key(), &bytes)
            .map_err(|_| SealError::TrustFileUnauthentic(path.clone()))?;
        if rec.fingerprint.id == fp.id {
            continue; // already present as the creator
        }
        entries.push(PolicyEntry {
            fingerprint_id: rec.fingerprint.id,
            factor_secret: rnd::<32>(),
            labels: rec.labels.clone(),
        });
    }
    let trusted_machines = entries.len();
    let record = PolicyRecord { entries };

    // --- identity and policy ----------------------------------------------
    let package_id = rnd::<16>();
    let runtime_binding = rnd::<32>();
    let package_salt = rnd::<32>();
    let bootstrap_key = rnd::<32>();
    let binding = Binding {
        package_id,
        runtime_binding,
        crypto_version: version::CRYPTO_VERSION,
    };
    let policy = Policy {
        location: req.location_tolerance_m.is_some(),
        time: req.schedule.is_some(),
        one_shot: req.one_shot,
    };
    let flags = policy.to_flags();
    let context = compose::payload_aad(&binding, flags);

    let record_bytes = bincode::serialize(&record).map_err(|_| SealError::Crypto("policy record"))?;
    let sealed_policy = nyedarch_crypto::policy_seal::seal_record(
        &bootstrap_key,
        &runtime_binding,
        &package_salt,
        &context,
        &record_bytes,
    )
    .map_err(|_| SealError::Crypto("seal policy record"))?
    .to_bytes();

    // --- location ----------------------------------------------------------
    let location_cell = match req.location_tolerance_m {
        Some(tol_m) => {
            on_stage(Stage::AcquiringLocation);
            // Native provider first; the browser consent flow is the fallback,
            // because most desktops have no usable native provider. Never typed
            // in, and never an IP lookup.
            let (reading, source) =
                nyedarch_platform::acquire_location_with_fallback(Some(tol_m))
                    .map_err(|e| SealError::LocationFailed(e))?;
            let _ = source;
            if reading.accuracy_m > tol_m as f64 {
                return Err(SealError::LocationTooCoarse {
                    reported_m: reading.accuracy_m,
                    required_m: tol_m,
                });
            }
            Some(
                nyedarch_crypto::geo::quantize(
                    &reading,
                    nyedarch_crypto::geo::ToleranceMeters(tol_m),
                )
                .map_err(|_| SealError::Crypto("quantize location"))?,
            )
        }
        None => None,
    };

    // --- time --------------------------------------------------------------
    let time_window = match &req.schedule {
        Some(sc) => Some(
            sc.window_id_for_slot(sc.slots_minutes[0])
                .map_err(|_| SealError::Crypto("time window"))?,
        ),
        None => None,
    };

    // --- key composition ---------------------------------------------------
    on_stage(Stage::DerivingKeys);
    if is_cancelled() {
        return Err(SealError::Cancelled);
    }
    let t_argon = std::time::Instant::now();
    let pass_key = passphrase_key(req.passphrase.as_bytes(), &package_salt, req.argon)
        .map_err(|_| SealError::Crypto("argon2id"))?;
    let contrib = Contributions {
        machine_secret: Some(creator_secret),
        passphrase_key: Some(pass_key),
        location_cell,
        time_window,
    };
    let payload_key = compose::derive_payload_key(&package_salt, &binding, flags, &contrib)
        .map_err(|_| SealError::Crypto("derive payload key"))?;

    if req.creator_mode {
        diagnostics.push(format!("argon2id took {:?}", t_argon.elapsed()));
    }

    // --- package -----------------------------------------------------------
    on_stage(Stage::CollectingFiles);
    if is_cancelled() {
        return Err(SealError::Cancelled);
    }
    let (manifest, files) =
        collect(&[req.source.clone()]).map_err(|e| SealError::Io(format!("collect: {e}")))?;

    on_stage(Stage::SealingPayload);
    let header = Header {
        crypto_version: version::CRYPTO_VERSION,
        package_id,
        runtime_binding,
        policy,
        argon: req.argon.into(),
        package_salt,
        compression: COMPRESSION_ZSTD,
        chunk_size: 256 * 1024,
    };
    let payload_bytes: u64 = files.iter().map(|f| f.size).sum();
    let level = req.compression.level(payload_bytes);
    if req.creator_mode {
        diagnostics.push(format!(
            "compression {} (level {level}) over {payload_bytes} byte(s)",
            req.compression.label()
        ));
    }
    let t_seal = std::time::Instant::now();
    let pkg = nyedarch_package::build::seal_package_cancellable(
        &header,
        &sealed_policy,
        &payload_key,
        &manifest,
        &files,
        level,
        &is_cancelled,
    )
    .map_err(|_| {
        if is_cancelled() {
            SealError::Cancelled
        } else {
            SealError::Crypto("seal package")
        }
    })?;
    if req.creator_mode {
        diagnostics.push(format!("sealed {} byte(s) in {:?}", pkg.len(), t_seal.elapsed()));
    }

    // --- generate the capsule project --------------------------------------
    on_stage(Stage::GeneratingProject);
    let diversifier = u64::from_le_bytes(rnd::<8>());
    let gen_cfg = crate::config::ProtectionConfig {
        schedule: req.schedule.clone(),
        location_tolerance_m: req.location_tolerance_m,
        one_shot: req.one_shot,
        trust_files: Vec::new(),
        trust_tags: Vec::new(),
        tag_mode_all: false,
        trust_query: String::new(),
        compression: req.compression,
        creator_mode: req.creator_mode,
        hardware: req.hardware,
    };
    if is_cancelled() {
        return Err(SealError::Cancelled);
    }
    std::fs::create_dir_all(&staging)
        .map_err(|e| SealError::Io(format!("cannot create {}: {e}", staging.display())))?;
    generator::generate(
        &staging,
        &pkg,
        &bootstrap_key,
        &runtime_binding,
        crates_root,
        diversifier,
        &gen_cfg,
    )
    .map_err(|e| SealError::Io(format!("generate capsule project: {e}")))?;

    if is_cancelled() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(SealError::Cancelled);
    }
    // Move the finished project into place. Only now does a usable project
    // exist at the path the caller was told about.
    let _ = std::fs::remove_dir_all(&req.project_dir);
    std::fs::rename(&staging, &req.project_dir).map_err(|e| {
        let _ = std::fs::remove_dir_all(&staging);
        SealError::Io(format!("cannot finalise {}: {e}", req.project_dir.display()))
    })?;

    on_stage(Stage::Done);
    Ok(SealOutcome {
        diagnostics,
        warnings,
        project_dir: req.project_dir.clone(),
        package_bytes: pkg.len(),
        trusted_machines,
        creator_fingerprint: fp.id_hex()[..16].to_string(),
        build_nonce: diversifier,
        package_commitment: sha256(&pkg, b"nyedarch:v1:package-commitment"),
        runtime_commitment: runtime_binding,
        entries: trusted_machines,
    })
}

/// Locate the vendored runtime crate sources.
///
/// A distributed client has no source workspace, so this cannot be baked in at
/// compile time. Order: `NYEDARCH_RUNTIME_SRC`, then `runtime-src` beside or
/// above the executable, then the development workspace.
pub fn runtime_source_root() -> PathBuf {
    if let Some(p) = std::env::var_os("NYEDARCH_RUNTIME_SRC") {
        let p = PathBuf::from(p);
        if p.join("nyedarch-runtime").is_dir() {
            return p;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for c in [
                dir.join("runtime-src"),
                dir.join("../runtime-src"),
                // Inside a macOS application bundle the executable lives in
                // Contents/MacOS and resources in Contents/Resources.
                dir.join("../Resources/runtime-src"),
            ] {
                if c.join("nyedarch-runtime").is_dir() {
                    return c;
                }
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("crates"))
}

/// Compile a generated capsule project on this machine.
///
/// The remote build is how a capsule is produced for *other* platforms, and it
/// stays the default. But when it cannot deliver - no network, a failed run, a
/// timeout - the client used to hand the user a directory and tell them to run
/// `stage-capsule.sh` themselves. That is the client's job: it has the project,
/// it knows where the capsule goes, and asking someone to run a shell script to
/// finish a build they already started is not a product.
///
/// Returns the path of the compiled capsule inside the project.
pub fn build_locally(
    project_dir: &std::path::Path,
    mut on_line: impl FnMut(String),
) -> Result<std::path::PathBuf, String> {
    on_line("Building the capsule on this machine.".to_string());
    on_line("This compiles the capsule's own source, so it takes a minute.".to_string());

    let out = std::process::Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(project_dir)
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .map_err(|e| format!("could not run cargo: {e}. Is the Rust toolchain installed?"))?;

    if !out.status.success() {
        // The compiler's own words are far more useful than "build failed".
        let err = String::from_utf8_lossy(&out.stderr);
        for line in err.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev() {
            on_line(line.to_string());
        }
        return Err("the capsule did not compile on this machine".to_string());
    }

    let mut candidate = project_dir.join("target/release/nyedarch-capsule");
    if !candidate.exists() {
        candidate = project_dir.join("target/release/nyedarch-capsule.exe");
    }
    if !candidate.exists() {
        return Err("the build reported success but produced no capsule".to_string());
    }
    Ok(candidate)
}
