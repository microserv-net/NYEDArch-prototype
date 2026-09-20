//! nyedarch-github — GitHub build orchestration (spec §42-§48). The client drives
//! GitHub as the remote build environment: repo lifecycle, encrypted secrets,
//! workflow + generated-runtime push, dispatch, run polling, and artifact
//! retrieval. All logic is transport-agnostic and unit-testable; the concrete
//! reqwest transport and libsodium sealed-box sealer are `net`-gated for
//! on-machine use with real credentials.
//!
//! Two independent directional keypairs (spec §42) are documented in
//! GITHUB_SECURITY.md: Client→GitHub (build-input authenticity) and
//! GitHub→Client (artifact confidentiality/integrity). They are never reused
//! across directions.

#![forbid(unsafe_code)]

pub mod base64;
pub mod curl_transport;
#[cfg(feature = "net")]
pub mod net;
#[cfg(feature = "sealer")]
pub mod sealer;
#[cfg(feature = "sealer")]
pub use sealer::SodiumSealer;
pub mod endpoints;
pub mod http;
pub mod orchestrator;
pub mod provenance;
pub mod workflow;

pub use http::{GhError, HttpRequest, HttpResponse, Method, SecretSealer, Transport};
pub use orchestrator::{BuildInputs, BuildState, Orchestrator, RepoConfig};
pub use curl_transport::CurlTransport;
pub use provenance::{verify_artifact, ArtifactClaim, ArtifactVerifier, BuildManifest, ProvenanceError};
pub use workflow::{workflow_yaml, Target};

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Mock transport: canned responses keyed by URL substring, recording calls.
    struct Mock {
        calls: RefCell<Vec<String>>,
    }
    impl Transport for Mock {
        fn send(&self, req: &HttpRequest) -> Result<HttpResponse, GhError> {
            self.calls.borrow_mut().push(format!("{:?} {}", req.method, req.url));
            let body = if req.url.contains("actions/secrets/public-key") {
                br#"{"key":"BASE64PUBKEY","key_id":"1234567890"}"#.to_vec()
            } else if req.url.contains("/actions/runs/") && !req.url.contains("/artifacts") {
                // A completed, successful run so the wait loop exits at once.
                br#"{"id":42,"status":"completed","conclusion":"success"}"#.to_vec()
            } else if req.url.ends_with("/actions/runs?per_page=30") {
                // Before dispatch the newest run is 41; afterwards it is 42, so
                // the client can tell a genuinely new run from a stale one.
                let dispatched = self
                    .calls
                    .borrow()
                    .iter()
                    .any(|c| c.contains("/dispatches"));
                if dispatched {
                    br#"{"total_count":2,"workflow_runs":[{"id":42,"status":"queued","run_number":2,"workflow_id":7}]}"#.to_vec()
                } else {
                    br#"{"total_count":1,"workflow_runs":[{"id":41,"status":"completed","run_number":1,"workflow_id":7}]}"#.to_vec()
                }
            } else if false {
                br#"{"total_count":1,"workflow_runs":[{"id":42,"status":"queued"}]}"#.to_vec()
            } else if req.url.ends_with("/zip") {
                // A stand-in capsule that carries the committed package, so the
                // provenance check has something real to accept.
                let mut art = b"MOCK-CAPSULE-BINARY".to_vec();
                art.extend_from_slice(&[0u8; 32]); // the commitment used below
                art.extend_from_slice(b"-TRAILER");
                art
            } else if req.url.contains("/artifacts") {
                br#"{"total_count":1,"artifacts":[{"id":7,"name":"nyedarch-capsule-x86_64-unknown-linux-gnu"}]}"#.to_vec()
            } else {
                b"{}".to_vec()
            };
            Ok(HttpResponse { status: 201, headers: vec![], body })
        }
    }

    /// Test-only sealer. NEVER used in production (real path = libsodium sealed
    /// box). Marked loudly so it cannot be mistaken for real sealing.
    struct TestSealer;
    impl SecretSealer for TestSealer {
        fn seal(&self, _pk: &str, plaintext: &[u8]) -> Result<String, GhError> {
            Ok(base64::encode(plaintext)) // NOT ENCRYPTION — test double only.
        }
    }

    #[test]
    fn full_build_sequence_is_ordered() {
        let m = Mock { calls: RefCell::new(vec![]) };
        let s = TestSealer;
        let mut orch = Orchestrator::new(&m, &s);
        // Never sleep in a test. The wait loop is real, and without this a unit
        // test blocks for twenty minutes - which is exactly what happened.
        orch.sleep = |_| {};
        orch.max_wait_polls = 3;
        let cfg = RepoConfig { token: "t".into(), owner: "o".into(), repo: "r".into(), private: true };
        let rt = b"fn main(){}".to_vec();
        let inputs = BuildInputs {
            runtime_files: vec![("runtime/src/main.rs".into(), rt.as_slice())],
            runtime_dir: "runtime".into(),
            targets: vec![Target::LinuxGnu],
            secrets: vec![("NYEDARCH_GH_TO_CLIENT_PUB".into(), b"pubkey".to_vec())],
            git_ref: "main".into(),
            package_commitment: [0u8; 32],
        };
        let mut states = vec![];
        let run_id = orch.run_build(&cfg, &inputs, |s| states.push(s)).unwrap();
        assert_eq!(run_id, 42);
        assert_eq!(states.first(), Some(&BuildState::Authenticate));
        assert_eq!(states.last(), Some(&BuildState::Done));
        // Secret public key fetched before any secret is written.
        let calls = m.calls.borrow();
        let pk = calls.iter().position(|c| c.contains("public-key")).unwrap();
        let put = calls.iter().position(|c| c.contains("/actions/secrets/NYEDARCH")).unwrap();
        assert!(pk < put, "must fetch repo public key before sealing/putting secret");
    }

    #[test]
    fn visibility_change_refused_during_build() {
        let m = Mock { calls: RefCell::new(vec![]) };
        let s = TestSealer;
        let mut orch = Orchestrator::new(&m, &s);
        // Never sleep in a test. The wait loop is real, and without this a unit
        // test blocks for twenty minutes - which is exactly what happened.
        orch.sleep = |_| {};
        orch.max_wait_polls = 3;
        orch.build_running = true;
        let cfg = RepoConfig { token: "t".into(), owner: "o".into(), repo: "r".into(), private: true };
        assert!(matches!(orch.change_visibility(&cfg, false), Err(GhError::Precondition(_))));
    }

    #[test]
    fn workflow_yaml_has_dispatch_and_targets() {
        let y = workflow_yaml(&[Target::WindowsMsvc, Target::MacosAppleSilicon, Target::LinuxGnu], "runtime");
        assert!(y.contains("workflow_dispatch"));
        assert!(y.contains("x86_64-pc-windows-msvc"));
        assert!(y.contains("aarch64-apple-darwin"));
        assert!(y.contains("x86_64-unknown-linux-gnu"));
        // No secret is ever echoed.
        assert!(!y.to_lowercase().contains("echo ${{ secrets"));
        // Capsules come back with the NYEDArch extension, never a raw binary
        // name, and with the executable bit requested.
        assert!(y.contains(".nyarch"), "artifact must carry the .nyarch extension");
        assert!(y.contains("chmod +x"), "executable permission must be requested");
        // The source must arrive encrypted and be decrypted only on the runner.
        assert!(y.contains("nyedarch-unlock"), "the workflow must decrypt the source");
        assert!(y.contains("secrets.NYEDARCH_SOURCE_KEY"));
        assert!(y.contains("Remove the decrypted source"), "plaintext must not outlive the build");
        // The key must never be echoed.
        assert!(!y.contains("echo $NYEDARCH_SOURCE_KEY"));
        assert!(!y.contains("echo \"$NYEDARCH_SOURCE_KEY\""));
        assert!(!y.contains("path: out/*\n"), "must not upload unrelated files");
    }
}
