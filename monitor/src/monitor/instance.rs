// General
use std::{
    collections::HashMap,
    env,
    sync::{Arc, LazyLock},
    time::Duration,
};

// Reqwest
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::{Client, ClientBuilder};

// Utils
use regex::Regex;
use serde_json::json;
use tracing::{debug, error, info};
use urlencoding::encode;

// Project
use super::product::{Product, Products};

use super::tls_api::TlsApiResponse;

use super::woot_api::WootResponse;
use crate::monitor::WootData;
use crate::proxy::manager::ProxyManager;
use crate::utils::minify_graphql;
use crate::webhook::{DiscordEmbed, DiscordPayload, ItemInfo, WebhookManager, WebhookPayload};

/// Base URL of the tls-client API, overridable with the `TLS_API_URL` env var.
///
/// The default suits both a bare `cargo run` against a tls-client container
/// publishing 8080, and an ECS task where both containers share a network
/// namespace. Under compose the services sit on their own network, so
/// `compose.yml` sets this to `http://tls-client:8080`.
static BASE_URL: LazyLock<String> = LazyLock::new(|| {
    env::var("TLS_API_URL")
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string())
});

pub struct MonitorInstance {
    delay: Duration,
    products: Products,
    webhook_manager: WebhookManager,
    proxy_manager: Arc<ProxyManager>,
    tls_client: reqwest::Client,
}

impl MonitorInstance {
    pub fn new(webhook_manager: WebhookManager, proxy_manager: Arc<ProxyManager>) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_static("yawn"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let tls_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .expect("Failed to create TLS forwarding client");

        Self {
            delay: Duration::from_secs(5),
            products: Products::new(),
            webhook_manager: webhook_manager,
            proxy_manager: proxy_manager,
            tls_client,
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

        let all_products = self.fetch_all_offers().await?;
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
            match self.fetch_all_offers().await {
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

    // More Internal Stuff here

    /// Refactored: Fetches all offers across paginated requests
    async fn fetch_all_offers(&self) -> Result<Vec<Product>, Box<dyn std::error::Error>> {
        let mut all_products = Vec::new();

        let limit: u16 = 200; // Number of offers to fetch per request
        let mut skip: u8 = 0; // Number of requests to skip (skip * limit)  (Specific # of products to skip)

        loop {
            let response = self.fetch_offers(limit, skip).await?;

            let data = response
                .data
                .ok_or("Missing `data` field in GraphQL response")?;
            let search_offers = data
                .search_offers
                .ok_or("Missing `searchOffers` field in GraphQL response")?;

            let offers = search_offers.offers;
            if offers.is_empty() {
                break;
            }

            let products: Vec<Product> = offers.into_iter().map(Product::from).collect();
            let count: usize = products.len();
            all_products.extend(products);

            if count < limit.into() {
                break;
            }

            skip += 1
        }

        info!(requests = skip + 1, limit, "Fetched all offers");

        Ok(all_products)
    }

    /// Internal function to fetch offers from Woot by page.
    /// Actual request to Woot's API.
    async fn fetch_offers(
        &self,
        limit: u16,
        skip: u8,
    ) -> Result<WootResponse, Box<dyn std::error::Error>> {
        let client = self
            .get_reqwest_client(|builder| builder)
            .expect("Failed to create client");

        let headers = Self::get_graphql_headers();

        let query = format!(
            r#"
                {{
                    searchOffers(
                        Filter: {{
                            Categories: ["home", "tech", "pc", "tools", "sport", "grocery"],
                            IsSoldOut: {{ exclude: true }}
                        }},
                        Sort: BestSelling,
                        Limit: {},
                        Skip: {}
                    ) {{
                        Offers {{
                            Id
                            SoldOut
                            Title
                            EndDate
                            Items {{
                                ListPrice
                                SalePrice
                                Attributes {{
                                    Key
                                    Value
                                }}
                            }}
                            Photos {{
                                Url
                            }}
                            Slug
                        }}
                    }}
                }}"#,
            limit,
            (skip as u16) * limit
        );

        let minified_query = minify_graphql(&query);
        let encoded_query = encode(&minified_query);
        let url = format!(
            "https://d24qg5zsx8xdc4.cloudfront.net/graphql?query={}",
            encoded_query
        );

        let response = client.get(url).headers(headers).send().await?;
        let body = response.text().await?;

        let parsed: WootResponse = serde_json::from_str(&body)?;

        if let Some(errors) = &parsed.errors {
            if !errors.is_empty() {
                return Err(format!("GraphQL errors: {:?}", errors).into());
            }
        }

        let data = parsed
            .data
            .ok_or("Missing `data` field in GraphQL response")?;

        let search_offers = data
            .search_offers
            .ok_or("Missing `searchOffers` field in GraphQL response")?;

        Ok(WootResponse {
            data: Some(WootData {
                search_offers: Some(search_offers),
            }),
            errors: parsed.errors,
        })
    }

    /// Fetches the details of a specific offer, including ASIN and total reviews.
    pub async fn fetch_offer_details(
        &self,
        url: &str,
    ) -> Result<(Option<u32>, Option<String>), Box<dyn std::error::Error>> {
        let headers = Self::get_woot_headers();

        let payload = json!({
            "tlsClientIdentifier": "chrome_137",
            "requestUrl": url,
            "requestMethod": "GET",
            "requestHeaders": headers,
            "followRedirects": true,
            "proxyUrl": self.proxy_manager.get_next_proxy().map(|p| p.to_proxy_url()).unwrap_or_else(|| "".to_string()),
        });

        let response = self
            .tls_client
            .post(format!("{}/api/forward", *BASE_URL))
            .json(&payload)
            .send()
            .await?;

        let body = response.text().await?;

        let tls_response: TlsApiResponse = serde_json::from_str(&body)?;
        let session_id: String = tls_response.id;

        Self::free_tls_session(session_id).await?;

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

    pub fn get_reqwest_client<F>(&self, customize: F) -> Result<Client, String>
    where
        F: FnOnce(ClientBuilder) -> ClientBuilder,
    {
        let mut builder = Client::builder();

        if let Some(proxy) = self.proxy_manager.get_next_proxy() {
            if let Some(reqwest_proxy) = proxy.to_reqwest_proxy() {
                builder = builder.proxy(reqwest_proxy);
            } else {
                return Err("Failed to set proxy".into());
            }
        }

        let builder = customize(builder)
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10));

        builder
            .build()
            .map_err(|e| format!("Failed to build client: {}", e))
    }

    pub fn get_woot_headers() -> HashMap<String, String> {
        let mut headers = HashMap::new();

        headers.insert("Host".into(), "www.woot.com".into());
        headers.insert(
            "sec-ch-ua".into(),
            r#""Google Chrome";v="137", "Chromium";v="137", "Not/A)Brand";v="24""#.into(),
        );
        headers.insert("sec-ch-ua-mobile".into(), "?0".into());
        headers.insert("sec-ch-ua-platform".into(), "\"Windows\"".into());
        headers.insert("Upgrade-Insecure-Requests".into(), "1".into());
        headers.insert(
                "User-Agent".into(),
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36".into(),
            );
        headers.insert(
                "Accept".into(),
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7".into(),
            );
        headers.insert("Sec-Fetch-Site".into(), "none".into());
        headers.insert("Sec-Fetch-Mode".into(), "navigate".into());
        headers.insert("Sec-Fetch-User".into(), "?1".into());
        headers.insert("Sec-Fetch-Dest".into(), "document".into());
        headers.insert("Accept-Language".into(), "en-US,en;q=0.9".into());

        headers
    }

    pub fn get_graphql_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("Host", "d24qg5zsx8xdc4.cloudfront.net".parse().unwrap());
        headers.insert("sec-ch-ua-platform", "\"Windows\"".parse().unwrap());
        headers.insert("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36".parse().unwrap());
        headers.insert(
            "sec-ch-ua",
            "\"Google Chrome\";v=\"137\", \"Chromium\";v=\"137\", \"Not/A)Brand\";v=\"24\""
                .parse()
                .unwrap(),
        );
        headers.insert(
            "x-api-key",
            "da2-hk2jpo7aljfvxollvmieghuqlu".parse().unwrap(),
        );
        headers.insert("sec-ch-ua-mobile", "?0".parse().unwrap());
        headers.insert("Accept", "*/*".parse().unwrap());
        headers.insert("Origin", "https://www.woot.com".parse().unwrap());
        headers.insert("Sec-Fetch-Site", "cross-site".parse().unwrap());
        headers.insert("Sec-Fetch-Mode", "cors".parse().unwrap());
        headers.insert("Sec-Fetch-Dest", "empty".parse().unwrap());
        headers.insert("Referer", "https://www.woot.com/".parse().unwrap());
        headers.insert("Accept-Language", "en-US,en;q=0.9".parse().unwrap());
        headers
    }

    pub async fn free_tls_session(session_id: String) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();

        client
            .post(format!("{}/api/free-session", *BASE_URL))
            .header("x-api-key", "yawn")
            .json(&json!({"sessionId": session_id}))
            .send()
            .await?;

        Ok(())
    }
}
