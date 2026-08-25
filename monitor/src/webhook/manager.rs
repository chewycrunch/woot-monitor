use crate::{
    config::WebhookConfig,
    proxy::ProxyManager,
    webhook::{discord::DiscordWebhook, WebhookPayload},
};
use std::collections::HashSet;
use std::sync::Arc;

/// Reviews an offer needs, alongside an ASIN, to count as verified.
pub const MIN_REVIEWS: u32 = 200;

pub struct WebhookManager {
    proxy_manager: Arc<ProxyManager>,
    unverified: Vec<DiscordWebhook>,
    verified: Vec<DiscordWebhook>,
    watchlist: Vec<(DiscordWebhook, Vec<String>, Vec<String>)>,
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
            unverified: Vec::new(),
            verified: Vec::new(),
            watchlist: Vec::new(),
        }
    }

    pub fn register_from_configs(&mut self, configs: Vec<WebhookConfig>) {
        for cfg in configs {
            if let Some(url) = &cfg.unverified_url {
                let hook = DiscordWebhook::new(Arc::clone(&self.proxy_manager), url.clone());
                self.unverified.push(hook);
            }

            if let Some(url) = &cfg.verified_url {
                let hook = DiscordWebhook::new(Arc::clone(&self.proxy_manager), url.clone());
                self.verified.push(hook);
            }

            if let Some(url) = &cfg.watchlist_url {
                let hook = DiscordWebhook::new(Arc::clone(&self.proxy_manager), url.clone());
                let keywords = cfg.keywords.clone().unwrap_or_default();
                let asins = cfg.asins.clone().unwrap_or_default();

                self.watchlist.push((hook, keywords, asins));
            }
        }
    }

    pub async fn broadcast(&self, payload: WebhookPayload, item_info: ItemInfo) {
        // Exclusive: an offer is one or the other, never both. A failed details
        // scrape yields no ASIN, so it lands as unverified rather than erroring.
        if item_info.asin.is_some() && item_info.total_reviews.unwrap_or(0) >= MIN_REVIEWS {
            for hook in &self.verified {
                let _ = hook.send(payload.clone()).await;
            }
        } else {
            for hook in &self.unverified {
                let _ = hook.send(payload.clone()).await;
            }
        }

        // Independent of the split above: a keyword match is an explicit
        // request, so it is not gated on the offer being verified.
        let hooks: Vec<&DiscordWebhook> = {
            let lower = item_info.title.to_lowercase();
            let words: HashSet<String> = lower
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| !w.is_empty())
                .map(String::from) // No need for another to_lowercase()
                .collect();

            let asin_lower = item_info.asin.as_ref().map(|s| s.to_lowercase());

            self.watchlist
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
