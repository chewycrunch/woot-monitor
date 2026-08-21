//! Renders a Woot offer into the webhook payload announcing it.
//!
//! This sits on the monitor side rather than in `webhook` so that `webhook`
//! stays a generic transport that knows nothing about Woot: the dependency
//! runs monitor -> webhook, and adding a second provider there should not mean
//! teaching it about offers.

use urlencoding::encode;

use super::product::Product;
use super::woot_api;
use crate::webhook::{
    DiscordEmbed, DiscordEmbedAuthor, DiscordEmbedField, DiscordEmbedThumbnail, DiscordPayload,
};

/// Embed accent colour.
const COLOR: u32 = 0x00FF00;

/// Prices are stored as whole cents.
const CENTS_PER_DOLLAR: f32 = 100.0;

/// Builds the announcement payload for a newly detected offer.
pub fn offer_payload(
    product: &Product,
    review_count: Option<u32>,
    asin: Option<&str>,
) -> DiscordPayload {
    let embed = DiscordEmbed {
        author: Some(DiscordEmbedAuthor {
            name: "Woot".to_string(),
            url: Some("https://www.woot.com".to_string()),
            icon_url: None,
        }),
        thumbnail: product
            .photos
            .first()
            .map(|p| DiscordEmbedThumbnail { url: p.url.clone() }),
        title: Some(product.title.clone()),
        url: Some(woot_api::offer_url(&product.slug)),
        description: Some(format!("End Date: {}", product.end_date)),
        color: Some(COLOR),
        fields: Some(offer_fields(product, review_count, asin)),
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    };

    DiscordPayload {
        embeds: Some(vec![embed]),
        ..Default::default()
    }
}

/// One field per variant, then whatever lookup links we managed to resolve.
fn offer_fields(
    product: &Product,
    review_count: Option<u32>,
    asin: Option<&str>,
) -> Vec<DiscordEmbedField> {
    let mut fields: Vec<DiscordEmbedField> = product
        .variants
        .iter()
        .map(|v| DiscordEmbedField {
            name: v.attrs.clone().unwrap_or("Default Variant".into()),
            value: price_label(v.list_price, v.sale_price),
            inline: Some(true),
        })
        .collect();

    if let Some(count) = review_count {
        fields.push(DiscordEmbedField {
            name: "Reviews".to_string(),
            value: count.to_string(),
            inline: Some(false),
        });
    }

    if let Some(asin) = asin {
        fields.push(DiscordEmbedField {
            name: "Amazon".to_string(),
            value: format!("[{}](https://www.amazon.com/dp/{})", asin, asin),
            inline: Some(true),
        });
    }

    fields.push(DiscordEmbedField {
        name: "Ebay".to_string(),
        value: format!(
            "[Search](https://www.ebay.com/sch/i.html?_nkw={}&rt=nc&LH_Sold=1&LH_Complete=1)",
            encode(&product.title)
        ),
        inline: Some(true),
    });

    fields
}

/// Formats a variant's price, striking through the list price when the offer
/// is a genuine discount. A missing or zero list price means "no discount to
/// show", so only the sale price is rendered.
fn price_label(list_price: Option<u16>, sale_price: Option<u16>) -> String {
    let sale = sale_price.unwrap_or(0) as f32 / CENTS_PER_DOLLAR;

    match list_price.unwrap_or(0) {
        0 => format!("${:.2}", sale),
        list => format!("~~${:.2}~~ ${:.2}", list as f32 / CENTS_PER_DOLLAR, sale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::product::Variant;
    use crate::monitor::woot_api::Photo;
    use chrono::TimeZone;

    fn product(variants: Vec<Variant>, photos: Vec<Photo>) -> Product {
        Product {
            id: "42".to_string(),
            out_of_stock: false,
            title: "Anker USB-C Cable".to_string(),
            end_date: chrono::Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            list_price: None,
            sale_price: None,
            condition: "New".to_string(),
            slug: "anker-usb-c-cable".to_string(),
            photos,
            variants,
        }
    }

    fn variant(list_price: Option<u16>, sale_price: Option<u16>) -> Variant {
        Variant {
            attrs: None,
            list_price,
            sale_price,
        }
    }

    fn field<'a>(payload: &'a DiscordPayload, name: &str) -> Option<&'a DiscordEmbedField> {
        payload.embeds.as_ref()?[0]
            .fields
            .as_ref()?
            .iter()
            .find(|f| f.name == name)
    }

    #[test]
    fn shows_only_the_sale_price_when_there_is_no_list_price() {
        assert_eq!(price_label(None, Some(1234)), "$12.34");
    }

    #[test]
    fn treats_a_zero_list_price_as_no_discount() {
        assert_eq!(price_label(Some(0), Some(1234)), "$12.34");
    }

    #[test]
    fn strikes_through_the_list_price_when_discounted() {
        assert_eq!(price_label(Some(2000), Some(1234)), "~~$20.00~~ $12.34");
    }

    #[test]
    fn renders_a_missing_sale_price_as_free() {
        assert_eq!(price_label(None, None), "$0.00");
    }

    #[test]
    fn names_an_unlabelled_variant() {
        let payload = offer_payload(&product(vec![variant(None, Some(999))], vec![]), None, None);
        assert!(field(&payload, "Default Variant").is_some());
    }

    #[test]
    fn links_the_offer_and_titles_the_embed() {
        let payload = offer_payload(&product(vec![], vec![]), None, None);
        let embed = &payload.embeds.as_ref().unwrap()[0];

        assert_eq!(embed.title.as_deref(), Some("Anker USB-C Cable"));
        assert_eq!(
            embed.url.as_deref(),
            Some("https://www.woot.com/offers/anker-usb-c-cable")
        );
    }

    #[test]
    fn uses_the_first_photo_as_the_thumbnail() {
        let photos = vec![
            Photo {
                url: "https://img/first.jpg".to_string(),
            },
            Photo {
                url: "https://img/second.jpg".to_string(),
            },
        ];
        let payload = offer_payload(&product(vec![], photos), None, None);

        assert_eq!(
            payload.embeds.as_ref().unwrap()[0]
                .thumbnail
                .as_ref()
                .map(|t| t.url.as_str()),
            Some("https://img/first.jpg")
        );
    }

    #[test]
    fn omits_the_thumbnail_when_there_are_no_photos() {
        let payload = offer_payload(&product(vec![], vec![]), None, None);
        assert!(payload.embeds.as_ref().unwrap()[0].thumbnail.is_none());
    }

    #[test]
    fn includes_reviews_and_amazon_only_when_known() {
        let bare = offer_payload(&product(vec![], vec![]), None, None);
        assert!(field(&bare, "Reviews").is_none());
        assert!(field(&bare, "Amazon").is_none());

        let full = offer_payload(&product(vec![], vec![]), Some(317), Some("B08N5WRWNW"));
        assert_eq!(field(&full, "Reviews").unwrap().value, "317");
        assert_eq!(
            field(&full, "Amazon").unwrap().value,
            "[B08N5WRWNW](https://www.amazon.com/dp/B08N5WRWNW)"
        );
    }

    #[test]
    fn always_offers_an_ebay_search_with_the_title_encoded() {
        let payload = offer_payload(&product(vec![], vec![]), None, None);
        assert!(field(&payload, "Ebay")
            .unwrap()
            .value
            .contains("_nkw=Anker%20USB-C%20Cable"));
    }
}
