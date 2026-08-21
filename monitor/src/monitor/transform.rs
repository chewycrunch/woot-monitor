use super::product::{Product, Variant};
use super::woot_api::WootOffer;

/// Converts a price in dollars to whole cents.
///
/// Two known defects, pinned by the `bug_*` tests below:
///
/// 1. It truncates instead of rounding, and `f64` cannot represent most
///    two-decimal prices exactly — `19.99 * 100.0` is `1998.999...`, so $19.99
///    becomes 1998 cents and renders as "$19.98".
/// 2. `u16` tops out at 65535 cents ($655.35). Float-to-int casts saturate
///    rather than wrap, so a $1,234.56 item silently clamps to $655.35.
///
/// Fixing both means rounding and widening the cent type past `u16`, which
/// ripples into `Product`, `Variant` and `notify::price_label`.
fn to_cents(dollars: f64) -> u16 {
    (dollars * 100.0) as u16
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
            .get(0)
            .and_then(|item| {
                item.attributes.as_ref().and_then(|attrs| {
                    attrs
                        .iter()
                        .find(|a| a.key.to_lowercase() == "condition")
                        .map(|a| a.value.clone())
                })
            })
            .unwrap_or_else(|| "Unknown".to_string());

        let (list_price, sale_price) = items.get(0).map_or((None, None), |item| {
            (
                item.list_price.map(to_cents),
                Some(to_cents(item.sale_price)),
            )
        });

        Product {
            id: woot_offer.id,
            out_of_stock: woot_offer.sold_out,
            title: woot_offer.title,
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
            end_date: chrono::Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            items,
            slug: "anker-usb-c-cable".to_string(),
        }
    }

    #[test]
    fn carries_the_offer_identity_across() {
        let product = Product::from(offer(vec![item(None, 9.99, None)], None));

        assert_eq!(product.id, "1234");
        assert_eq!(product.title, "Anker USB-C Cable");
        assert_eq!(product.slug, "anker-usb-c-cable");
        assert!(!product.out_of_stock);
    }

    #[test]
    fn maps_sold_out_to_out_of_stock() {
        let mut woot_offer = offer(vec![item(None, 9.99, None)], None);
        woot_offer.sold_out = true;

        assert!(Product::from(woot_offer).out_of_stock);
    }

    #[test]
    fn treats_missing_photos_as_none_rather_than_failing() {
        assert!(Product::from(offer(vec![], None)).photos.is_empty());
    }

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

    #[test]
    fn reads_condition_from_the_first_item() {
        let attrs = Some(vec![attr("Condition", "Refurbished")]);
        let product = Product::from(offer(vec![item(None, 9.99, attrs)], None));

        assert_eq!(product.condition, "Refurbished");
    }

    #[test]
    fn matches_the_condition_key_case_insensitively() {
        let attrs = Some(vec![attr("CONDITION", "Scratch and Dent")]);
        let product = Product::from(offer(vec![item(None, 9.99, attrs)], None));

        assert_eq!(product.condition, "Scratch and Dent");
    }

    #[test]
    fn falls_back_to_unknown_condition() {
        let attrs = Some(vec![attr("Color", "Black")]);
        let product = Product::from(offer(vec![item(None, 9.99, attrs)], None));

        assert_eq!(product.condition, "Unknown");
    }

    #[test]
    fn falls_back_to_unknown_condition_when_there_are_no_items() {
        let product = Product::from(offer(vec![], None));

        assert_eq!(product.condition, "Unknown");
        assert_eq!(product.list_price, None);
        assert_eq!(product.sale_price, None);
    }

    #[test]
    fn drops_a_new_condition_from_the_variant_label() {
        let attrs = Some(vec![attr("Condition", "New")]);
        let product = Product::from(offer(vec![item(None, 9.99, attrs)], None));

        // "New" is the default and would be noise on every single variant.
        assert_eq!(product.variants[0].attrs, None);
    }

    #[test]
    fn keeps_a_non_new_condition_in_the_variant_label() {
        let attrs = Some(vec![attr("Condition", "Refurbished")]);
        let product = Product::from(offer(vec![item(None, 9.99, attrs)], None));

        assert_eq!(product.variants[0].attrs.as_deref(), Some("Refurbished"));
    }

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

    #[test]
    fn labels_a_variant_with_no_attributes_as_none() {
        let product = Product::from(offer(vec![item(None, 9.99, None)], None));

        assert_eq!(product.variants[0].attrs, None);
    }

    #[test]
    fn converts_dollars_to_whole_cents() {
        let product = Product::from(offer(vec![item(Some(24.99), 12.50, None)], None));

        assert_eq!(product.variants[0].list_price, Some(2499));
        assert_eq!(product.variants[0].sale_price, Some(1250));
    }

    #[test]
    fn lifts_the_first_item_prices_onto_the_product() {
        let items = vec![item(Some(24.99), 12.50, None), item(Some(9.99), 5.00, None)];
        let product = Product::from(offer(items, None));

        assert_eq!(product.list_price, Some(2499));
        assert_eq!(product.sale_price, Some(1250));
    }

    #[test]
    fn builds_one_variant_per_item() {
        let items = vec![item(None, 9.99, None), item(None, 5.00, None)];

        assert_eq!(Product::from(offer(items, None)).variants.len(), 2);
    }

    // --- Characterization tests: these pin CURRENT behaviour, which is wrong.
    // See the note above `to_cents`. Fixing the conversion should flip both.

    #[test]
    fn bug_loses_a_cent_when_float_math_lands_just_under() {
        // $19.99 * 100.0 is 1998.9999999999998 in f64, and `as u16` truncates.
        let product = Product::from(offer(vec![item(None, 19.99, None)], None));

        assert_eq!(product.variants[0].sale_price, Some(1998));
    }

    #[test]
    fn bug_saturates_prices_above_the_u16_ceiling() {
        // u16 tops out at 65535 cents, i.e. $655.35. Float-to-int casts
        // saturate rather than wrap, so anything dearer silently clamps.
        let product = Product::from(offer(vec![item(Some(1234.56), 700.00, None)], None));

        assert_eq!(product.variants[0].list_price, Some(65535));
        assert_eq!(product.variants[0].sale_price, Some(65535));
    }
}
