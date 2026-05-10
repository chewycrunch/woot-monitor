pub mod discord;
pub mod manager;

pub use manager::WebhookManager;
pub use manager::ItemInfo;

use serde::Serialize;

/// Generic webhook message. Extendable for other services.
#[derive(Debug, Clone)]
pub enum WebhookPayload {
    Discord(DiscordPayload),
    // Slack(SlackPayload),
    // Telegram(TelegramPayload),
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>, // use ISO 8601 format
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

#[derive(Debug, Serialize, Clone)]
pub struct DiscordTimestamp(pub String); // ISO 8601 string

/// Trait for any webhook sender
#[async_trait::async_trait]
pub trait WebhookSender: Send + Sync {
    async fn send(&self, payload: WebhookPayload) -> Result<(), reqwest::Error>;
}
