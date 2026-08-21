//! Discord webhooks: the shape of the payload Discord accepts, and the sender
//! that delivers it.
//!
//! Every field Discord treats as optional is an `Option` that is skipped when
//! empty, so a partially filled embed serialises to exactly the keys it set.

use std::sync::Arc;

use reqwest::Client;
use serde::Serialize;
use serde_json::to_value;

use crate::proxy::ProxyManager;
use crate::webhook::WebhookPayload;

#[derive(Debug, Serialize, Clone, Default)]
pub struct DiscordPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embeds: Option<Vec<DiscordEmbed>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tts: Option<bool>,
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct DiscordEmbed {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<DiscordEmbedField>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<DiscordEmbedAuthor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<DiscordEmbedThumbnail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<DiscordEmbedFooter>,
    /// ISO 8601.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct DiscordEmbedAuthor {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct DiscordEmbedThumbnail {
    pub url: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DiscordEmbedField {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct DiscordEmbedFooter {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
}

/// One Discord webhook endpoint.
pub struct DiscordWebhook {
    pub url: String,
    pub proxy_manager: Arc<ProxyManager>,
}

impl DiscordWebhook {
    pub fn new(proxy_manager: Arc<ProxyManager>, url: String) -> Self {
        Self { proxy_manager, url }
    }

    pub async fn send(&self, payload: WebhookPayload) -> Result<(), reqwest::Error> {
        let WebhookPayload::Discord(discord) = payload;

        let json_body = to_value(discord).unwrap();

        let proxy = self.proxy_manager.get_next_proxy();
        let client = match proxy {
            Some(p) => p
                .to_reqwest_proxy()
                .and_then(|rp| Client::builder().proxy(rp).build().ok())
                .unwrap_or_else(Client::new),
            None => Client::new(),
        };

        client
            .post(&self.url)
            .json(&json_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}
