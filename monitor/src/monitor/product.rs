use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::woot_api::Photo;

#[derive(Clone)]
pub struct Product {
    pub id: String,
    pub out_of_stock: bool,
    pub title: String,
    pub end_date: DateTime<Utc>,
    pub list_price: Option<u32>,
    pub sale_price: Option<u32>,
    pub condition: String,
    pub slug: String,
    pub photos: Vec<Photo>,
    pub variants: Vec<Variant>,
}

#[derive(Clone)]
pub struct Variant {
    pub attrs: Option<String>,
    pub list_price: Option<u32>,
    pub sale_price: Option<u32>,
}

pub struct Products {
    offers: HashMap<String, Product>,
}

impl Default for Products {
    fn default() -> Self {
        Self::new()
    }
}

impl Products {
    pub fn new() -> Self {
        Self {
            offers: HashMap::new(),
        }
    }

    pub fn add_offer(&mut self, offer: Product) -> bool {
        self.offers.insert(offer.id.clone(), offer).is_none()
    }

    pub fn get_count(&self) -> usize {
        self.offers.len()
    }
}
