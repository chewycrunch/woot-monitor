use super::product::{Product, Variant};
use super::woot_api::WootOffer;

/// Converts a price in dollars to whole cents, rounding because `19.99 * 100.0`
/// is `1998.999...`. `u32` rather than `u16`: 65535 cents is only $655.35.
fn to_cents(dollars: f64) -> u32 {
    (dollars * 100.0).round() as u32
}

impl From<WootOffer> for Product {
    fn from(woot_offer: WootOffer) -> Self {
        let items = &woot_offer.items;
        let photos = woot_offer.photos.unwrap_or_default();

        let variants = items
            .iter()
            .map(|item| match &item.attributes {
                Some(attrs) => {
                    let filtered_attrs: Vec<String> = attrs
                        .iter()
                        .filter(|a| {
                            !(a.key.to_lowercase() == "condition"
                                && a.value.to_lowercase() == "new")
                        })
                        .map(|a| a.value.clone())
                        .collect();

                    let attrs = if filtered_attrs.is_empty() {
                        None
                    } else {
                        Some(filtered_attrs.join(" / "))
                    };

                    Variant {
                        attrs,
                        list_price: item.list_price.map(to_cents),
                        sale_price: Some(to_cents(item.sale_price)),
                    }
                }
                None => Variant {
                    attrs: None,
                    list_price: item.list_price.map(to_cents),
                    sale_price: Some(to_cents(item.sale_price)),
                },
            })
            .collect();

        let condition = items
            .first()
            .and_then(|item| {
                item.attributes.as_ref().and_then(|attrs| {
                    attrs
                        .iter()
                        .find(|a| a.key.to_lowercase() == "condition")
                        .map(|a| a.value.clone())
                })
            })
            .unwrap_or_else(|| "Unknown".to_string());

        let (list_price, sale_price) = items.first().map_or((None, None), |item| {
            (
                item.list_price.map(to_cents),
                Some(to_cents(item.sale_price)),
            )
        });

        Product {
            id: woot_offer.id,
            out_of_stock: woot_offer.sold_out,
            title: woot_offer.title,
            start_date: woot_offer.start_date,
            end_date: woot_offer.end_date,
            list_price,
            sale_price,
            condition,
            slug: woot_offer.slug,
            photos,
            variants,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::woot_api::{Item, Photo, WootAttribute};
    use chrono::TimeZone;

    fn attr(key: &str, value: &str) -> WootAttribute {
        WootAttribute {
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    fn item(
        list_price: Option<f64>,
        sale_price: f64,
        attributes: Option<Vec<WootAttribute>>,
    ) -> Item {
        Item {
            list_price,
            sale_price,
            attributes,
        }
    }

    fn offer(items: Vec<Item>, photos: Option<Vec<Photo>>) -> WootOffer {
        WootOffer {
            id: "1234".to_string(),
            sold_out: false,
            title: "Anker USB-C Cable".to_string(),
            photos,
            start_date: chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            end_date: chrono::Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            items,
            slug: "anker-usb-c-cable".to_string(),
        }
    }

    // @spec ROUTING-040
    #[test]
    fn carries_the_offer_identity_across() {
        let product = Product::from(offer(vec![item(None, 9.99, None)], None));

        assert_eq!(product.id, "1234");
        assert_eq!(product.title, "Anker USB-C Cable");
        assert_eq!(product.slug, "anker-usb-c-cable");
        assert!(!product.out_of_stock);
    }

    // @spec ROUTING-002
    #[test]
    fn maps_sold_out_to_out_of_stock() {
        let mut woot_offer = offer(vec![item(None, 9.99, None)], None);
        woot_offer.sold_out = true;

        assert!(Product::from(woot_offer).out_of_stock);
    }

    // @spec ROUTING-043
    #[test]
    fn treats_missing_photos_as_none_rather_than_failing() {
        assert!(Product::from(offer(vec![], None)).photos.is_empty());
    }

    // @spec ROUTING-043
    #[test]
    fn keeps_photos_in_order() {
        let photos = vec![
            Photo {
                url: "https://img/a.jpg".to_string(),
            },
            Photo {
                url: "https://img/b.jpg".to_string(),
            },
        ];
        let product = Product::from(offer(vec![], Some(photos)));

        assert_eq!(product.photos[0].url, "https://img/a.jpg");
        assert_eq!(product.photos[1].url, "https://img/b.jpg");
    }

    // @spec ROUTING-040
    #[test]
    fn reads_condition_from_the_first_item() {
        let attrs = Some(vec![attr("Condition", "Refurbished")]);
        let product = Product::from(offer(vec![item(None, 9.99, attrs)], None));

        assert_eq!(product.condition, "Refurbished");
    }

    // @spec ROUTING-040
    #[test]
    fn matches_the_condition_key_case_insensitively() {
        let attrs = Some(vec![attr("CONDITION", "Scratch and Dent")]);
        let product = Product::from(offer(vec![item(None, 9.99, attrs)], None));

        assert_eq!(product.condition, "Scratch and Dent");
    }

    // @spec ROUTING-040
    #[test]
    fn falls_back_to_unknown_condition() {
        let attrs = Some(vec![attr("Color", "Black")]);
        let product = Product::from(offer(vec![item(None, 9.99, attrs)], None));

        assert_eq!(product.condition, "Unknown");
    }

    // @spec ROUTING-040
    #[test]
    fn falls_back_to_unknown_condition_when_there_are_no_items() {
        let product = Product::from(offer(vec![], None));

        assert_eq!(product.condition, "Unknown");
        assert_eq!(product.list_price, None);
        assert_eq!(product.sale_price, None);
    }

    // @spec ROUTING-042
    #[test]
    fn drops_a_new_condition_from_the_variant_label() {
        let attrs = Some(vec![attr("Condition", "New")]);
        let product = Product::from(offer(vec![item(None, 9.99, attrs)], None));

        // "New" is the default and would be noise on every single variant.
        assert_eq!(product.variants[0].attrs, None);
    }

    // @spec ROUTING-042
    #[test]
    fn keeps_a_non_new_condition_in_the_variant_label() {
        let attrs = Some(vec![attr("Condition", "Refurbished")]);
        let product = Product::from(offer(vec![item(None, 9.99, attrs)], None));

        assert_eq!(product.variants[0].attrs.as_deref(), Some("Refurbished"));
    }

    // @spec ROUTING-042
    #[test]
    fn joins_multiple_attributes_with_a_slash() {
        let attrs = Some(vec![
            attr("Color", "Black"),
            attr("Condition", "New"),
            attr("Size", "2m"),
        ]);
        let product = Product::from(offer(vec![item(None, 9.99, attrs)], None));

        assert_eq!(product.variants[0].attrs.as_deref(), Some("Black / 2m"));
    }

    // @spec ROUTING-042
    #[test]
    fn labels_a_variant_with_no_attributes_as_none() {
        let product = Product::from(offer(vec![item(None, 9.99, None)], None));

        assert_eq!(product.variants[0].attrs, None);
    }

    // @spec ROUTING-044
    #[test]
    fn converts_dollars_to_whole_cents() {
        let product = Product::from(offer(vec![item(Some(24.99), 12.50, None)], None));

        assert_eq!(product.variants[0].list_price, Some(2499));
        assert_eq!(product.variants[0].sale_price, Some(1250));
    }

    // @spec ROUTING-040
    #[test]
    fn lifts_the_first_item_prices_onto_the_product() {
        let items = vec![item(Some(24.99), 12.50, None), item(Some(9.99), 5.00, None)];
        let product = Product::from(offer(items, None));

        assert_eq!(product.list_price, Some(2499));
        assert_eq!(product.sale_price, Some(1250));
    }

    // @spec ROUTING-042
    #[test]
    fn builds_one_variant_per_item() {
        let items = vec![item(None, 9.99, None), item(None, 5.00, None)];

        assert_eq!(Product::from(offer(items, None)).variants.len(), 2);
    }

    // @spec ROUTING-044
    #[test]
    fn rounds_prices_that_float_math_lands_just_under() {
        // $19.99 * 100.0 is 1998.9999999999998 in f64; truncating loses a cent.
        let product = Product::from(offer(vec![item(Some(1.15), 19.99, None)], None));

        assert_eq!(product.variants[0].sale_price, Some(1999));
        assert_eq!(product.variants[0].list_price, Some(115));
    }

    // @spec ROUTING-044
    #[test]
    fn keeps_prices_above_the_old_u16_ceiling() {
        // 65535 cents is $655.35, which u16 used to clamp everything down to.
        let product = Product::from(offer(vec![item(Some(1234.56), 700.00, None)], None));

        assert_eq!(product.variants[0].list_price, Some(123456));
        assert_eq!(product.variants[0].sale_price, Some(70000));
    }

    // @spec ROUTING-044
    #[test]
    fn rounds_a_half_cent_up() {
        let product = Product::from(offer(vec![item(None, 0.125, None)], None));

        assert_eq!(product.variants[0].sale_price, Some(13));
    }
}
