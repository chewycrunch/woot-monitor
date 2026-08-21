// General
use std::{sync::Arc, time::Duration};

// Utils
use tracing::{debug, error, info};

// Project
use super::notify;
use super::product::{Product, Products};
use super::scrape;
use super::tls_api::TlsClient;
use super::woot_api::{self, WootApi};
use crate::proxy::ProxyManager;
use crate::webhook::{ItemInfo, WebhookManager, WebhookPayload};

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
                            let url = woot_api::offer_url(&product.slug);
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

        let response = self
            .tls_client
            .forward(url, WootApi::page_headers(), proxy_url)
            .await?;

        Ok((
            scrape::total_reviews(&response.body),
            scrape::asin(&response.body),
        ))
    }

    /// Sends a webhook notification for a new offer.
    pub async fn new_offer_webhook(
        &self,
        product: &Product,
        review_count: &Option<u32>,
        asin: &Option<String>,
    ) {
        let payload = WebhookPayload::Discord(notify::offer_payload(
            product,
            *review_count,
            asin.as_deref(),
        ));

        self.webhook_manager
            .broadcast(
                payload,
                ItemInfo {
                    total_reviews: *review_count,
                    asin: asin.clone(),
                    title: product.title.clone(),
                },
            )
            .await;
    }
}
