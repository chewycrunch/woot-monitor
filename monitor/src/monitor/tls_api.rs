//! Client for the sidecar [tls-client] API, which performs the outbound HTTPS
//! requests on the monitor's behalf so they carry a real browser's TLS
//! fingerprint.
//!
//! [tls-client]: https://github.com/bogdanfinn/tls-client-api

use std::collections::HashMap;
use std::env;
use std::sync::LazyLock;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::json;

/// Auth key expected by the sidecar. Must match `api_auth_keys` in
/// `tls-client/config.yml`.
const API_KEY: &str = "yawn";

/// Base URL of the tls-client API, overridable with the `TLS_API_URL` env var.
///
/// The default suits both a bare `cargo run` against a tls-client container
/// publishing 8080, and an ECS task where both containers share a network
/// namespace. Deployments that put the two on separate networks set this to
/// the sidecar's hostname, e.g. `http://tls-client:8080`.
static BASE_URL: LazyLock<String> = LazyLock::new(|| {
    env::var("TLS_API_URL")
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string())
});

/// Envelope the tls-client API wraps every forwarded response in. The upstream
/// response body arrives as a JSON-escaped string in `body`.
#[derive(Deserialize, Debug)]
pub struct TlsApiResponse {
    pub cookies: serde_json::Value,
    pub headers: serde_json::Value,
    pub id: String,
    pub body: String,
    pub target: String,
    #[serde(rename = "usedProtocol")]
    pub used_protocol: String,
    pub status: u16,
}

/// Handle on the tls-client sidecar.
pub struct TlsClient {
    http: reqwest::Client,
}

impl TlsClient {
    pub fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_static(API_KEY));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .expect("Failed to create TLS forwarding client");

        Self { http }
    }

    /// Forwards a GET through the sidecar and returns its raw response text.
    ///
    /// The text is the whole `TlsApiResponse` envelope rather than the decoded
    /// upstream body, because the callers scrape it with regexes written
    /// against the JSON-escaped form. Each forward opens a session on the
    /// sidecar, so the session is freed before returning.
    pub async fn forward(
        &self,
        url: &str,
        headers: HashMap<String, String>,
        proxy_url: String,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let payload = json!({
            "tlsClientIdentifier": "chrome_137",
            "requestUrl": url,
            "requestMethod": "GET",
            "requestHeaders": headers,
            "followRedirects": true,
            "proxyUrl": proxy_url,
        });

        let response = self
            .http
            .post(format!("{}/api/forward", *BASE_URL))
            .json(&payload)
            .send()
            .await?;

        let body = response.text().await?;

        let parsed: TlsApiResponse = serde_json::from_str(&body)?;
        self.free_session(&parsed.id).await?;

        Ok(body)
    }

    /// Releases a session so the sidecar does not accumulate them.
    async fn free_session(&self, session_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.http
            .post(format!("{}/api/free-session", *BASE_URL))
            .json(&json!({ "sessionId": session_id }))
            .send()
            .await?;

        Ok(())
    }
}

impl Default for TlsClient {
    fn default() -> Self {
        Self::new()
    }
}
