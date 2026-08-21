use super::product::{Product, Variant};
use super::woot_api::WootOffer;

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
                        list_price: item.list_price.map(|p| (p * 100.0) as u16),
                        sale_price: Some((item.sale_price * 100.0) as u16),
                    }
                }
                None => Variant {
                    attrs: None,
                    list_price: item.list_price.map(|p| (p * 100.0) as u16),
                    sale_price: Some((item.sale_price * 100.0) as u16),
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
            let lp = item.list_price.map(|p| (p * 100.0) as u16);
            let sp = Some((item.sale_price * 100.0) as u16);
            (lp, sp)
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
