//! Client and response types for Woot's public GraphQL API.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::header::HeaderMap;
use reqwest::{Client, ClientBuilder};
use serde::Deserialize;
use tracing::{info, warn};
use urlencoding::encode;

use super::product::Product;
use crate::proxy::ProxyManager;

/// GraphQL endpoint
const GRAPHQL_URL: &str = "https://d24qg5zsx8xdc4.cloudfront.net/graphql";

/// GraphQL API Key. AppSync caps expiry of these at a year, "You are not
/// authorized to make this call." means it is stale. Refresh by grepping
/// woot.com's HTML for the `da2-` value.
pub const DEFAULT_GRAPHQL_API_KEY: &str = "da2-gdf6f2cxpnb3xikqgzzhfhovem";

/// Browser identity presented to Woot. The three values travel together for
/// all requests.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";
const SEC_CH_UA: &str = r#""Google Chrome";v="137", "Chromium";v="137", "Not/A)Brand";v="24""#;
const SEC_CH_UA_PLATFORM: &str = "\"Windows\"";

/// Offers requested per page.
const PAGE_SIZE: u16 = 200;

/// Woot's GraphQL API rejects any search whose `Skip + Limit` exceeds this, so
/// the catalog is only reachable this deep however many offers match.
const MAX_SEARCH_DEPTH: u16 = 10_000;

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
    #[serde(rename = "TotalHits")]
    pub total_hits: u32,
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

/// Collapses a multi-line GraphQL query into a single line for the URL. Comment
/// lines are dropped, not joined: a `#` would swallow the rest of the query.
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
    api_key: String,
}

impl WootApi {
    pub fn new(proxy_manager: Arc<ProxyManager>, api_key: String) -> Self {
        Self {
            proxy_manager,
            api_key,
        }
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

            let total_hits = search_offers.total_hits;
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

            // Past the ceiling the whole request fails rather than returning a
            // short page, so stop first.
            if Self::next_page_depth(skip) > MAX_SEARCH_DEPTH {
                warn!(
                    fetched = all_products.len(),
                    total = total_hits,
                    depth = MAX_SEARCH_DEPTH,
                    "Reached Woot's search depth limit; older offers beyond it are not visible"
                );
                break;
            }

            skip += 1
        }

        info!(requests = skip + 1, limit = PAGE_SIZE, "Fetched all offers");

        Ok(all_products)
    }

    /// Depth the page after `skip` would request, measured as `Skip + Limit`.
    fn next_page_depth(skip: u8) -> u16 {
        (u16::from(skip) + 1) * PAGE_SIZE + PAGE_SIZE
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
                        Sort: NewestFirst,
                        Limit: {},
                        Skip: {}
                    ) {{
                        TotalHits
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
            .headers(self.graphql_headers())
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
    fn graphql_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers.insert("Host", "d24qg5zsx8xdc4.cloudfront.net".parse().unwrap());
        headers.insert("sec-ch-ua-platform", SEC_CH_UA_PLATFORM.parse().unwrap());
        headers.insert("User-Agent", USER_AGENT.parse().unwrap());
        headers.insert("sec-ch-ua", SEC_CH_UA.parse().unwrap());
        headers.insert("x-api-key", self.api_key.parse().unwrap());
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The limit is inclusive, so a naive `>=` would drop the last valid page.
    #[test]
    fn allows_the_page_that_lands_exactly_on_the_depth_limit() {
        assert_eq!(WootApi::next_page_depth(48), MAX_SEARCH_DEPTH);
        assert!(WootApi::next_page_depth(48) <= MAX_SEARCH_DEPTH);
    }

    #[test]
    fn stops_before_the_first_page_that_would_exceed_the_depth_limit() {
        assert!(WootApi::next_page_depth(49) > MAX_SEARCH_DEPTH);
    }

    /// Paging should reach exactly `MAX_SEARCH_DEPTH` offers.
    #[test]
    fn reaches_the_depth_limit_exactly() {
        let last_page = (0u8..)
            .find(|s| WootApi::next_page_depth(*s) > MAX_SEARCH_DEPTH)
            .unwrap();
        assert_eq!(
            u16::from(last_page) * PAGE_SIZE,
            MAX_SEARCH_DEPTH - PAGE_SIZE
        );
        assert_eq!((u16::from(last_page) + 1) * PAGE_SIZE, MAX_SEARCH_DEPTH);
    }
}
