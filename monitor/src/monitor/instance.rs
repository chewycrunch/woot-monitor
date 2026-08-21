// General
use std::{sync::Arc, time::Duration};

// Utils
use regex::Regex;
use tracing::{debug, error, info};
use urlencoding::encode;

// Project
use super::product::{Product, Products};
use super::tls_api::TlsClient;
use super::woot_api::WootApi;
use crate::proxy::ProxyManager;
use crate::webhook::{DiscordEmbed, DiscordPayload, ItemInfo, WebhookManager, WebhookPayload};

pub struct MonitorInstance {
    delay: Duration,
    products: Products,
    webhook_manager: WebhookManager,
    proxy_manager: Arc<ProxyManager>,
    woot_api: WootApi,
    tls_client: TlsClient,
}

impl MonitorInstance {
    pub fn new(webhook_manager: WebhookManager, proxy_manager: Arc<ProxyManager>) -> Self {
        Self {
            delay: Duration::from_secs(5),
            products: Products::new(),
            webhook_manager,
            woot_api: WootApi::new(Arc::clone(&proxy_manager)),
            proxy_manager,
            tls_client: TlsClient::new(),
        }
    }

    /// Starts the Woot Monitor instance.
    /// This function initializes the monitor, then begins monitoring for new products.
    pub async fn start(&mut self) {
        info!(delay_ms = self.delay.as_millis() as u64, "Starting monitor");

        match self.initialize().await {
            Ok(count) => count,
            Err(e) => {
                error!(error = %e, "Error during initialization");

                return;
            }
        };

        tokio::time::sleep(self.delay).await;

        self.monitor().await;
    }

    /// Intializes the monitor by loading in all products from Woot.
    pub async fn initialize(&mut self) -> Result<u32, Box<dyn std::error::Error>> {
        info!("Initializing Monitor | Adding initial offers...");

        let all_products = self.woot_api.fetch_all_offers().await?;
        let all_products_len = all_products.len();

        for product in all_products {
            self.products.add_offer(product);
        }

        info!(count = all_products_len, "Added offers");

        Ok(self.products.get_count() as u32)
    }

    /// Monitors Woot for new products.
    pub async fn monitor(&mut self) {
        loop {
            match self.woot_api.fetch_all_offers().await {
                Ok(products) => {
                    info!(count = products.len(), "Fetched offers");

                    for product in products {
                        let is_new = self.products.add_offer(product.clone());

                        if is_new {
                            info!(target: "offers", id = %product.id, "New offer detected");
                        }

                        if is_new && !product.out_of_stock {
                            let url = format!("https://www.woot.com/offers/{}", product.slug);
                            let (review_count, asin) = match self.fetch_offer_details(&url).await {
                                Ok((reviews, asin_val)) => (reviews, asin_val),
                                Err(e) => {
                                    error!(error = %e, "Error fetching offer details");
                                    (None, None)
                                }
                            };

                            debug!(?review_count, ?asin, "Fetched offer details");

                            self.new_offer_webhook(&product, &review_count, &asin).await;
                        }
                    }
                }
                Err(e) => error!(error = %e, "Error fetching offers"),
            }

            tokio::time::sleep(self.delay).await;
        }
    }

    /// Fetches the details of a specific offer, including ASIN and total reviews.
    pub async fn fetch_offer_details(
        &self,
        url: &str,
    ) -> Result<(Option<u32>, Option<String>), Box<dyn std::error::Error>> {
        let proxy_url = self
            .proxy_manager
            .get_next_proxy()
            .map(|p| p.to_proxy_url())
            .unwrap_or_default();

        let body = self
            .tls_client
            .forward(url, WootApi::page_headers(), proxy_url)
            .await?;

        let total_reviews = Self::extract_total_reviews(&body);
        let asin = Self::extract_asin(&body);

        Ok((total_reviews, asin))
    }

    pub fn extract_total_reviews(html_content: &str) -> Option<u32> {
        if let Ok(regex) = Regex::new(r#"\\?"TotalReviewCount\\?"\s*:\s*(\d+)"#) {
            if let Some(captures) = regex.captures(html_content) {
                if let Some(matched) = captures.get(1) {
                    if let Ok(count) = matched.as_str().parse::<u32>() {
                        return Some(count);
                    }
                }
            }
        }
        None
    }

    pub fn extract_asin(html_content: &str) -> Option<String> {
        if let Ok(regex) = Regex::new(
            r#"(?s)RatingSummaryData\s*=\s*\[.*?\\?"Asin\\?"\s*:\s*\\?"([A-Z0-9]{10})\\?""#,
        ) {
            if let Some(captures) = regex.captures(html_content) {
                if let Some(matched) = captures.get(1) {
                    return Some(matched.as_str().to_string());
                }
            }
        }
        None
    }

    /// Sends a webhook notification for a new offer.
    pub async fn new_offer_webhook(
        &self,
        product: &Product,
        review_count: &Option<u32>,
        asin: &Option<String>,
    ) {
        let embed = DiscordEmbed {
            author: Some(crate::webhook::DiscordEmbedAuthor {
                name: "Woot".to_string(),
                url: Some("https://www.woot.com".to_string()),
                icon_url: None, // Add an icon URL if needed
            }),
            thumbnail: product
                .photos
                .get(0)
                .map(|p| crate::webhook::DiscordEmbedThumbnail { url: p.url.clone() }),

            title: Some(product.title.clone()),
            url: Some(format!("https://www.woot.com/offers/{}", product.slug)),
            description: Some(format!(
                "End Date: {}",
                // product.sale_price.unwrap_or(0) as f32 / 100.0,
                product.end_date.to_string()
            )),

            color: Some(0x00FF00),
            fields: Some({
                let mut fields: Vec<crate::webhook::DiscordEmbedField> = product
                    .variants
                    .iter()
                    .map(|v| crate::webhook::DiscordEmbedField {
                        name: v.attrs.clone().unwrap_or("Default Variant".into()),
                        value: if v.list_price.unwrap_or(0) == 0 {
                            format!("${:.2}", v.sale_price.unwrap_or(0) as f32 / 100.0)
                        } else {
                            format!(
                                "~~${:.2}~~ ${:.2}",
                                v.list_price.unwrap_or(0) as f32 / 100.0,
                                v.sale_price.unwrap_or(0) as f32 / 100.0
                            )
                        },
                        inline: Some(true),
                    })
                    .collect();

                if let Some(count) = review_count {
                    fields.push(crate::webhook::DiscordEmbedField {
                        name: "Reviews".to_string(),
                        value: count.to_string(),
                        inline: Some(false),
                    });
                }
                if let Some(asin_val) = asin {
                    fields.push(crate::webhook::DiscordEmbedField {
                        name: "Amazon".to_string(),
                        value: format!("[{}](https://www.amazon.com/dp/{})", asin_val, asin_val),
                        inline: Some(true),
                    });
                }

                fields.push(crate::webhook::DiscordEmbedField {
                        name: "Ebay".to_string(),
                        value: format!(
                            "[Search](https://www.ebay.com/sch/i.html?_nkw={}&rt=nc&LH_Sold=1&LH_Complete=1)",
                        encode(&product.title)
                        ),
                        inline: Some(true),
                    });
                fields
            }),
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        };

        let payload = WebhookPayload::Discord(DiscordPayload {
            embeds: Some(vec![embed]),
            ..Default::default()
        });

        self.webhook_manager
            .broadcast(
                payload,
                ItemInfo {
                    total_reviews: review_count.clone(),
                    asin: asin.clone(),
                    title: product.title.clone(),
                },
            )
            .await;
    }
}
