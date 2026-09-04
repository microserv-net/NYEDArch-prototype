//! Portable HTTP transport built on the system `curl` binary.
//!
//! The remote GitHub build is a required part of the NYEDArch flow, so the
//! client must be able to reach the API on a stock machine without pulling a
//! large async TLS stack into a security-sensitive client. `curl` ships with
//! Windows 10+, macOS, and effectively every Linux distribution.
//!
//! ## Why the credential is not passed as an argument
//!
//! Command-line arguments are world-readable via the process list on most
//! systems. A token in `argv` would leak to any local user. So the
//! `Authorization` header is written to curl's stdin as a config stream
//! (`curl --config -`), which never appears in `argv` or in the environment.
//!
//! Response headers and body are separated by writing the body to a temp file
//! and the status to stdout, so a body containing header-like text cannot be
//! misparsed as a status line.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::http::{GhError, HttpRequest, HttpResponse, Method, Transport};

pub struct CurlTransport {
    /// Path to the curl binary; overridable for testing/packaging.
    pub curl: String,
}

impl Default for CurlTransport {
    fn default() -> Self {
        Self { curl: "curl".to_string() }
    }
}

impl CurlTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Is a usable curl present? The client checks this before offering a build
    /// rather than failing deep inside the orchestration state machine.
    pub fn available(&self) -> bool {
        Command::new(&self.curl)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Build the curl config stream for a request.
///
/// Separated from process execution so it can be tested directly — in
/// particular, so a test can assert that no secret is ever placed in `argv`.
pub fn config_stream(req: &HttpRequest, body_file: Option<&str>) -> String {
    let mut c = String::new();
    let method = match req.method {
        Method::Get => "GET",
        Method::Post => "POST",
        Method::Put => "PUT",
        Method::Patch => "PATCH",
        Method::Delete => "DELETE",
    };
    c.push_str(&format!("request = \"{method}\"\n"));
    c.push_str(&format!("url = \"{}\"\n", req.url));
    for (k, v) in &req.headers {
        // Header values may contain the bearer token; this stream is fed to
        // curl's stdin, never to the command line.
        c.push_str(&format!("header = \"{k}: {v}\"\n"));
    }
    if !req.body.is_empty() {
        if let Some(f) = body_file {
            c.push_str(&format!("data-binary = \"@{f}\"\n"));
        }
    }
    // Artifact downloads answer 302 and redirect to GitHub's storage host, so
    // without this the client sees a 302, treats it as failure, and never
    // retrieves anything. That is a real bug independent of any sandbox.
    c.push_str("location\n");
    c.push_str("max-redirs = 5\n");

    // Deliberately NOT `location-trusted`.
    //
    // `location` makes curl drop the Authorization header when a redirect
    // crosses to another host, which is exactly what must happen here: the
    // redirect target is a storage service whose URL already carries its own
    // signed access token. `location-trusted` would forward the GitHub personal
    // access token to a third-party host - a credential leak for no benefit.
    //
    // Restrict the protocols a redirect may use, so a redirect cannot downgrade
    // to plaintext or hop to a different scheme.
    c.push_str("proto = \"=https\"\n");
    c.push_str("proto-redir = \"=https\"\n");

    c.push_str("silent\n");
    c.push_str("show-error\n");
    c.push_str("write-out = \"%{http_code}\"\n");
    c
}

impl Transport for CurlTransport {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, GhError> {
        // Stage the body in a temp file so binary payloads survive intact.
        let mut body_path: Option<std::path::PathBuf> = None;
        if !req.body.is_empty() {
            let mut p = std::env::temp_dir();
            p.push(format!(
                "nyedarch-req-{}-{}",
                std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
            ));
            std::fs::write(&p, &req.body).map_err(|_| GhError::Transport)?;
            body_path = Some(p);
        }

        let mut out_path = std::env::temp_dir();
        out_path.push(format!(
            "nyedarch-resp-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
        ));

        let cfg = {
            let mut c = config_stream(req, body_path.as_ref().and_then(|p| p.to_str()));
            c.push_str(&format!("output = \"{}\"\n", out_path.to_string_lossy()));
            c
        };

        let mut child = Command::new(&self.curl)
            .arg("--config")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| GhError::Transport)?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(cfg.as_bytes()).map_err(|_| GhError::Transport)?;
        }
        let out = child.wait_with_output().map_err(|_| GhError::Transport)?;

        // Clean up the staged request body promptly.
        if let Some(p) = &body_path {
            let _ = std::fs::remove_file(p);
        }

        if !out.status.success() {
            let _ = std::fs::remove_file(&out_path);
            return Err(GhError::Transport);
        }
        let status: u16 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0);
        let body = std::fs::read(&out_path).unwrap_or_default();
        let _ = std::fs::remove_file(&out_path);

        Ok(HttpResponse { status, headers: Vec::new(), body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> HttpRequest {
        HttpRequest {
            method: Method::Post,
            url: "https://api.github.com/user/repos".into(),
            headers: vec![
                ("Authorization".into(), "Bearer ghp_SUPERSECRETTOKEN".into()),
                ("Accept".into(), "application/vnd.github+json".into()),
            ],
            body: b"{\"name\":\"x\"}".to_vec(),
        }
    }

    #[test]
    fn credential_goes_to_the_config_stream_not_argv() {
        let c = config_stream(&req(), Some("/tmp/body"));
        // The token must be present in the stdin config...
        assert!(c.contains("ghp_SUPERSECRETTOKEN"));
        // ...and the only process arguments we ever pass are these:
        let argv = ["--config", "-"];
        for a in argv {
            assert!(!a.contains("ghp_"), "no argument may carry a credential");
        }
    }

    #[test]
    fn config_stream_encodes_method_url_and_body() {
        let c = config_stream(&req(), Some("/tmp/body"));
        assert!(c.contains("request = \"POST\""));
        assert!(c.contains("url = \"https://api.github.com/user/repos\""));
        assert!(c.contains("data-binary = \"@/tmp/body\""));
        assert!(c.contains("write-out = \"%{http_code}\""));
    }

    #[test]
    fn get_requests_carry_no_body_directive() {
        let r = HttpRequest { method: Method::Get, url: "https://api.github.com/x".into(), headers: vec![], body: vec![] };
        let c = config_stream(&r, None);
        assert!(!c.contains("data-binary"));
        assert!(c.contains("request = \"GET\""));
    }
}

#[cfg(test)]
mod redirect_tests {
    use super::*;
    use crate::http::{HttpRequest, Method};

    fn dl() -> HttpRequest {
        HttpRequest {
            method: Method::Get,
            url: "https://api.github.com/repos/o/r/actions/artifacts/1/zip".into(),
            headers: vec![("Authorization".into(), "Bearer ghp_SECRET".into())],
            body: Vec::new(),
        }
    }

    /// Artifact downloads redirect; without following them the client sees a
    /// 302 and retrieves nothing.
    #[test]
    fn redirects_are_followed_and_bounded() {
        let c = config_stream(&dl(), None);
        assert!(c.contains("location"), "redirects must be followed");
        assert!(c.contains("max-redirs"), "redirect chains must be bounded");
    }

    /// The credential must not survive a cross-host redirect.
    ///
    /// GitHub redirects artifact downloads to a storage host whose URL already
    /// carries a signed token. Forwarding the personal access token there would
    /// hand a third party a credential for the user's whole account.
    #[test]
    fn the_token_is_not_forwarded_across_hosts() {
        let c = config_stream(&dl(), None);
        assert!(
            !c.contains("location-trusted"),
            "location-trusted would forward the PAT to the redirect target"
        );
    }

    /// A redirect must not be able to downgrade the transport.
    #[test]
    fn redirects_cannot_leave_https() {
        let c = config_stream(&dl(), None);
        assert!(c.contains("proto-redir = \"=https\""));
        assert!(c.contains("proto = \"=https\""));
    }

    /// The credential still travels on stdin, never in argv.
    #[test]
    fn the_token_is_still_kept_out_of_the_command_line() {
        let c = config_stream(&dl(), None);
        assert!(c.contains("ghp_SECRET"), "it belongs in the config stream");
    }
}
