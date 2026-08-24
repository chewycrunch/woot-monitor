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
use crate::proxy::ProxyManager;
use crate::webhook::{ItemInfo, WebhookManager, WebhookPayload};

/// First wait after a failed initialization, doubled on each further failure.
const INIT_RETRY_INITIAL: Duration = Duration::from_secs(5);

/// Ceiling on the retry wait. Initialization failures are usually either
/// transient (a proxy or a timeout, cleared by the next attempt) or structural
/// (an expired API key, cleared only by a deploy), and backing off past a minute
/// would delay recovery from the first without meaningfully helping the second.
const INIT_RETRY_MAX: Duration = Duration::from_secs(60);

pub struct Monitor {
    delay: Duration,
    products: Products,
    webhook_manager: WebhookManager,
    proxy_manager: Arc<ProxyManager>,
    woot_api: WootApi,
    tls_client: TlsClient,
}

/// Doubles the wait, stopping at [`INIT_RETRY_MAX`].
fn next_backoff(current: Duration) -> Duration {
    (current * 2).min(INIT_RETRY_MAX)
}

impl Monitor {
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

        // `run` already tolerates a failed fetch and carries on, so treating the
        // identical failure as fatal here only converted a recoverable blip into
        // a process exit — and, under a restart policy, a silent restart loop.
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
    pub async fn run(&mut self) {
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn never_waits_longer_than_the_ceiling() {
        assert_eq!(next_backoff(Duration::from_secs(40)), INIT_RETRY_MAX);
        assert_eq!(next_backoff(INIT_RETRY_MAX), INIT_RETRY_MAX);
    }

    /// A structural failure retries indefinitely, so the wait has to converge
    /// rather than grow without bound.
    #[test]
    fn converges_on_the_ceiling_from_the_initial_wait() {
        let mut backoff = INIT_RETRY_INITIAL;
        for _ in 0..64 {
            backoff = next_backoff(backoff);
        }
        assert_eq!(backoff, INIT_RETRY_MAX);
    }
}
