//! Transport-agnostic HTTP model. The orchestrator builds `HttpRequest`s and
//! runs them through a `Transport`. This keeps all GitHub logic testable with a
//! mock and free of a network dependency in the core build (spec §38 — minimize
//! attack surface); the concrete reqwest transport lives behind the `net`
//! feature for on-machine use.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum GhError {
    #[error("transport error")]
    Transport,
    #[error("unexpected status {0}")]
    Status(u16),
    /// GitHub explains most refusals in the response body. Discarding that
    /// turns a fixable problem into a bare number, so it is carried through.
    #[error("{0}: {1}")]
    StatusDetail(u16, String),
    #[error("response decode error")]
    Decode,
    #[error("secret sealing unavailable")]
    Sealer,
    /// The build produced an artifact that does not carry the package this
    /// client committed to. The build environment is untrusted, so this is a
    /// refusal rather than a warning.
    #[error("artifact provenance check failed: it does not carry the committed package")]
    Provenance,
    #[error("the remote build failed (run {0}); see the workflow logs")]
    BuildFailed(u64),
    #[error("orchestration precondition failed: {0}")]
    Precondition(&'static str),
}

/// Pluggable HTTP transport. Real impl (reqwest) is provided as `net`-gated
/// source; tests use a mock.
pub trait Transport {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, GhError>;
}

/// GitHub requires repository secrets to be encrypted client-side with the
/// repo's public key using a libsodium sealed box. This trait isolates that
/// step; the production impl uses libsodium (crypto_box sealed box). A secret
/// must NEVER be sent unsealed (spec §46/§48).
pub trait SecretSealer {
    /// Seal `plaintext` for the repo public key (base64) -> base64 sealed value.
    fn seal(&self, repo_public_key_b64: &str, plaintext: &[u8]) -> Result<String, GhError>;
}
