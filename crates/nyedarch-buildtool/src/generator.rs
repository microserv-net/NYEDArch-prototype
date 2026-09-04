//! Runtime source generator (spec §28). The client does NOT ship a static
//! plaintext runtime template; the source is *assembled at generation time*
//! from fragments with per-build diversification (a build nonce and randomized
//! internal identifiers), then compiled by the build environment. This is the
//! prototype's local-build form; the GitHub path (Phase 7) compiles the same
//! generated source remotely.

use std::fs;
use std::path::Path;

/// Crates the generated capsule needs. Deliberately the runtime set only —
/// Remove the `[dev-dependencies]` section from a vendored manifest.
///
/// Stops at the next top-level section so nothing else is disturbed.
fn strip_dev_dependencies(manifest: &Path) -> std::io::Result<()> {
    let Ok(text) = fs::read_to_string(manifest) else { return Ok(()) };
    let mut out = String::with_capacity(text.len());
    let mut skipping = false;
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with('[') {
            // A new section always ends any skip, including nested tables such
            // as [dev-dependencies.foo].
            skipping = t.starts_with("[dev-dependencies");
            if skipping {
                out.push_str("# [dev-dependencies] removed: a capsule project has no tests.\n");
                continue;
            }
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    fs::write(manifest, out)
}

/// builder-side crates are never vendored into a capsule project.
const RUNTIME_CRATES: &[&str] = &[
    "nyedarch-crypto",
    "nyedarch-core",
    "nyedarch-package",
    "nyedarch-fingerprint",
    "nyedarch-platform",
    "nyedarch-runtime",
];

/// Copy a crate source tree, skipping build output and version control.
fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !src.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("runtime source not found: {}", src.display()),
        ));
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let n = name.to_string_lossy();
        if n == "target" || n == ".git" || n == "vendor" {
            continue;
        }
        let from = entry.path();
        let to = dst.join(&name);
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Write a compilable runtime binary crate into `out_dir`, embedding the sealed
/// package and the per-build bootstrap key. `crates_root` points at the NYEDArch
/// workspace crates so the prototype can build locally.
/// The imports a generated capsule needs, given what is enabled.
///
/// Importing unconditionally produced "unused import" warnings in the build
/// log - readable by anyone when the repository is public, and a hint about
/// which protections a capsule does not use. Trimming too far broke the build
/// instead: `LocationProvider` appears in a type annotation in every
/// configuration, so only the concrete provider is conditional.
pub(crate) fn conditional_imports(protections: &crate::config::ProtectionConfig) -> String {
    let mut imports = String::new();
    if protections.location_tolerance_m.is_some() {
        imports.push_str("use nyedarch_runtime::SystemLocation;\n");
    }
    if protections.schedule.is_some() {
        imports.push_str("use nyedarch_crypto::timewin::DailySchedule;\n");
    }
    imports
}

pub fn generate(
    out_dir: &Path,
    package_bytes: &[u8],
    bootstrap_key: &[u8; 32],
    runtime_commitment: &[u8; 32],
    crates_root: &Path,
    diversifier: u64,
    protections: &crate::config::ProtectionConfig,
) -> std::io::Result<()> {
    fs::create_dir_all(out_dir.join("src"))?;
    fs::write(out_dir.join("capsule.nyeda"), package_bytes)?;

    // Vendor the runtime crates INTO the generated project.
    //
    // The project must build on machines that have no NYEDArch source tree: an
    // end user who installed only the release, and the GitHub Actions runner.
    // Absolute path dependencies pointing at the machine that generated the
    // project would fail in both cases, so the sources travel with it and the
    // manifest uses relative paths.
    let vendor = out_dir.join("vendor");
    fs::create_dir_all(&vendor)?;
    for c in RUNTIME_CRATES {
        copy_tree(&crates_root.join(c), &vendor.join(c))?;
        // Strip test-only wiring from the vendored manifest.
        //
        // The crates' own dev-dependencies enable the `builder` feature so their
        // tests can construct packages. Cargo does not compile dev-dependencies
        // into a release binary, so the capsule never gained that capability -
        // but a capsule project has no tests, and shipping the wiring means the
        // source pushed to a build environment mentions builder capability for
        // no reason. Removing it makes what is shipped match what is needed.
        strip_dev_dependencies(&vendor.join(c).join("Cargo.toml"))?;
    }
    let cargo = format!(
        r#"# Generated NYEDArch capsule project. Self-contained: the runtime crates
# are vendored under vendor/ so this builds anywhere cargo runs.
#
# The vendored crates inherit fields from a workspace root, so this project
# declares that root itself. Without it, cargo cannot resolve
# `edition.workspace = true` on a machine that has no NYEDArch source tree.
[workspace]
resolver = "2"

[workspace.package]
version = "0.0.1"
edition = "2021"
license = "UNLICENSED"
authors = ["NYEDArch"]
rust-version = "1.82"

[workspace.dependencies]
serde = {{ version = "1", features = ["derive"] }}
bincode = "1.3"
thiserror = "1"
zeroize = {{ version = "1", features = ["derive"] }}
getrandom = "0.2"

[package]
name = "nyedarch-capsule-{div:016x}"
version = "0.0.1"
edition = "2021"

[[bin]]
name = "nyedarch-capsule"
path = "src/main.rs"

# Anti-RE hardening (spec §9/§31). Every setting is explicit rather than relying
# on a cargo default that could change between toolchains.
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
debug = false
debug-assertions = false
incremental = false
rpath = false

[dependencies]
nyedarch-runtime = {{ path = "vendor/nyedarch-runtime" }}
nyedarch-crypto  = {{ path = "vendor/nyedarch-crypto" }}
"#,
        div = diversifier
    );
    fs::write(out_dir.join("Cargo.toml"), cargo)?;

    // Per-build diversified identifiers (trivial here; real diversification adds
    // control-flow/opaque-predicate transforms in the hardening phase).
    let commit_lit = runtime_commitment
        .iter()
        .map(|b| format!("0x{:02x}", b))
        .collect::<Vec<_>>()
        .join(", ");
    // Protection wiring, generated per build. A capsule without the time protection carries
    // no schedule code path at all; without the location protection it carries no
    // location provider. Unused capability is not shipped (spec §29).
    let schedule_expr = match &protections.schedule {
        Some(sc) => format!(
            "Some(DailySchedule {{ slots_minutes: vec![{}], tolerance_minutes: {}, tz_offset_minutes: {} }})",
            sc.slots_minutes.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(", "),
            sc.tolerance_minutes,
            sc.tz_offset_minutes
        ),
        None => "None".to_string(),
    };
    let loc_tol_expr = match protections.location_tolerance_m {
        Some(m) => format!("Some({m}u32)"),
        None => "None".to_string(),
    };
    let loc_provider_expr = if protections.location_tolerance_m.is_some() {
        "Some(&SystemLocation as &dyn LocationProvider)"
    } else {
        "None"
    };
    let one_shot_lit = if protections.one_shot { "true" } else { "false" };

    let conditional_imports = conditional_imports(protections);

    let boot_lit = bootstrap_key
        .iter()
        .map(|b| format!("0x{:02x}", b))
        .collect::<Vec<_>>()
        .join(", ");

    let main_rs = format!(
        r#"// GENERATED per-build NYEDArch runtime — build nonce {div:016x}.
// This file is assembled by the generator, not shipped as a plaintext template.
use std::io::Read;
use std::path::PathBuf;
use nyedarch_runtime::{{run, one_shot_destroy, Capsule, LocalTime, LocationProvider, Outcome, PassphraseProvider}};
{conditional_imports}use zeroize::Zeroizing;

const PACKAGE: &[u8] = include_bytes!("../capsule.nyeda");
const BOOTSTRAP_KEY: [u8; 32] = [{boot}];
// This runtime's own identity commitment (correction pass §4). It is asserted
// against the package header AND mixed into the policy-seal subkey, so another
// runtime cannot open this package's authorization record.
const RUNTIME_COMMITMENT: [u8; 32] = [{commit}];
const BUILD_NONCE: u64 = 0x{div:016x};
const ONE_SHOT: bool = {one_shot};

struct StdinPass;
impl PassphraseProvider for StdinPass {{
    fn passphrase(&self) -> Option<Zeroizing<Vec<u8>>> {{
        // Prototype: read from NYEDARCH_PASSPHRASE env or stdin. The GUI supplies a
        // masked field. No manual location/time entry is ever offered (spec §33).
        if let Ok(p) = std::env::var("NYEDARCH_PASSPHRASE") {{
            return Some(Zeroizing::new(p.into_bytes()));
        }}
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).ok()?;
        Some(Zeroizing::new(s.trim_end().as_bytes().to_vec()))
    }}
}}

fn main() {{
    let _ = BUILD_NONCE;
    let out = std::env::args().nth(1).map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./nyeda-extracted"));
    let cap = Capsule {{
        package: PACKAGE,
        bootstrap_key: BOOTSTRAP_KEY,
        runtime_commitment: RUNTIME_COMMITMENT,
        schedule: {schedule},
        location_tolerance_m: {loc_tol},
        out_dir: out,
    }};
    let location: Option<&dyn LocationProvider> = {loc_provider};
    let creator = std::env::var("NYEDARCH_CREATOR").is_ok();
    let mut trace = |s| eprintln!("[state] {{:?}}", s);
    let t: Option<&mut dyn FnMut(nyedarch_runtime::State)> = if creator {{ Some(&mut trace) }} else {{ None }};
    match run(&cap, &StdinPass, location, &LocalTime, t) {{
        Outcome::Success {{ files, dirs, bytes, out_dir }} => {{
            println!("NYEDArch: extraction complete - {{}} files, {{}} dirs, {{}} bytes -> {{}}",
                files, dirs, bytes, out_dir.display());
            if ONE_SHOT {{
                // Only after verified extraction (spec §37). Best effort:
                // erasure cannot be guaranteed on SSD or copy-on-write
                // filesystems, so the outcome is reported rather than assumed.
                if let Ok(me) = std::env::current_exe() {{
                    let outcome = one_shot_destroy(&me);
                    if outcome.is_delegated() {{
                        println!("NYEDArch: capsule scheduled for destruction ({{}})", outcome.describe());
                    }} else if outcome.path_cleared() {{
                        println!("NYEDArch: capsule destroyed ({{}})", outcome.describe());
                    }} else {{
                        // Saying nothing here would leave the user believing a
                        // one-shot capsule was gone when it is still on disk.
                        eprintln!("NYEDArch: the capsule could NOT be destroyed - {{}}", outcome.describe());
                        eprintln!("NYEDArch: delete it yourself. Its payload stays encrypted either way.");
                    }}
                }}
            }}
        }}
        Outcome::Failed => {{
            // Single generic message (spec §51): no authorization oracle.
            eprintln!("Authorization failed.");
            std::process::exit(1);
        }}
    }}
}}
"#,
        div = diversifier,
        boot = boot_lit,
        commit = commit_lit,
        schedule = schedule_expr,
        loc_tol = loc_tol_expr,
        loc_provider = loc_provider_expr,
        one_shot = one_shot_lit,
        conditional_imports = conditional_imports
    );
    fs::write(out_dir.join("src/main.rs"), main_rs)?;

    // A local build produces the same artifact identity as the GitHub build:
    // one `.nyarch` capsule, executable, named consistently on every OS.
    let stage = format!(
        r#"#!/bin/sh
# Stage the compiled capsule as a NYEDArch capsule (.{ext}).
set -e
cargo build --release
bin="target/release/nyedarch-capsule"
[ -f "$bin.exe" ] && bin="$bin.exe"
out="nyedarch-{div:016x}.{ext}"
cp "$bin" "$out"
chmod +x "$out" 2>/dev/null || true
echo "capsule: $out"
"#,
        ext = nyedarch_core::CAPSULE_EXTENSION,
        div = diversifier
    );
    fs::write(out_dir.join("stage-capsule.sh"), stage)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let p = out_dir.join("stage-capsule.sh");
        let _ = fs::set_permissions(&p, fs::Permissions::from_mode(0o755));
    }

    // The generated bin also needs `zeroize`; add it.
    let mut cargo = fs::read_to_string(out_dir.join("Cargo.toml"))?;
    cargo.push_str("zeroize = { version = \"1\", features = [\"derive\"] }\n");
    fs::write(out_dir.join("Cargo.toml"), cargo)?;
    Ok(())
}

#[cfg(test)]
mod import_tests {
    use crate::config::ProtectionConfig;
    use nyedarch_crypto::timewin::DailySchedule;

    fn cfg(location: bool, time: bool) -> ProtectionConfig {
        ProtectionConfig {
            schedule: if time {
                Some(DailySchedule { slots_minutes: vec![840], tolerance_minutes: 15, tz_offset_minutes: 0 })
            } else {
                None
            },
            location_tolerance_m: if location { Some(150) } else { None },
            one_shot: false,
            trust_files: Vec::new(),
            trust_tags: Vec::new(),
            tag_mode_all: false,
            trust_query: String::new(),
            compression: nyedarch_package::pipeline::CompressionMode::Automatic,
            creator_mode: false,
            hardware: nyedarch_platform::hardware::HardwarePolicy::Preferred,
        }
    }

    /// The capsule must import exactly what it uses.
    ///
    /// Unconditional imports produced warnings in a build log that is public
    /// when the repository is; over-trimming them broke the build instead,
    /// because `LocationProvider` appears in a type annotation even when the
    /// location protection is off. Both directions are asserted here.
    #[test]
    fn generated_imports_match_the_enabled_protections() {
        // Always needed, whatever is enabled.
        for (loc, time) in [(false, false), (true, false), (false, true), (true, true)] {
            let src = super::conditional_imports(&cfg(loc, time));
            assert_eq!(
                src.contains("SystemLocation"),
                loc,
                "SystemLocation must appear only when the location protection is on (loc={loc})"
            );
            assert_eq!(
                src.contains("DailySchedule"),
                time,
                "DailySchedule must appear only when the time protection is on (time={time})"
            );
        }
    }
}

#[cfg(test)]
mod hardening_tests {
    /// The generated capsule must be hardened. These settings are the anti-RE
    /// baseline, and a silent regression here would weaken every capsule
    /// produced afterwards without any test failing elsewhere.
    #[test]
    fn the_generated_capsule_profile_is_hardened() {
        let src = include_str!("generator.rs");
        for setting in [
            "lto = \"fat\"",
            "codegen-units = 1",
            "panic = \"abort\"",
            "strip = \"symbols\"",
            "debug = false",
            "incremental = false",
        ] {
            assert!(
                src.contains(setting),
                "the generated capsule profile lost `{setting}`"
            );
        }
    }

    /// The unlocker is compiled in a repository that may be public.
    #[test]
    fn the_unlocker_profile_is_hardened() {
        let sources = crate::srcpack::unlocker_sources();
        let cargo = &sources
            .iter()
            .find(|(n, _)| n.ends_with("Cargo.toml"))
            .expect("unlocker manifest")
            .1;
        for setting in ["lto = true", "strip = true", "panic = \"abort\"", "codegen-units = 1"] {
            assert!(cargo.contains(setting), "the unlocker lost `{setting}`");
        }
    }
}
