pub mod notify;
pub mod product;
pub mod scrape;
pub mod tls_api;
pub mod transform;
pub mod woot_api;

// General
use std::{sync::Arc, time::Duration};

// Utils
use tracing::{debug, error, info};

// Project
use self::product::{Product, Products};
use self::tls_api::TlsClient;
use self::woot_api::WootApi;
use chrono::{DateTime, Utc};

use crate::config::Config;
use crate::proxy::ProxyManager;
use crate::webhook::{ItemInfo, WebhookManager, WebhookPayload};

/// First wait after a failed initialization, doubled on each further failure.
const INIT_RETRY_INITIAL: Duration = Duration::from_secs(5);

/// Ceiling on the retry wait. Longer would delay recovery from a transient
/// failure without helping a structural one, which needs a deploy regardless.
const INIT_RETRY_MAX: Duration = Duration::from_secs(60);

pub struct Monitor {
    delay: Duration,
    products: Products,
    webhook_manager: WebhookManager,
    proxy_manager: Arc<ProxyManager>,
    woot_api: WootApi,
    tls_client: TlsClient,
    /// Newest StartDate already seen, the point each poll pages back from.
    newest_start_date: Option<DateTime<Utc>>,
}

/// Doubles the wait, stopping at [`INIT_RETRY_MAX`].
fn next_backoff(current: Duration) -> Duration {
    (current * 2).min(INIT_RETRY_MAX)
}

impl Monitor {
    pub fn new(
        webhook_manager: WebhookManager,
        proxy_manager: Arc<ProxyManager>,
        config: &Config,
    ) -> Self {
        Self {
            delay: Duration::from_millis(config.delay_ms),
            products: Products::new(),
            webhook_manager,
            woot_api: WootApi::new(Arc::clone(&proxy_manager), config.graphql_api_key.clone()),
            proxy_manager,
            tls_client: TlsClient::new(config.tls_api_url.clone(), &config.tls_api_key),
            newest_start_date: None,
        }
    }

    // @spec DETECTION-011, DETECTION-012, DETECTION-013
    /// Starts the Woot Monitor instance.
    /// This function initializes the monitor, then begins monitoring for new products.
    pub async fn start(&mut self) {
        info!(delay_ms = self.delay.as_millis() as u64, "Starting monitor");

        // `run` already carries on after a failed fetch; exiting here turned the
        // same blip into a restart loop.
        let mut backoff = INIT_RETRY_INITIAL;
        let mut attempt: u32 = 1;

        while let Err(e) = self.initialize().await {
            error!(
                error = %e,
                attempt,
                retry_in_ms = backoff.as_millis() as u64,
                "Error during initialization, retrying"
            );

            tokio::time::sleep(backoff).await;

            backoff = next_backoff(backoff);
            attempt += 1;
        }

        tokio::time::sleep(self.delay).await;

        self.run().await;
    }

    // @spec DETECTION-010
    /// Intializes the monitor by loading in all products from Woot.
    pub async fn initialize(&mut self) -> Result<u32, Box<dyn std::error::Error>> {
        info!("Initializing Monitor | Adding initial offers...");

        let all_products = self.woot_api.fetch_all_offers().await?;
        let all_products_len = all_products.len();

        for product in all_products {
            self.advance_newest(&product);
            self.products.add_offer(product);
        }

        info!(count = all_products_len, "Added offers");

        Ok(self.products.get_count() as u32)
    }

    // @spec DETECTION-040, DETECTION-041
    /// Tracks the newest StartDate seen, which bounds how deep a poll pages.
    fn advance_newest(&mut self, product: &Product) {
        if self
            .newest_start_date
            .is_none_or(|newest| product.start_date > newest)
        {
            self.newest_start_date = Some(product.start_date);
        }
    }

    // @spec DETECTION-023, DETECTION-030, DETECTION-031, DETECTION-032, DETECTION-033, DETECTION-050, DETECTION-051, ROUTING-001, ROUTING-002
    /// Monitors Woot for new products.
    pub async fn run(&mut self) {
        loop {
            // Before the first sweep completes there is no cutoff to page back
            // from, so fall back to reading everything.
            let fetched = match self.newest_start_date {
                Some(since) => self.woot_api.fetch_offers_since(since).await,
                None => self.woot_api.fetch_all_offers().await,
            };

            match fetched {
                Ok(products) => {
                    info!(count = products.len(), "Fetched offers");

                    for product in products {
                        self.advance_newest(&product);
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

    // @spec FETCHING-024
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

#[cfg(test)]
mod tests {
    use super::*;

    // @spec DETECTION-011
    #[test]
    fn doubles_the_wait_on_each_failure() {
        assert_eq!(
            next_backoff(Duration::from_secs(5)),
            Duration::from_secs(10)
        );
        assert_eq!(
            next_backoff(Duration::from_secs(10)),
            Duration::from_secs(20)
        );
    }

    // @spec DETECTION-011
    #[test]
    fn never_waits_longer_than_the_ceiling() {
        assert_eq!(next_backoff(Duration::from_secs(40)), INIT_RETRY_MAX);
        assert_eq!(next_backoff(INIT_RETRY_MAX), INIT_RETRY_MAX);
    }

    // @spec DETECTION-011
    /// Retries are unbounded, so the wait must converge.
    #[test]
    fn converges_on_the_ceiling_from_the_initial_wait() {
        let mut backoff = INIT_RETRY_INITIAL;
        for _ in 0..64 {
            backoff = next_backoff(backoff);
        }
        assert_eq!(backoff, INIT_RETRY_MAX);
    }
}
