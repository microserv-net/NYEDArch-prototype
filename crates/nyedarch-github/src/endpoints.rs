//! GitHub REST + Git Data API request builders (spec §42-§48). Pure functions:
//! they construct requests; they never hold secrets in logs (spec §48).

use crate::http::{HttpRequest, Method};

const API: &str = "https://api.github.com";
const UA: &str = "nyeda-builder";

fn auth_headers(token: &str) -> Vec<(String, String)> {
    vec![
        ("Authorization".into(), format!("Bearer {token}")),
        ("Accept".into(), "application/vnd.github+json".into()),
        ("User-Agent".into(), UA.into()),
        ("X-GitHub-Api-Version".into(), "2022-11-28".into()),
    ]
}

pub fn create_repo(token: &str, name: &str, private: bool) -> HttpRequest {
    let body = format!(
        r#"{{"name":"{name}","private":{private},"auto_init":true,"description":"NYEDArch capsule build (generated)"}}"#
    );
    HttpRequest { method: Method::Post, url: format!("{API}/user/repos"), headers: auth_headers(token), body: body.into_bytes() }
}

/// Change repository visibility — only permitted when no build is running
/// (spec §43); the orchestrator enforces that precondition.
pub fn set_visibility(token: &str, owner: &str, repo: &str, private: bool) -> HttpRequest {
    let body = format!(r#"{{"private":{private},"visibility":"{}"}}"#, if private { "private" } else { "public" });
    HttpRequest { method: Method::Patch, url: format!("{API}/repos/{owner}/{repo}"), headers: auth_headers(token), body: body.into_bytes() }
}

pub fn get_actions_public_key(token: &str, owner: &str, repo: &str) -> HttpRequest {
    HttpRequest { method: Method::Get, url: format!("{API}/repos/{owner}/{repo}/actions/secrets/public-key"), headers: auth_headers(token), body: Vec::new() }
}

/// Store a repository secret. `sealed_value` must already be a libsodium sealed
/// box (see `SecretSealer`) — never plaintext (spec §48).
pub fn put_secret(token: &str, owner: &str, repo: &str, name: &str, sealed_value_b64: &str, key_id: &str) -> HttpRequest {
    let body = format!(r#"{{"encrypted_value":"{sealed_value_b64}","key_id":"{key_id}"}}"#);
    HttpRequest { method: Method::Put, url: format!("{API}/repos/{owner}/{repo}/actions/secrets/{name}"), headers: auth_headers(token), body: body.into_bytes() }
}

/// Create/update a file (workflow YAML or generated runtime source) via the
/// contents API. `content_b64` is base64 of the file bytes.
pub fn put_file(token: &str, owner: &str, repo: &str, path: &str, content_b64: &str, message: &str, sha: Option<&str>) -> HttpRequest {
    let sha_field = sha.map(|s| format!(r#","sha":"{s}""#)).unwrap_or_default();
    let body = format!(r#"{{"message":"{message}","content":"{content_b64}"{sha_field}}}"#);
    HttpRequest { method: Method::Put, url: format!("{API}/repos/{owner}/{repo}/contents/{path}"), headers: auth_headers(token), body: body.into_bytes() }
}

/// Fetch a file's metadata so an update can supply its blob SHA.
///
/// The contents API rejects an update that does not carry the current SHA, so
/// without this every build after the first fails with 422 - and one repository
/// holding many builds is the intended model.
pub fn get_file(token: &str, owner: &str, repo: &str, path: &str) -> HttpRequest {
    HttpRequest {
        method: Method::Get,
        url: format!("{API}/repos/{owner}/{repo}/contents/{path}"),
        headers: auth_headers(token),
        body: Vec::new(),
    }
}

/// List the whole tree of a branch, so stale files can be found.
pub fn get_tree(token: &str, owner: &str, repo: &str, branch: &str) -> HttpRequest {
    HttpRequest {
        method: Method::Get,
        url: format!("{API}/repos/{owner}/{repo}/git/trees/{branch}?recursive=1"),
        headers: auth_headers(token),
        body: Vec::new(),
    }
}

/// Delete a file. Used to remove build outputs left by an earlier build.
pub fn delete_file(token: &str, owner: &str, repo: &str, path: &str, sha: &str, message: &str) -> HttpRequest {
    let body = format!(r#"{{"message":"{message}","sha":"{sha}"}}"#);
    HttpRequest {
        method: Method::Delete,
        url: format!("{API}/repos/{owner}/{repo}/contents/{path}"),
        headers: auth_headers(token),
        body: body.into_bytes(),
    }
}

/// Delete an artifact once it has been retrieved and verified.
///
/// The artifact *is* the capsule binary. Leaving it on GitHub keeps a copy of
/// the specimen where anyone with read access can fetch it - and on a public
/// repository that is any authenticated user. Removing it after retrieval
/// shrinks the exposure window to the length of the build, and keeps Actions
/// storage from filling with capsules.
pub fn delete_artifact(token: &str, owner: &str, repo: &str, artifact_id: u64) -> HttpRequest {
    HttpRequest {
        method: Method::Delete,
        url: format!("{API}/repos/{owner}/{repo}/actions/artifacts/{artifact_id}"),
        headers: auth_headers(token),
        body: Vec::new(),
    }
}

pub fn dispatch_workflow(token: &str, owner: &str, repo: &str, workflow_file: &str, git_ref: &str) -> HttpRequest {
    let body = format!(r#"{{"ref":"{git_ref}"}}"#);
    HttpRequest {
        method: Method::Post,
        url: format!("{API}/repos/{owner}/{repo}/actions/workflows/{workflow_file}/dispatches"),
        headers: auth_headers(token),
        body: body.into_bytes(),
    }
}

pub fn list_runs(token: &str, owner: &str, repo: &str) -> HttpRequest {
    HttpRequest { method: Method::Get, url: format!("{API}/repos/{owner}/{repo}/actions/runs?per_page=5"), headers: auth_headers(token), body: Vec::new() }
}

pub fn run_logs(token: &str, owner: &str, repo: &str, run_id: u64) -> HttpRequest {
    HttpRequest { method: Method::Get, url: format!("{API}/repos/{owner}/{repo}/actions/runs/{run_id}/logs"), headers: auth_headers(token), body: Vec::new() }
}

/// Status of a single workflow run, for waiting on completion.
/// The account a token belongs to.
pub fn authenticated_user(token: &str) -> HttpRequest {
    HttpRequest {
        method: Method::Get,
        url: format!("{API}/user"),
        headers: auth_headers(token),
        body: Vec::new(),
    }
}

pub fn get_run(token: &str, owner: &str, repo: &str, run_id: u64) -> HttpRequest {
    HttpRequest {
        method: Method::Get,
        url: format!("{API}/repos/{owner}/{repo}/actions/runs/{run_id}"),
        headers: auth_headers(token),
        body: Vec::new(),
    }
}

pub fn list_artifacts(token: &str, owner: &str, repo: &str, run_id: u64) -> HttpRequest {
    HttpRequest { method: Method::Get, url: format!("{API}/repos/{owner}/{repo}/actions/runs/{run_id}/artifacts"), headers: auth_headers(token), body: Vec::new() }
}

pub fn download_artifact(token: &str, owner: &str, repo: &str, artifact_id: u64) -> HttpRequest {
    HttpRequest { method: Method::Get, url: format!("{API}/repos/{owner}/{repo}/actions/artifacts/{artifact_id}/zip"), headers: auth_headers(token), body: Vec::new() }
}
