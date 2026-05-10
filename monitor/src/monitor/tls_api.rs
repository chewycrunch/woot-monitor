use serde::{Deserialize};
use serde_json;

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