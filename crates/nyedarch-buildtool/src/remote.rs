//! Remote GitHub build — a required stage of the NYEDArch flow, not an option.
//!
//! The client orchestrates GitHub as an untrusted build environment: it creates
//! and configures the repository, seals and stores repository secrets, pushes
//! the generated runtime and workflow, dispatches the build, follows it, and
//! then verifies artifact provenance before accepting the resulting `.nyarch`
//! capsule.

use std::path::Path;

use nyedarch_github::{
    provenance::{BuildManifest, ProvenanceError},
    BuildInputs, CurlTransport, Orchestrator, RepoConfig, Target, Transport,
};

pub struct RemoteBuildArgs<'a> {
    /// Supplied by the caller (GUI) or read from the environment (CLI).
    pub token: Option<String>,
    pub owner: &'a str,
    pub repo: &'a str,
    pub private: bool,
    pub targets: Vec<Target>,
    pub project_dir: &'a Path,
    pub build_id: String,
    pub package_commitment: [u8; 32],
    pub runtime_commitment: [u8; 32],
}

/// Read the token from the environment so it never appears in shell history or
/// in this process's argv.
pub fn token() -> Option<String> {
    std::env::var("NYEDARCH_GITHUB_TOKEN").ok().or_else(|| std::env::var("GITHUB_TOKEN").ok())
}

/// Prepare what the build repository receives.
///
/// The capsule source never leaves as plaintext. The whole project is packed
/// into one archive, encrypted under a fresh random key, and pushed as
/// ciphertext alongside a small unlocker that holds no secret. The key travels
/// separately, as a repository secret sealed to the repository's public key, so
/// it is readable only inside the Actions runner.
///
/// Returns the files to push and the source key to store as that secret.
pub fn prepare_encrypted_push(
    project_dir: &Path,
) -> std::io::Result<(Vec<(String, Vec<u8>)>, [u8; 32])> {
    // The unlocker is written into the project but excluded from the archive it
    // is meant to open.
    let unlocker = crate::srcpack::unlocker_sources();
    for (rel, body) in &unlocker {
        let path = project_dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, body)?;
    }

    let archive = crate::srcpack::pack(project_dir, &["unlock"])?;

    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key)
        .map_err(|_| std::io::Error::other("no secure randomness available"))?;
    let sealed = nyedarch_crypto::aead::seal(&key, crate::srcpack::ARCHIVE_AAD, &archive)
        .map_err(|_| std::io::Error::other("could not encrypt the capsule source"))?
        .to_bytes();

    let mut out: Vec<(String, Vec<u8>)> = vec![("runtime/capsule.sealed".to_string(), sealed)];
    for (rel, body) in unlocker {
        out.push((format!("runtime/{rel}"), body.into_bytes()));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok((out, key))
}

/// Collect every file of the generated runtime project.
///
/// Retained for local inspection and tests; the remote path uses
/// `prepare_encrypted_push` so nothing sensitive is committed in the clear.
pub fn collect_runtime_files(project_dir: &Path) -> std::io::Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    walk(project_dir, project_dir, &mut out)?;
    out.sort_by(|a, b| a.0.cmp(&b.0)); // deterministic ordering for the commitment
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let n = name.to_string_lossy();
        if n == "target" || n == ".git" || n == "Cargo.lock" {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            out.push((format!("runtime/{rel}"), std::fs::read(&path)?));
        }
    }
    Ok(())
}

/// Run the remote build. Returns the workflow run id on success.
pub fn run_remote_build(args: &RemoteBuildArgs<'_>) -> Result<u64, String> {
    run_remote_build_with(args, |m| println!("[remote] {m}"))
}

/// Same, but reporting progress through a callback so a GUI can show it.
pub fn run_remote_build_with(
    args: &RemoteBuildArgs<'_>,
    mut report: impl FnMut(String),
) -> Result<u64, String> {
    let token = args.token.clone().or_else(token).ok_or_else(|| {
        "no GitHub token found. Set NYEDARCH_GITHUB_TOKEN (preferred) or GITHUB_TOKEN.\n\
         The token is read from the environment so it never enters argv or shell history."
            .to_string()
    })?;

    let transport = CurlTransport::new();
    if !transport.available() {
        return Err("`curl` was not found. NYEDArch uses it as its HTTP transport; \
                    it ships with Windows 10+, macOS, and mainstream Linux."
            .to_string());
    }

    let (files, source_key) = prepare_encrypted_push(args.project_dir)
        .map_err(|e| format!("cannot prepare the capsule source: {e}"))?;
    if files.is_empty() {
        return Err("generated capsule project is empty - seal a package first".to_string());
    }
    let file_refs: Vec<(String, &[u8])> = files.iter().map(|(p, b)| (p.clone(), b.as_slice())).collect();
    let source_key_hex: String = source_key.iter().map(|b| format!("{b:02x}")).collect();

    // Commit to the build BEFORE dispatch, so the returned artifact can be
    // checked against something we chose rather than something GitHub reports.
    let manifest = BuildManifest {
        build_id: args.build_id.clone(),
        package_commitment: args.package_commitment,
        runtime_commitment: args.runtime_commitment,
        source_commitment: source_commitment(&files),
        target: *args.targets.first().unwrap_or(&Target::LinuxGnu),
        crypto_version: nyedarch_crypto::version::CRYPTO_VERSION,
    };

    // Real libsodium sealed box. A secret is never transmitted unsealed.
    let sealer = nyedarch_github::SodiumSealer;
    let mut orch = Orchestrator::new(&transport, &sealer);
    let cfg = RepoConfig {
        token,
        owner: args.owner.to_string(),
        repo: args.repo.to_string(),
        private: args.private,
    };
    let inputs = BuildInputs {
        runtime_files: file_refs,
        runtime_dir: "runtime".to_string(),
        targets: args.targets.clone(),
        secrets: vec![
            // Sealed to the repository's public key before transmission; only
            // the runner can read it.
            ("NYEDARCH_SOURCE_KEY".to_string(), source_key_hex.clone().into_bytes()),
            ("NYEDARCH_GH_TO_CLIENT_PUB".to_string(), b"<channel-B public key>".to_vec()),
        ],
        git_ref: "main".to_string(),
        package_commitment: args.package_commitment,
    };

    report(format!("Build id {}", manifest.build_id));
    report(format!(
        "Pushing {} file(s) to {}/{}: the capsule source is encrypted, only the unlocker is in the clear",
        files.len(),
        args.owner,
        args.repo
    ));
    let run_id = orch
        .run_build(&cfg, &inputs, |state| report(format!("{state:?}")))
        .map_err(|e| format!("remote build failed: {e}"))?;
    report(format!("Workflow run {run_id} dispatched"));
    // Store the verified artifact next to the capsule project, so the build
    // ends with something the user can hand over rather than a run id (§45).
    if let Some(bytes) = &orch.last_artifact {
        let dest = args.project_dir.join("artifact.zip");
        match std::fs::write(&dest, bytes) {
            Ok(()) => report(format!(
                "Artifact verified against the committed package and saved to {}",
                dest.display()
            )),
            Err(e) => report(format!("Artifact verified but could not be saved: {e}")),
        }
    } else {
        report(
            "No artifact was retrieved. The build may still be running, or the artifact was not              reachable from this network."
                .to_string(),
        );
    }
    report("Artifact will be checked against the committed build manifest before use.".to_string());
    Ok(run_id)
}

fn source_commitment(files: &[(String, Vec<u8>)]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"nyedarch:v1:source-commitment");
    let mut sorted: Vec<&(String, Vec<u8>)> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (p, b) in sorted {
        h.update(p.as_bytes());
        h.update([0]);
        h.update(b);
    }
    h.finalize().into()
}

/// Who does this token belong to?
///
/// The build repository lives under an account, and the API needs to know
/// which. Asking the user to type it is asking them to repeat something the
/// token already proves - and to get it wrong. This reads the login from the
/// token itself, so the field can be shown as a fact rather than a question.
pub fn resolve_owner(token: Option<String>) -> Result<String, String> {
    let token = token.or_else(self::token).ok_or_else(|| {
        "no GitHub token found. Set NYEDARCH_GITHUB_TOKEN (preferred) or GITHUB_TOKEN.".to_string()
    })?;
    let transport = CurlTransport::new();
    if !transport.available() {
        return Err("`curl` was not found; it is used as the HTTP transport.".to_string());
    }
    let r = transport
        .send(&nyedarch_github::endpoints::authenticated_user(&token))
        .map_err(|e| format!("could not identify the token's account: {e}"))?;
    if !(200..300).contains(&r.status) {
        return Err(format!("GitHub refused the token ({})", r.status));
    }
    let body = String::from_utf8_lossy(&r.body);
    // "login" is the account name the repository will be created under.
    let i = body.find("\"login\"").ok_or("no login in the response")? + 7;
    let rest = body[i..].trim_start().trim_start_matches(':').trim_start();
    let rest = rest.strip_prefix('"').ok_or("unexpected response shape")?;
    let end = rest.find('"').ok_or("unexpected response shape")?;
    Ok(rest[..end].to_string())
}

/// Change a repository's visibility.
///
/// Refused while a build is running (spec §38). The check asks GitHub whether a
/// workflow run is queued or in progress, rather than trusting local state: the
/// build may have been started from the other client, or from another machine
/// entirely.
///
/// Visibility does not change what is pushed. The capsule source is encrypted
/// either way, because a private repository still exposes it to collaborators
/// and to anyone later granted read access.
pub fn set_repository_visibility(
    token: Option<String>,
    owner: &str,
    repo: &str,
    private: bool,
) -> Result<(), String> {
    let token = token.or_else(self::token).ok_or_else(|| {
        "no GitHub token found. Set NYEDARCH_GITHUB_TOKEN (preferred) or GITHUB_TOKEN.".to_string()
    })?;
    let transport = CurlTransport::new();
    if !transport.available() {
        return Err("`curl` was not found; it is used as the HTTP transport.".to_string());
    }

    // Refuse while a build is active.
    let runs = transport
        .send(&nyedarch_github::endpoints::list_runs(&token, owner, repo))
        .map_err(|e| format!("could not check for running builds: {e}"))?;
    let body = String::from_utf8_lossy(&runs.body);
    if body.contains("\"status\":\"in_progress\"") || body.contains("\"status\":\"queued\"") {
        return Err(
            "a build is currently running in this repository. Changing visibility mid-build \
             could expose or hide files a running workflow is using, so it is refused until \
             the build finishes."
                .to_string(),
        );
    }

    let r = transport
        .send(&nyedarch_github::endpoints::set_visibility(&token, owner, repo, private))
        .map_err(|e| format!("visibility change failed: {e}"))?;
    if !(200..300).contains(&r.status) {
        let text = String::from_utf8_lossy(&r.body);
        return Err(format!("visibility change refused ({}): {}", r.status, text.chars().take(200).collect::<String>()));
    }
    Ok(())
}

/// Describe what a provenance failure means, in operator terms.
pub fn explain_provenance(e: &ProvenanceError) -> &'static str {
    match e {
        ProvenanceError::BuildIdMismatch => "the artifact belongs to a different build",
        ProvenanceError::TargetMismatch => "the artifact was built for a different target",
        ProvenanceError::DigestMismatch => "the artifact bytes are not what this build committed to",
        ProvenanceError::SignatureInvalid => "the artifact is not signed by the expected build channel",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_project(tag: &str) -> PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("nyedarch-remote-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src")).unwrap();
        std::fs::write(d.join("Cargo.toml"), b"[package]\nname=\"x\"").unwrap();
        std::fs::write(d.join("src/main.rs"), b"fn main(){}").unwrap();
        // Stand-in for the sealed package: this is the sensitive part.
        std::fs::write(d.join("capsule.nyeda"), b"SEALED-PACKAGE-MARKER").unwrap();
        d
    }

    /// Encryption must not depend on repository visibility.
    ///
    /// A public repository is the obvious case, but a private one leaks to every
    /// collaborator and to anyone later granted read access, so there is a
    /// single push path and no branch on `private`.
    #[test]
    fn the_push_is_encrypted_regardless_of_visibility() {
        let d = tmp_project("vis");
        let (files, key) = prepare_encrypted_push(&d).expect("prepare");

        // Only the ciphertext and the unlocker are pushed.
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"runtime/capsule.sealed"));
        assert!(names.iter().any(|n| n.starts_with("runtime/unlock/")));
        // Nothing from the capsule project itself. The unlocker has its own
        // `src/main.rs`, which is expected: it is the one file that is meant to
        // be readable, and it holds no secret.
        assert!(
            !names
                .iter()
                .any(|n| !n.starts_with("runtime/unlock/") && n != &"runtime/capsule.sealed"),
            "only the ciphertext and the unlocker may be pushed: {names:?}"
        );

        // The sealed package must not be recoverable from what is pushed.
        for (name, body) in &files {
            assert!(
                !body.windows(21).any(|w| w == b"SEALED-PACKAGE-MARKER"),
                "{name} contains the sealed package in the clear"
            );
        }

        // The key is real and is carried separately, not inside the push.
        assert_eq!(key.len(), 32);
        assert!(key.iter().any(|b| *b != 0));
        let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
        for (_, body) in &files {
            let text = String::from_utf8_lossy(body);
            assert!(!text.contains(&hex), "the source key must never be pushed");
        }
    }

    /// Two builds of the same project must not reuse a key.
    #[test]
    fn each_build_gets_a_fresh_source_key() {
        let d = tmp_project("fresh");
        let (_, k1) = prepare_encrypted_push(&d).unwrap();
        let (_, k2) = prepare_encrypted_push(&d).unwrap();
        assert_ne!(k1, k2, "reusing a source key across builds would link them");
    }
}
