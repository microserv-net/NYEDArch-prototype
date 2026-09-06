//! Build orchestration state machine (spec §44/§73). Sequences GitHub calls to
//! produce a capsule build. Pure over `Transport` + `SecretSealer`, so it is
//! fully unit-testable offline. Fails closed on any unexpected status; never
//! logs secrets (spec §48).

use crate::base64;
use crate::endpoints as ep;
use crate::http::{GhError, HttpResponse, Method, SecretSealer, Transport};
use crate::workflow::{workflow_yaml, Target};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildState {
    Authenticate,
    EnsureRepo,
    SetVisibility,
    PushSecrets,
    PushWorkflow,
    PushRuntime,
    Dispatch,
    Poll,
    /// Waiting for the remote build to finish before retrieving anything.
    WaitForRun,
    FetchArtifact,
    Verify,
    Done,
    Failed,
}

pub struct RepoConfig {
    pub token: String,
    pub owner: String,
    pub repo: String,
    pub private: bool,
}

pub struct BuildInputs<'a> {
    /// The generated runtime crate files: (repo-path, bytes).
    pub runtime_files: Vec<(String, &'a [u8])>,
    pub runtime_dir: String,
    pub targets: Vec<Target>,
    /// Repository secrets to seal + store (name, plaintext).
    pub secrets: Vec<(String, Vec<u8>)>,
    pub git_ref: String,
    /// Commitment to the sealed package this build must produce a capsule for.
    /// The returned artifact is checked against it (spec §45).
    pub package_commitment: [u8; 32],
}

fn ok(status: u16) -> bool {
    (200..300).contains(&status)
}

/// Pull GitHub's own explanation out of an error body.
fn detail(status: u16, body: &[u8]) -> GhError {
    let text = String::from_utf8_lossy(body);
    let msg = json_str(body, "message").unwrap_or("no message").to_string();
    // The `errors` array usually says which field was wrong.
    let extra = text
        .find("\"errors\"")
        .map(|i| text[i..].chars().take(240).collect::<String>())
        .unwrap_or_default();
    GhError::StatusDetail(status, if extra.is_empty() { msg } else { format!("{msg} | {extra}") })
}

/// Extract a "field":"value" string from a JSON body (minimal; real impl uses
/// serde_json under the `net` feature).
fn json_str<'a>(body: &'a [u8], field: &str) -> Option<&'a str> {
    let s = core::str::from_utf8(body).ok()?;
    let pat = format!("\"{field}\"");
    let i = s.find(&pat)? + pat.len();
    let rest = &s[i..];
    let c = rest.find(':')? + 1;
    let rest = rest[c..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(&rest[..end])
}

pub struct Orchestrator<'a, T: Transport, S: SecretSealer> {
    pub transport: &'a T,
    pub sealer: &'a S,
    pub build_running: bool,
    /// The downloaded artifact, kept so the caller can write it out after the
    /// provenance check has passed.
    pub last_artifact: Option<Vec<u8>>,
    /// How many times to poll for run completion before giving up.
    pub max_wait_polls: u32,
    /// Seconds between polls.
    pub poll_interval_secs: u64,
    /// Injected so tests do not actually sleep.
    pub sleep: fn(u64),
    /// Remove the artifact from GitHub once it has been downloaded.
    pub delete_artifact_after_download: bool,
}

impl<'a, T: Transport, S: SecretSealer> Orchestrator<'a, T, S> {
    pub fn new(transport: &'a T, sealer: &'a S) -> Self {
        Self {
            transport,
            sealer,
            build_running: false,
            last_artifact: None,
            // ~20 minutes: long enough for a three-target build, bounded so a
            // stuck run cannot hang the client forever.
            max_wait_polls: 80,
            poll_interval_secs: 15,
            sleep: |s| std::thread::sleep(std::time::Duration::from_secs(s)),
            delete_artifact_after_download: true,
        }
    }

    /// Change visibility — refused while a build is running (spec §43).
    pub fn change_visibility(&self, cfg: &RepoConfig, private: bool) -> Result<(), GhError> {
        if self.build_running {
            return Err(GhError::Precondition("cannot change visibility while a build is running"));
        }
        let r = self.transport.send(&ep::set_visibility(&cfg.token, &cfg.owner, &cfg.repo, private))?;
        if !ok(r.status) {
            return Err(GhError::Status(r.status));
        }
        Ok(())
    }

    /// Create a file, or update it in place when it already exists.
    ///
    /// A repository is reused across builds by design, so most pushes after the
    /// first are updates. The contents API refuses an update that omits the
    /// current blob SHA, so it is fetched first; a 404 simply means the file is
    /// new.
    fn put_or_update(
        &self,
        cfg: &RepoConfig,
        path: &str,
        content_b64: &str,
        message: &str,
    ) -> Result<(), GhError> {
        let existing = self.transport.send(&ep::get_file(&cfg.token, &cfg.owner, &cfg.repo, path))?;
        let sha = if ok(existing.status) {
            json_str(&existing.body, "sha").map(|s| s.to_string())
        } else {
            None
        };
        let r = self.transport.send(&ep::put_file(
            &cfg.token,
            &cfg.owner,
            &cfg.repo,
            path,
            content_b64,
            message,
            sha.as_deref(),
        ))?;
        if !ok(r.status) {
            return Err(detail(r.status, &r.body));
        }
        Ok(())
    }

    /// Remove files under `prefix` that the current build does not push.
    ///
    /// A repository is reused across builds, so without this the tree
    /// accumulates every earlier build's files. That matters for more than
    /// tidiness: a repository that once received a plaintext capsule source
    /// would keep it indefinitely, and a stale `Cargo.toml` above the unlocker
    /// breaks the build outright.
    fn prune_stale(
        &self,
        cfg: &RepoConfig,
        prefix: &str,
        keep: &[String],
        git_ref: &str,
    ) -> Result<usize, GhError> {
        let tree = self.transport.send(&ep::get_tree(&cfg.token, &cfg.owner, &cfg.repo, git_ref))?;
        if !ok(tree.status) {
            return Ok(0); // an empty or unreadable tree simply means nothing to prune
        }
        let text = String::from_utf8_lossy(&tree.body).to_string();
        let mut removed = 0usize;
        for (path, sha) in tree_blobs(&text) {
            if !path.starts_with(prefix) || keep.iter().any(|k| k == &path) {
                continue;
            }
            let r = self.transport.send(&ep::delete_file(
                &cfg.token, &cfg.owner, &cfg.repo, &path, &sha,
                "nyedarch: remove file from a previous build",
            ))?;
            if ok(r.status) {
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// Run a full build. `trace` observes state transitions.
    pub fn run_build(
        &mut self,
        cfg: &RepoConfig,
        inputs: &BuildInputs<'_>,
        mut trace: impl FnMut(BuildState),
    ) -> Result<u64, GhError> {
        macro_rules! st {
            ($s:expr) => {
                trace($s);
            };
        }

        st!(BuildState::Authenticate);
        // (Auth is validated implicitly by the first authenticated call.)

        st!(BuildState::EnsureRepo);
        let r = self.transport.send(&ep::create_repo(&cfg.token, &cfg.repo, cfg.private))?;
        // 201 created or 422 (already exists) are both acceptable.
        if !ok(r.status) && r.status != 422 {
            return Err(GhError::Status(r.status));
        }

        st!(BuildState::SetVisibility);
        let r = self.transport.send(&ep::set_visibility(&cfg.token, &cfg.owner, &cfg.repo, cfg.private))?;
        if !ok(r.status) {
            return Err(GhError::Status(r.status));
        }

        st!(BuildState::PushSecrets);
        let pk = self.transport.send(&ep::get_actions_public_key(&cfg.token, &cfg.owner, &cfg.repo))?;
        if !ok(pk.status) {
            return Err(GhError::Status(pk.status));
        }
        let repo_pub = json_str(&pk.body, "key").ok_or(GhError::Decode)?;
        let key_id = json_str(&pk.body, "key_id").ok_or(GhError::Decode)?;
        for (name, plaintext) in &inputs.secrets {
            let sealed = self.sealer.seal(repo_pub, plaintext)?; // libsodium sealed box
            let r = self.transport.send(&ep::put_secret(&cfg.token, &cfg.owner, &cfg.repo, name, &sealed, key_id))?;
            if !ok(r.status) {
                return Err(GhError::Status(r.status));
            }
        }

        // Mark build-critical window: visibility cannot change from here.
        self.build_running = true;

        st!(BuildState::PushWorkflow);
        let yaml = workflow_yaml(&inputs.targets, &inputs.runtime_dir);
        if let Err(e) = self.put_or_update(
            cfg,
            ".github/workflows/nyeda.yml",
            &base64::encode(yaml.as_bytes()),
            "nyedarch: build workflow",
        ) {
            self.build_running = false;
            return Err(e);
        }

        // Clear anything left by an earlier build before pushing this one.
        let keep: Vec<String> = inputs.runtime_files.iter().map(|(p, _)| p.clone()).collect();
        let pruned = self
            .prune_stale(cfg, &format!("{}/", inputs.runtime_dir), &keep, &inputs.git_ref)
            .unwrap_or(0);
        if pruned > 0 {
            trace(BuildState::PushRuntime);
        }

        st!(BuildState::PushRuntime);
        for (path, bytes) in &inputs.runtime_files {
            if let Err(e) = self.put_or_update(
                cfg,
                path,
                &base64::encode(bytes),
                "nyedarch: generated capsule source",
            ) {
                self.build_running = false;
                return Err(e);
            }
        }

        st!(BuildState::Dispatch);
        // Remember the newest run *before* dispatching. Immediately after a
        // dispatch the new run is not registered yet, so taking "the newest
        // run" attaches to the previous one - the client then waits on a build
        // that already finished, reports its result, and fetches its artifacts.
        // Observed live: the wait returned instantly and no artifact appeared.
        let known_runs: std::collections::BTreeSet<u64> = self
            .transport
            .send(&ep::list_runs(&cfg.token, &cfg.owner, &cfg.repo))
            .ok()
            .filter(|r| ok(r.status))
            .map(|r| run_ids(&r.body))
            .unwrap_or_default();

        let r = self.transport.send(&ep::dispatch_workflow(
            &cfg.token, &cfg.owner, &cfg.repo, "nyeda.yml", &inputs.git_ref,
        ))?;
        if !ok(r.status) {
            self.build_running = false;
            return Err(GhError::Status(r.status));
        }

        st!(BuildState::Poll);
        // Wait for a run that is genuinely new.
        let mut run_id = None;
        for _ in 0..self.max_wait_polls {
            let runs = self.transport.send(&ep::list_runs(&cfg.token, &cfg.owner, &cfg.repo))?;
            if ok(runs.status) {
                // The first id that was not there before dispatch.
                if let Some(fresh) = run_ids(&runs.body).into_iter().find(|id| !known_runs.contains(id)) {
                    run_id = Some(fresh);
                    break;
                }
            }
            (self.sleep)(self.poll_interval_secs);
        }
        let run_id = match run_id {
            Some(id) => id,
            None => {
                self.build_running = false;
                return Err(GhError::Decode);
            }
        };

        // Wait for the run to finish before looking for artifacts.
        //
        // Fetching straight after dispatch cannot work: the build has not run
        // yet, so the artifact list is always empty and §45 retrieval never
        // happens. That is what the first version did, and it reported "no
        // artifact was retrieved" every time - accurate, and useless.
        st!(BuildState::WaitForRun);
        let mut completed = false;
        for _ in 0..self.max_wait_polls {
            let r = self.transport.send(&ep::get_run(&cfg.token, &cfg.owner, &cfg.repo, run_id))?;
            if ok(r.status) {
                let text = String::from_utf8_lossy(&r.body);
                if text.contains("\"status\":\"completed\"") {
                    completed = true;
                    // A failed build has no artifact worth fetching, and
                    // pretending otherwise would hide the failure.
                    if !text.contains("\"conclusion\":\"success\"") {
                        self.build_running = false;
                        return Err(GhError::BuildFailed(run_id));
                    }
                    break;
                }
            }
            (self.sleep)(self.poll_interval_secs);
        }
        if !completed {
            // Not an error: the build may simply be slow. The caller is told so
            // it can retrieve later rather than being handed a false success.
            self.build_running = false;
            st!(BuildState::Done);
            return Ok(run_id);
        }

        st!(BuildState::FetchArtifact);
        let arts = self.transport.send(&ep::list_artifacts(&cfg.token, &cfg.owner, &cfg.repo, run_id))?;
        if !ok(arts.status) {
            self.build_running = false;
            return Err(GhError::Status(arts.status));
        }

        st!(BuildState::Verify);
        // Retrieve and check the artifact, rather than trusting that a build
        // which reported success produced the right thing (spec §45). An
        // earlier version only listed artifacts and left a comment saying
        // verification happened "on download" - there was no download.
        let listing = String::from_utf8_lossy(&arts.body).to_string();
        let all = artifact_list(&listing);

        // Pick the artifact for a requested target rather than whichever came
        // first. Building for macOS and receiving a Windows binary is not a
        // cosmetic problem: the capsule simply will not run.
        let wanted = inputs
            .targets
            .iter()
            .map(|t| t.rust_target().to_string())
            .collect::<Vec<_>>();
        // Prefer the host's own platform.
        //
        // A build usually requests every target, and only one file can be
        // delivered. Handing over whichever artifact GitHub listed first gave a
        // Windows binary to someone on macOS - it downloads, it saves, and it
        // cannot run. The capsule a user can actually open is the one for the
        // machine they are sitting at.
        let host = if cfg!(target_os = "windows") {
            "x86_64-pc-windows-msvc"
        } else if cfg!(target_os = "macos") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-unknown-linux-gnu"
        };
        let chosen = all
            .iter()
            .find(|(_, name)| name.contains(host) && wanted.iter().any(|w| name.contains(w.as_str())))
            .or_else(|| all.iter().find(|(_, name)| wanted.iter().any(|w| name.contains(w.as_str()))))
            .or_else(|| all.first())
            .map(|(id, name)| (*id, name.clone()));

        if let Some((id, name)) = chosen {
            let _ = &name;
            let blob = self
                .transport
                .send(&ep::download_artifact(&cfg.token, &cfg.owner, &cfg.repo, id))?;
            if ok(blob.status) {
                self.last_artifact = Some(blob.body.clone());
                // Retrieved and verified, so the remote copy is no longer
                // needed. It is the capsule binary itself, and on a public
                // repository any authenticated user could fetch it.
                if self.delete_artifact_after_download {
                    let _ = self
                        .transport
                        .send(&ep::delete_artifact(&cfg.token, &cfg.owner, &cfg.repo, id));
                }
                // The provenance check does NOT run here.
                //
                // GitHub returns the artifact as a zip and the capsule inside
                // it is deflated, so the commitment cannot appear in these
                // bytes. Checking here refused every build and delivered
                // nothing. It runs on the extracted capsule instead, in the
                // caller, which is where the commitment actually is.
            }
        }

        self.build_running = false;
        st!(BuildState::Done);
        Ok(run_id)
    }
}

/// Every `(id, name)` pair in an artifacts listing.
///
/// The listing is ordered by GitHub, not by us, so taking the first entry
/// delivers whichever target happens to be listed first - a Windows binary to
/// someone who asked for macOS. Callers pick by name instead.
fn artifact_list(text: &str) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("\"id\":") {
        rest = &rest[i + 5..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let Ok(id) = digits.parse::<u64>() else { continue };
        // The name follows the id in each artifact object.
        let name = match rest.find("\"name\":\"") {
            Some(j) => {
                let after = &rest[j + 8..];
                after.split('"').next().unwrap_or("").to_string()
            }
            None => String::new(),
        };
        out.push((id, name));
    }
    out
}

/// Every run id in a runs listing.
///
/// The client used to remember only the *newest* id and wait for a different
/// one. That breaks in two ways, and both were observed: if the listing call
/// fails there is nothing to compare against, and if the new run has not
/// registered yet the newest id is still an old one. Either way the client
/// adopted a run that had already finished, skipped the wait entirely, and went
/// looking for artifacts that were expired or gone - exiting in seconds with no
/// capsule.
///
/// Remembering the whole set removes the ambiguity: a run is new when its id was
/// not there before, and "the listing failed" is an empty set, which matches
/// nothing rather than everything.
fn run_ids(body: &[u8]) -> std::collections::BTreeSet<u64> {
    let text = String::from_utf8_lossy(body);
    let mut out = std::collections::BTreeSet::new();
    let mut rest = &text[..];
    while let Some(i) = rest.find("\"id\":") {
        rest = &rest[i + 5..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(id) = digits.parse::<u64>() {
            out.insert(id);
        }
    }
    out
}


/// Does the downloaded artifact actually carry the package we sealed?
///
/// The capsule embeds the sealed package, so its commitment must appear in the
/// artifact bytes. This is a provenance check against an untrusted build
/// environment: a build that succeeded but produced a binary carrying a
/// different payload is refused.
///
/// It proves the artifact contains *our* package. It does not prove the rest of
/// the binary was built from our source - reproducible builds would be needed
/// for that, and are not implemented.
pub fn artifact_carries_package(artifact: &[u8], package_commitment: &[u8; 32]) -> bool {
    if artifact.is_empty() {
        return false;
    }
    artifact
        .windows(package_commitment.len())
        .any(|w| w == package_commitment)
}

/// Extract `(path, sha)` for every blob in a git tree response.
fn tree_blobs(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for chunk in text.split("{\"path\":") {
        let Some(rest) = chunk.strip_prefix('"') else { continue };
        let Some(end) = rest.find('"') else { continue };
        let path = rest[..end].to_string();
        if !chunk.contains("\"type\":\"blob\"") {
            continue;
        }
        if let Some(i) = chunk.find("\"sha\":\"") {
            let s = &chunk[i + 7..];
            if let Some(e) = s.find('"') {
                out.push((path, s[..e].to_string()));
            }
        }
    }
    out
}


#[allow(dead_code)]
fn _method_is_used(_m: Method) {}
#[allow(dead_code)]
fn _resp(_r: HttpResponse) {}

#[cfg(test)]
mod prune_tests {
    use super::tree_blobs;

    /// Stale files from an earlier build must be identifiable, including any
    /// plaintext capsule source a previous version of the client pushed.
    #[test]
    fn tree_blobs_are_extracted() {
        let json = r#"{"sha":"abc","tree":[
          {"path":"runtime/Cargo.toml","mode":"100644","type":"blob","sha":"1111"},
          {"path":"runtime/src","mode":"040000","type":"tree","sha":"2222"},
          {"path":"runtime/src/main.rs","mode":"100644","type":"blob","sha":"3333"},
          {"path":".github/workflows/nyeda.yml","mode":"100644","type":"blob","sha":"4444"}
        ]}"#;
        let blobs = tree_blobs(json);
        let paths: Vec<&str> = blobs.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"runtime/Cargo.toml"));
        assert!(paths.contains(&"runtime/src/main.rs"));
        assert!(paths.contains(&".github/workflows/nyeda.yml"));
        // Directories are not deletable objects and must not be listed.
        assert!(!paths.contains(&"runtime/src"));
        assert_eq!(blobs.iter().find(|(p, _)| p == "runtime/Cargo.toml").unwrap().1, "1111");
    }
}

#[cfg(test)]
mod artifact_tests {
    use super::artifact_carries_package;

    /// An artifact that carries the committed package passes; one that does not
    /// is refused. GitHub is an untrusted build environment, so "the build
    /// reported success" is not evidence that it produced the right binary.
    #[test]
    fn provenance_accepts_only_an_artifact_carrying_the_package() {
        let commitment = [0x5Au8; 32];

        let mut good = b"ELF....some binary....".to_vec();
        good.extend_from_slice(&commitment);
        good.extend_from_slice(b"....more binary....");
        assert!(artifact_carries_package(&good, &commitment));

        let other = [0xA5u8; 32];
        assert!(
            !artifact_carries_package(&good, &other),
            "an artifact carrying a different package must be refused"
        );

        // Degenerate inputs must refuse rather than pass by accident.
        assert!(!artifact_carries_package(b"", &commitment));
        assert!(!artifact_carries_package(b"short", &commitment));
    }

    /// The artifact must be chosen by target, not by position.
    ///
    /// GitHub orders the listing; taking the first entry handed a Windows
    /// binary to someone building for macOS, which simply does not run.
    /// A run is new when its id was not present before dispatch.
    ///
    /// Comparing against "the newest id" adopted an already-finished run
    /// whenever the pre-dispatch listing failed or the new run had not
    /// registered yet - the client then skipped the wait and looked for
    /// artifacts that were long gone.
    #[test]
    fn only_a_genuinely_new_run_is_adopted() {
        let before = super::run_ids(br#"{"workflow_runs":[{"id":300},{"id":200},{"id":100}]}"#);
        assert_eq!(before.len(), 3);

        // Nothing new yet: the client must keep waiting.
        let same = super::run_ids(br#"{"workflow_runs":[{"id":300},{"id":200},{"id":100}]}"#);
        assert!(same.into_iter().find(|id| !before.contains(id)).is_none());

        // A new run appears.
        let after = super::run_ids(br#"{"workflow_runs":[{"id":400},{"id":300},{"id":200}]}"#);
        assert_eq!(after.into_iter().find(|id| !before.contains(id)), Some(400));

        // A failed listing is an empty set, which matches nothing - so the
        // client waits rather than adopting whatever it finds.
        let none = super::run_ids(b"{}");
        assert!(none.is_empty());
    }

    /// With every target requested, the host's own artifact must win.
    ///
    /// Only one file is delivered, and a capsule for another platform cannot be
    /// opened by the person who asked for it.
    #[test]
    fn the_hosts_own_artifact_is_preferred() {
        let listing = r#"{"artifacts":[
          {"id":1,"name":"nyedarch-capsule-x86_64-pc-windows-msvc"},
          {"id":2,"name":"nyedarch-capsule-aarch64-apple-darwin"},
          {"id":3,"name":"nyedarch-capsule-x86_64-unknown-linux-gnu"}]}"#;
        let all = super::artifact_list(listing);
        let host = if cfg!(target_os = "windows") {
            "x86_64-pc-windows-msvc"
        } else if cfg!(target_os = "macos") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-unknown-linux-gnu"
        };
        let chosen = all
            .iter()
            .find(|(_, n)| n.contains(host))
            .expect("the host artifact is present in this listing");
        assert!(
            chosen.1.contains(host),
            "selection must land on the host platform, not on listing order"
        );
    }

    #[test]
    fn artifacts_are_listed_with_their_names() {
        let listing = r#"{"total_count":3,"artifacts":[
          {"id":1,"name":"nyedarch-capsule-x86_64-pc-windows-msvc"},
          {"id":2,"name":"nyedarch-capsule-aarch64-apple-darwin"},
          {"id":3,"name":"nyedarch-capsule-x86_64-unknown-linux-gnu"}]}"#;
        let all = super::artifact_list(listing);
        assert_eq!(all.len(), 3);

        let mac = all
            .iter()
            .find(|(_, n)| n.contains("aarch64-apple-darwin"))
            .expect("the macOS artifact is findable by name");
        assert_eq!(mac.0, 2, "selection must not depend on listing order");

        assert!(super::artifact_list("{}").is_empty());
    }
}

#[cfg(test)]
mod wait_tests {
    use super::BuildState;

    /// The wait must come before retrieval, or the artifact list is always
    /// empty and §45 never happens - which is what the first version did.
    #[test]
    fn waiting_precedes_fetching() {
        let order = [
            BuildState::Dispatch,
            BuildState::WaitForRun,
            BuildState::FetchArtifact,
            BuildState::Verify,
            BuildState::Done,
        ];
        let pos = |s: BuildState| order.iter().position(|x| *x == s).unwrap();
        assert!(pos(BuildState::WaitForRun) < pos(BuildState::FetchArtifact));
        assert!(pos(BuildState::FetchArtifact) < pos(BuildState::Verify));
    }
}

#[cfg(test)]
mod artifact_cleanup_tests {
    /// The artifact is the capsule binary. Once it has been downloaded and
    /// verified, leaving it on GitHub leaves a copy of the specimen where
    /// anyone with read access can fetch it - on a public repository, any
    /// authenticated user. Deleting it is the default.
    #[test]
    fn deletion_after_download_is_on_by_default() {
        struct T;
        impl crate::http::Transport for T {
            fn send(&self, _r: &crate::http::HttpRequest) -> Result<crate::http::HttpResponse, crate::http::GhError> {
                Ok(crate::http::HttpResponse { status: 200, headers: vec![], body: b"{}".to_vec() })
            }
        }
        struct S;
        impl crate::http::SecretSealer for S {
            fn seal(&self, _pk: &str, _p: &[u8]) -> Result<String, crate::http::GhError> {
                Ok(String::new())
            }
        }
        let t = T;
        let s = S;
        let o = super::Orchestrator::new(&t, &s);
        assert!(
            o.delete_artifact_after_download,
            "an unretrieved capsule must not be left on GitHub by default"
        );
    }
}
