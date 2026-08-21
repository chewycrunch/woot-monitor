//! Webhook delivery.
//!
//! This module is deliberately provider-agnostic: it knows how to address and
//! send a message, not what the message is about. Rendering a Woot offer into
//! a payload lives on the monitor side, in `monitor::notify`.

pub mod discord;
pub mod manager;

pub use manager::{ItemInfo, WebhookManager};

use discord::DiscordPayload;

/// A message bound for one webhook provider. Extendable for other services.
#[derive(Debug, Clone)]
pub enum WebhookPayload {
    Discord(DiscordPayload),
    // Slack(SlackPayload),
    // Telegram(TelegramPayload),
}

/// Trait for any webhook sender.
#[async_trait::async_trait]
pub trait WebhookSender: Send + Sync {
    async fn send(&self, payload: WebhookPayload) -> Result<(), reqwest::Error>;
}
