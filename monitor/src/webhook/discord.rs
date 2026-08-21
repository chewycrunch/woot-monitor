use crate::proxy::ProxyManager;
use crate::webhook::{ WebhookPayload};
use reqwest::Client;
use serde_json::to_value;
use std::sync::Arc;

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
