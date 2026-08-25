# Routing — Specs

## Notability

- [x] **ROUTING-001**: When an offer is new and not already sold out, the system shall notify the channels it matches.
- [x] **ROUTING-002**: When an offer is new and already sold out, the system shall record it without notifying any channel.

## Verification tiers

- [x] **ROUTING-010**: The system shall treat an offer as verified when it carries a product identity and a review count at or above the minimum the system defines.
- [x] **ROUTING-011**: When a notified offer is verified, the system shall deliver it to every configured verified endpoint and to no unverified endpoint.
- [x] **ROUTING-012**: When a notified offer is not verified, the system shall deliver it to every configured unverified endpoint and to no verified endpoint.
- [x] **ROUTING-013**: If an offer's product identity or review count could not be established, then the system shall treat the offer as not verified.

## Watchlist

- [x] **ROUTING-020**: When a notified offer's title matches a keyword configured on a webhook entry, the system shall deliver it to that entry's watchlist endpoint.
- [x] **ROUTING-021**: When a notified offer's product identity — read from the offer page, and absent whenever that read fails — matches an ASIN configured on a webhook entry, the system shall deliver it to that entry's watchlist endpoint.
- [x] **ROUTING-022**: The system shall deliver a watchlist match regardless of whether the offer is verified.
- [x] **ROUTING-023**: The system shall compare a keyword against an offer title by reducing both to their lowercased alphanumeric words, joined and bounded by single spaces, and testing whether the keyword's reduction occurs in the title's.
- [x] **ROUTING-024**: The system shall treat punctuation within a keyword or title as a word break rather than removing it.
- [x] **ROUTING-025**: When a configured keyword reduces to no words, the system shall discard it rather than matching every offer.
- [x] **ROUTING-026**: The system shall compare configured ASINs to an offer's product identity without regard to case.

## Delivery

- [x] **ROUTING-030**: The system shall deliver to each matched endpoint independently.
- [x] **ROUTING-031**: If delivery to an endpoint fails, then the system shall report the failure, leave delivery to every other matched endpoint unaffected, and not retry.
- [x] **ROUTING-032**: Where two webhook entries declare the same endpoint, the system shall deliver once for each entry rather than deduplicating.
- [x] **ROUTING-033**: Where a webhook entry omits an endpoint, the system shall deliver nothing for that channel and treat the entry's other channels normally.

## Notification content

- [x] **ROUTING-040**: The system shall include the offer's title, condition, list price, sale price, and a link to its page in every notification.
- [x] **ROUTING-041**: Where an offer's product identity or review count is known, the system shall include it in the notification.
- [x] **ROUTING-042**: Where an offer has variants differing in price or attributes, the system shall include them in the notification.
- [x] **ROUTING-043**: Where an offer carries photos, the system shall include them in the notification.
- [x] **ROUTING-044**: The system shall hold prices as whole cents, rounding rather than truncating when converting from the fractional amounts Woot reports.
