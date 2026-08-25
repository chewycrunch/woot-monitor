//! Client for the sidecar [tls-client] API, which performs the outbound HTTPS
//! requests on the monitor's behalf so they carry a real browser's TLS
//! fingerprint.
//!
//! [tls-client]: https://github.com/bogdanfinn/tls-client-api

use std::collections::HashMap;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::json;

/// Auth key expected by the sidecar. Overridden together with the sidecar's own
/// `API_AUTH_KEYS`, which replaces the key baked into its image.
pub const DEFAULT_TLS_API_KEY: &str = "yawn";

/// Suits a bare `cargo run` against a sidecar publishing 8080. Deployments put
/// the two on one network and set the service name instead.
pub const DEFAULT_TLS_API_URL: &str = "http://127.0.0.1:8080";

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
    base_url: String,
}

impl TlsClient {
    pub fn new(base_url: String, api_key: &str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(api_key).expect("tls api key is not a valid header value"),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .expect("Failed to create TLS forwarding client");

        Self { http, base_url }
    }

    // @spec FETCHING-011, FETCHING-013, FETCHING-014, FETCHING-017
    /// Forwards a GET through the sidecar and returns the decoded envelope.
    ///
    /// Each forward opens a session on the sidecar, so the session is freed
    /// before returning.
    pub async fn forward(
        &self,
        url: &str,
        headers: HashMap<String, String>,
        proxy_url: String,
    ) -> Result<TlsApiResponse, Box<dyn std::error::Error>> {
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
            .post(format!("{}/api/forward", self.base_url))
            .json(&payload)
            .send()
            .await?;

        let body = response.text().await?;

        let parsed: TlsApiResponse = serde_json::from_str(&body)?;
        self.free_session(&parsed.id).await?;

        Ok(parsed)
    }

    /// Releases a session so the sidecar does not accumulate them.
    async fn free_session(&self, session_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.http
            .post(format!("{}/api/free-session", self.base_url))
            .json(&json!({ "sessionId": session_id }))
            .send()
            .await?;

        Ok(())
    }
}
