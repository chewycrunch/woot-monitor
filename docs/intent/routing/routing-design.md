---
parent: high-level-design
prefix: ROUTING
---

# Routing

## Context and Design Philosophy

Routing decides which Discord channels a new offer reaches and what the notification says. It receives offers already established as novel and answers one question per configured webhook: does this offer belong here?

The design leans on the project's preference for a false alarm over a missed offer. Where a rule could be drawn tightly or loosely, routing draws it loosely — an unwanted notification costs a glance, a suppressed one costs the deal.

## Channels

Each configured webhook entry may declare up to three endpoints. Two are exclusive; the third is independent.

| Channel | Receives |
|---|---|
| `verified` | Offers whose product identity and review count could be established, and whose review count meets the minimum |
| `unverified` | Every other offer |
| `watchlist` | Offers matching the entry's configured keywords or ASINs |

`verified` and `unverified` partition the offers: every notified offer reaches exactly one. `watchlist` overlaps both — an offer can arrive on `watchlist` and on either of the other two.

`watchlist` is deliberately not gated on verification. A keyword match is the operator asking to be told about something specific; suppressing it because the offer's page yielded no product identity would substitute the system's judgement for an explicit request.

## Verification

An offer is verified when it carries a product identity (ASIN) *and* a review count at or above the minimum. Both values come from scraping the offer's page, not from the search API.

The consequence worth stating plainly: when the page scrape fails, both values are absent and the offer routes to `unverified`. A scraping outage therefore presents as a sudden shift of all traffic to `unverified` rather than as an error — the channel means "could not establish", not "established as poor".

## Watchlist matching

Keywords match on word boundaries within the offer title. Both the title and each keyword are reduced to their lowercased alphanumeric words, joined by single spaces and padded with a space at each end; a keyword matches when its reduced form occurs in the title's.

This shape is chosen so that multi-word keywords work — the reduced form of a phrase is a contiguous string — while a keyword cannot match inside a longer word. A plain substring test would fire a keyword on any title containing its letters consecutively; a whole-word set membership test would make every multi-word keyword unmatchable.

Punctuation is a word break rather than a deletion, so two spellings of a punctuated brand reduce to different forms and a configuration wanting both must list both.

A keyword reducing to nothing — punctuation only — would match every title. Such keywords are discarded when the configuration is registered rather than being allowed to match everything.

ASINs match exactly, case-insensitively, against the identity scraped from the offer page.

## Delivery

Channels are delivered independently. A failure on one does not prevent or alter delivery to another, is logged, and is not retried — the offer is not re-offered by a later poll, so a dropped notification is lost rather than duplicated.

Endpoints are addressed as configured, not deduplicated. The same endpoint declared by two entries receives one notification per entry; whether that is wanted is the operator's call.

An entry declaring keywords or ASINs without a watchlist endpoint is inert — omitting an endpoint is how a channel is turned off.

## Notification content

A notification carries what a reader needs to decide without opening the page: title, condition, list and sale price, the offer's photos, its variants where they differ in price or attributes, and the review count and product identity when known. Prices are held in whole cents.

## Suppression

An offer that is already sold out when first seen is recorded as seen but not notified. Novelty and notability are distinct: recording prevents a later re-notification, while notifying about something already unavailable is noise.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Channel structure | Two exclusive tiers plus an independent watchlist | Three exclusive channels; one channel with severity | The tiers answer "is this credible?" and the watchlist answers "did I ask for this?" — different questions, so an offer can warrant both. |
| Watchlist gating | Ungated on verification | Only notify verified watchlist matches | A keyword match is an explicit request; a failed page scrape is not a reason to withhold it. |
| Keyword matching | Word-boundary match on reduced forms | Whole-word set membership; plain substring | Set membership makes multi-word keywords dead; plain substring matches inside longer words. |
| Punctuation | Word break | Stripped before comparison | Stripping would conflate distinct brand spellings; a break keeps them separate and lets a configuration list both. |
| Empty keywords | Discarded at registration | Allowed to match everything; rejected as a config error | A keyword reducing to nothing is a typo, not an intent to match all offers; discarding is quieter than failing to start. |
| Sold-out offers | Recorded, not notified | Notified; not recorded | Recording prevents re-notification later; notifying about something unavailable is noise. [inferred] |
| Delivery failures | Logged and dropped, per channel | Retried; queued; abort remaining channels | Nothing can be done about a rejected webhook, and stalling or aborting would cost the channels that would have succeeded. |
| Duplicate endpoints | Sent once per declaring entry | Deduplicated across entries | Which entries feed an endpoint is the operator's arrangement to make. |
| Channel with no endpoint | Silently inert | Warn; reject as a config error | Omitting an endpoint is the means of disabling a channel, so it is ordinary rather than exceptional. |

## Open Questions & Future Decisions

### Resolved
1. ✅ The verified/unverified split is exclusive and the watchlist is independent.
2. ✅ Keyword matching respects word boundaries and supports multi-word keywords.

### Deferred
1. A scraping outage routes all traffic to `unverified` with no distinct signal. Whether that warrants its own alert is unsettled.
2. Delivery failures are dropped silently. Discord rate limits during a large batch of new offers are the likely cause and are not currently visible.
3. An entry configuring keywords but no watchlist endpoint is inert with no signal. A startup warning would make the dead configuration visible.
3. The review-count minimum is a single global threshold, not per-webhook.

## References

- `docs/high-level-design.md` — segment boundaries and tenets.
- `docs/intent/fetching/fetching-design.md` — where the ASIN and review count come from.
- `docs/intent/config/config-design.md` — how webhook entries are declared.
