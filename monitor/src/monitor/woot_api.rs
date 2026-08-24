//! Client and response types for Woot's public GraphQL API.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::header::HeaderMap;
use reqwest::{Client, ClientBuilder};
use serde::Deserialize;
use tracing::info;
use urlencoding::encode;

use super::product::Product;
use crate::proxy::ProxyManager;

/// GraphQL endpoint backing woot.com's own search.
const GRAPHQL_URL: &str = "https://d24qg5zsx8xdc4.cloudfront.net/graphql";

/// Public API key woot.com ships to its own front end.
///
/// This is an AppSync API key, which AWS caps at a 365-day lifetime, so it goes
/// stale on its own schedule regardless of anything on our side. An expired key
/// surfaces as `UnauthorizedException` with "You are not authorized to make this
/// call." — as distinct from "Valid authorization header not provided.", which
/// means the header never went out. Refresh it by grepping woot.com's HTML for
/// the current `da2-` value.
const GRAPHQL_API_KEY: &str = "da2-gdf6f2cxpnb3xikqgzzhfhovem";

/// Browser identity presented to Woot. The three values travel together — a
/// `User-Agent` naming one Chrome version alongside a `sec-ch-ua` naming another
/// is a fingerprint mismatch, so bump them as a set.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";
const SEC_CH_UA: &str = r#""Google Chrome";v="137", "Chromium";v="137", "Not/A)Brand";v="24""#;
const SEC_CH_UA_PLATFORM: &str = "\"Windows\"";

/// Offers requested per page.
const PAGE_SIZE: u16 = 200;

/// Public URL of an offer's page on woot.com.
pub fn offer_url(slug: &str) -> String {
    format!("https://www.woot.com/offers/{}", slug)
}

// API Structure
// This structure represents the response from Woot's API.
// Each offer has multiple products (items), this can be different colors etc. They can each their own unique price and attributes.

#[derive(Deserialize, Debug)]
pub struct WootResponse {
    pub data: Option<WootData>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Deserialize, Debug)]
pub struct GraphQLError {
    pub message: String,
}

#[derive(Deserialize, Debug)]
pub struct WootData {
    #[serde(rename = "searchOffers")]
    pub search_offers: Option<SearchOffers>,
}

#[derive(Deserialize, Debug)]
pub struct SearchOffers {
    #[serde(rename = "Offers")]
    pub offers: Vec<WootOffer>,
    // #[serde(rename = "TotalHits")]
    // pub total_hits: u32,
}

#[derive(Deserialize, Debug)]
pub struct WootOffer {
    #[serde(rename = "Id")]
    pub id: String,
    // #[serde(rename = "IsAppFeatured")]
    // pub is_app_featured: bool,
    // #[serde(rename = "IsFeatured")]
    // pub is_featured: bool,
    #[serde(rename = "SoldOut")]
    pub sold_out: bool,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Photos")]
    pub photos: Option<Vec<Photo>>,
    #[serde(rename = "EndDate")]
    pub end_date: DateTime<Utc>,
    #[serde(rename = "Items")]
    pub items: Vec<Item>,
    #[serde(rename = "Slug")]
    pub slug: String,
}

#[derive(Deserialize, Debug)]
pub struct Item {
    #[serde(rename = "ListPrice")]
    pub list_price: Option<f64>,
    #[serde(rename = "SalePrice")]
    pub sale_price: f64,
    #[serde(rename = "Attributes")]
    pub attributes: Option<Vec<WootAttribute>>,
}

#[derive(Deserialize, Debug)]
pub struct WootAttribute {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "Value")]
    pub value: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Photo {
    // #[serde(rename = "Width")]
    // pub width: u32,
    // #[serde(rename = "Height")]
    // pub height: u32,
    #[serde(rename = "Url")]
    pub url: String,
}

/// Collapses a multi-line GraphQL query into a single line.
///
/// The queries are written as indented raw strings for readability, but they
/// travel to Woot as a URL query parameter, so the whitespace is only overhead.
/// Comment lines are dropped rather than joined, since a `#` comment would
/// otherwise swallow the rest of the flattened query.
pub fn minify_graphql(query: &str) -> String {
    query
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Client for Woot's GraphQL API. Rotates to the next proxy on every request.
pub struct WootApi {
    proxy_manager: Arc<ProxyManager>,
}

impl WootApi {
    pub fn new(proxy_manager: Arc<ProxyManager>) -> Self {
        Self { proxy_manager }
    }

    /// Fetches every available offer, paging until Woot returns a short page.
    pub async fn fetch_all_offers(&self) -> Result<Vec<Product>, Box<dyn std::error::Error>> {
        let mut all_products = Vec::new();
        let mut skip: u8 = 0;

        loop {
            let response = self.fetch_offers(PAGE_SIZE, skip).await?;

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

            if count < PAGE_SIZE.into() {
                break;
            }

            skip += 1
        }

        info!(requests = skip + 1, limit = PAGE_SIZE, "Fetched all offers");

        Ok(all_products)
    }

    /// Fetches a single page of offers. `skip` is a page index, not an offset.
    async fn fetch_offers(
        &self,
        limit: u16,
        skip: u8,
    ) -> Result<WootResponse, Box<dyn std::error::Error>> {
        let client = self.client(|builder| builder)?;

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

        let url = format!("{}?query={}", GRAPHQL_URL, encode(&minify_graphql(&query)));

        let response = client
            .get(url)
            .headers(Self::graphql_headers())
            .send()
            .await?;
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

    /// Builds a one-shot client bound to the next proxy in the rotation.
    ///
    /// A fresh client per request is deliberate: reqwest fixes the proxy at
    /// build time, so reusing one would pin every request to the same proxy.
    fn client<F>(&self, customize: F) -> Result<Client, String>
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

    /// Headers for an ordinary browser navigation to a woot.com offer page.
    /// Sent through the tls-client sidecar, which wants plain strings.
    pub fn page_headers() -> HashMap<String, String> {
        let mut headers = HashMap::new();

        headers.insert("Host".into(), "www.woot.com".into());
        headers.insert("sec-ch-ua".into(), SEC_CH_UA.into());
        headers.insert("sec-ch-ua-mobile".into(), "?0".into());
        headers.insert("sec-ch-ua-platform".into(), SEC_CH_UA_PLATFORM.into());
        headers.insert("Upgrade-Insecure-Requests".into(), "1".into());
        headers.insert("User-Agent".into(), USER_AGENT.into());
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

    /// Headers for a cross-origin XHR from woot.com to the GraphQL endpoint.
    fn graphql_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers.insert("Host", "d24qg5zsx8xdc4.cloudfront.net".parse().unwrap());
        headers.insert("sec-ch-ua-platform", SEC_CH_UA_PLATFORM.parse().unwrap());
        headers.insert("User-Agent", USER_AGENT.parse().unwrap());
        headers.insert("sec-ch-ua", SEC_CH_UA.parse().unwrap());
        headers.insert("x-api-key", GRAPHQL_API_KEY.parse().unwrap());
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
}
