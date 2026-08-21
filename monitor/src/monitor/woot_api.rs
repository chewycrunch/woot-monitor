use chrono::{DateTime, Utc};
use serde::Deserialize;

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
