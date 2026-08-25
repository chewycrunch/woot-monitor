use crate::{
    config::WebhookConfig,
    proxy::ProxyManager,
    webhook::{discord::DiscordWebhook, WebhookPayload},
};
use std::sync::Arc;

/// Reviews an offer needs, alongside an ASIN, to count as verified.
pub const MIN_REVIEWS: u32 = 200;

// @spec ROUTING-023, ROUTING-024
/// Reduces text to its lowercased words, space-delimited and space-padded, so
/// `contains` matches whole words only: " nest " does not occur in " honest ",
/// while a multi-word keyword like "instant pot" survives as one string.
fn normalize(text: &str) -> String {
    let mut normalized = String::from(" ");

    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
    {
        normalized.push_str(&word.to_lowercase());
        normalized.push(' ');
    }

    normalized
}

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

    // @spec ROUTING-025, ROUTING-033, CONFIG-013
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
                // Normalized once here rather than per offer. Keywords that
                // are punctuation-only normalize to " " and would match every
                // title, so they are dropped instead.
                let keywords: Vec<String> = cfg
                    .keywords
                    .clone()
                    .unwrap_or_default()
                    .iter()
                    .map(|keyword| normalize(keyword))
                    .filter(|keyword| keyword != " ")
                    .collect();
                let asins = cfg.asins.clone().unwrap_or_default();

                self.watchlist.push((hook, keywords, asins));
            }
        }
    }

    // @spec ROUTING-010, ROUTING-011, ROUTING-012, ROUTING-013, ROUTING-020, ROUTING-021, ROUTING-022, ROUTING-026, ROUTING-030, ROUTING-032, ROUTING-033
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
            let title = normalize(&item_info.title);
            let asin_lower = item_info.asin.as_ref().map(|asin| asin.to_lowercase());

            self.watchlist
                .iter()
                .filter(|(_, keywords, asins)| {
                    keywords.iter().any(|keyword| title.contains(keyword))
                        || asins
                            .iter()
                            .map(|asin| asin.to_lowercase())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Keywords are stored normalized, so a match is `contains` on the title.
    fn matches(keyword: &str, title: &str) -> bool {
        normalize(title).contains(&normalize(keyword))
    }

    // @spec ROUTING-023
    #[test]
    fn matches_a_single_word_anywhere_in_the_title() {
        assert!(matches("ninja", "Ninja Professional Blender"));
        assert!(matches("ninja", "Refurbished Ninja"));
    }

    // @spec ROUTING-023
    /// The previous matcher split the title into whole words and looked each
    /// keyword up, so every multi-word entry in a config could never fire.
    #[test]
    fn matches_multi_word_keywords() {
        assert!(matches("instant pot", "Instant Pot Duo 6qt"));
        assert!(matches("6700 xt", "AMD Radeon RX 6700 XT 12GB"));
    }

    // @spec ROUTING-024
    /// Punctuation is a word break on both sides, not a deletion, so "g.skill"
    /// and "gskill" stay distinct tokens. That is why a config wanting both
    /// spellings has to list both, as this one does.
    #[test]
    fn treats_punctuation_as_a_word_break() {
        assert!(matches("g.skill", "G.SKILL Trident Z 32GB"));
        assert!(matches("g.skill", "G Skill Trident Z"));
        assert!(matches("gskill", "GSKILL Trident Z"));

        assert!(!matches("g.skill", "GSKILL Trident Z"));
        assert!(!matches("gskill", "G.Skill Trident Z"));
    }

    // @spec ROUTING-023
    #[test]
    fn ignores_case_on_both_sides() {
        assert!(matches("DEWALT", "dewalt drill"));
    }

    // @spec ROUTING-023
    /// Substring matching without word boundaries would fire "nest" on
    /// "Honest", which is why the normalized forms are space-padded.
    #[test]
    fn does_not_match_inside_a_longer_word() {
        assert!(!matches("nest", "The Honest Company Wipes"));
        assert!(!matches("shark", "Sharkskin Wallet"));
    }

    // @spec ROUTING-023
    #[test]
    fn does_not_match_a_different_word() {
        assert!(!matches("dyson", "Shark Vacuum"));
    }

    // @spec ROUTING-025
    /// A keyword of only punctuation normalizes to " ", which every title
    /// contains, so registration drops those rather than matching everything.
    #[test]
    fn punctuation_only_keywords_normalize_to_a_bare_space() {
        assert_eq!(normalize("---"), " ");
        assert_eq!(normalize(""), " ");
    }
}
