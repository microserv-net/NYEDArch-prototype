//! Concrete on-machine transport + secret sealer (spec §42/§48). Compiled only
//! with `--features net`, on your desktop with real GitHub credentials. Kept out
//! of the default build so the core has zero network attack surface (spec §38).

#![cfg(feature = "net")]

use crate::http::{GhError, HttpRequest, HttpResponse, Method, SecretSealer, Transport};

/// Blocking reqwest transport.
pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub fn new() -> Self {
        Self { client: reqwest::blocking::Client::new() }
    }
}

impl Transport for ReqwestTransport {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, GhError> {
        let method = match req.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Patch => reqwest::Method::PATCH,
            Method::Delete => reqwest::Method::DELETE,
        };
        let mut rb = self.client.request(method, &req.url);
        for (k, v) in &req.headers {
            rb = rb.header(k, v);
        }
        if !req.body.is_empty() {
            rb = rb.body(req.body.clone());
        }
        let resp = rb.send().map_err(|_| GhError::Transport)?;
        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = resp.bytes().map_err(|_| GhError::Transport)?.to_vec();
        Ok(HttpResponse { status, headers, body })
    }
}

