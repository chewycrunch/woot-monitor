use crate::{
    config::WebhookConfig,
    proxy::ProxyManager,
    webhook::{discord::DiscordWebhook, WebhookPayload},
};
use std::collections::HashSet;
use std::sync::Arc;

pub struct WebhookManager {
    proxy_manager: Arc<ProxyManager>,
    junk: Vec<DiscordWebhook>,
    unfiltered: Vec<DiscordWebhook>,
    filtered: Vec<(DiscordWebhook, Vec<String>, Vec<String>)>,
}

pub struct ItemInfo {
    pub total_reviews: Option<u32>,
    pub asin: Option<String>,
    pub title: String,
}

impl WebhookManager {
    pub fn new(proxy_manager: Arc<ProxyManager>) -> Self {
        Self {
            proxy_manager,
            junk: Vec::new(),
            unfiltered: Vec::new(),
            filtered: Vec::new(),
        }
    }

    pub fn register_from_configs(&mut self, configs: Vec<WebhookConfig>) {
        for cfg in configs {
            if let Some(url) = &cfg.junk_url {
                let hook = DiscordWebhook::new(Arc::clone(&self.proxy_manager), url.clone());
                self.junk.push(hook);
            }

            if let Some(url) = &cfg.unfiltered_url {
                let hook = DiscordWebhook::new(Arc::clone(&self.proxy_manager), url.clone());
                self.unfiltered.push(hook);
            }

            if let Some(url) = &cfg.filtered_url {
                let hook = DiscordWebhook::new(Arc::clone(&self.proxy_manager), url.clone());
                let keywords = cfg.keywords.clone().unwrap_or_default();
                let asins = cfg.asins.clone().unwrap_or_default();

                self.filtered.push((hook, keywords, asins));
            }
        }
    }

    pub async fn broadcast(&self, payload: WebhookPayload, item_info: ItemInfo) {
        if item_info.asin.is_some() && item_info.total_reviews.unwrap_or(0) >= 200 {
            // Unfiltered (Anything with ASIN, any reviews)
            for hook in &self.unfiltered {
                let _ = hook.send(payload.clone()).await;
            }
        } else {
            // Junk webhook no reviews or ASIN
            for hook in &self.junk {
                let _ = hook.send(payload.clone()).await;
            }
        }

        // Filtered (Anything with keywords)
        let hooks: Vec<&DiscordWebhook> = {
            let lower = item_info.title.to_lowercase();
            let words: HashSet<String> = lower
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| !w.is_empty())
                .map(String::from) // No need for another to_lowercase()
                .collect();

            let asin_lower = item_info.asin.as_ref().map(|s| s.to_lowercase());

            self.filtered
                .iter()
                .filter(|(_, keywords, asins)| {
                    keywords
                        .iter()
                        .any(|kw| words.contains(kw.to_lowercase().as_str()))
                        || asins
                            .iter()
                            .map(|s| s.to_lowercase())
                            .any(|asin| Some(asin) == asin_lower)
                })
                .map(|(hook, _, _)| hook)
                .collect()
        };

        for hook in hooks {
            let _ = hook.send(payload.clone()).await;
        }
    }
}
