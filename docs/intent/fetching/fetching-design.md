---
parent: high-level-design
prefix: FETCHING
---

# Fetching

## Context and Design Philosophy

Fetching owns every outbound request the monitor makes: how it reaches the network, which identity it presents, and what it extracts from what comes back. It exists because Woot treats a plain HTTP client as a bot, and because a single source address is rate-limited long before the polling cadence would be.

Two distinct obstacles shape the design. Rate limiting is answered by rotating source addresses. Fingerprinting is answered by delegating those requests to a process that can present a browser's TLS signature.

## Proxy rotation

A proxy list is read once at startup from a file, one entry per line, as either `host:port` or `host:port:user:pass`. Lines that parse as neither are skipped.

Rotation is round-robin: each request takes the next proxy and advances the cursor. Two independent rotations are maintained — one for catalogue and page reads, one for notification delivery — so notification traffic does not consume the read rotation's position.

A missing or empty proxy file is not an error. It yields an empty list, and requests are made directly. The count read is reported at startup, which is the only signal distinguishing "no proxies configured" from "proxy file not found where expected".

## Egress paths

Requests take one of two paths depending on whether the destination fingerprints its clients.

| Destination | Path | Why |
|---|---|---|
| Search API | Direct, proxied | Served by a CDN that does not fingerprint |
| Offer pages | Through the sidecar, proxied | Rejects clients whose TLS signature is not a browser's |
| Discord webhooks | Direct, proxied | No fingerprinting |

The sidecar is a forwarding service, not a client of the domain. It accepts a target URL, a header set, and a proxy, performs the request with a browser TLS signature, and returns the response body wrapped in an envelope. It holds no offer state.

Requests to the sidecar carry a shared auth key. The sidecar's own key list and the monitor's key are supplied from one value at deployment so they cannot drift apart; the key baked into the sidecar image is a default that a deployment replaces rather than adds to.

Proxies are expressed differently on the two paths. The direct path builds a client-native proxy with credentials attached separately. The sidecar path passes a URL string, with credentials embedded between the scheme and the host — the scheme appears once, and the sidecar accepts the string without validating it, so a malformed URL degrades to an unproxied request rather than an error.

## Browser identity

Requests present a consistent browser identity: a `User-Agent`, matching client-hint headers, and the fetch-metadata headers a real navigation would carry. The version named in the user agent and the version named in the client hints travel as a set — a mismatch between them is itself a fingerprint.

## Page extraction

Two values are extracted from an offer page: the product identity (ASIN) and the total review count. Both are embedded in inline script JSON rather than markup, so extraction matches against the raw page text.

The page may arrive decoded or still JSON-escaped inside the sidecar's envelope, so the patterns tolerate an optional backslash before each quote.

Both values are optional, and their absence is ordinary rather than exceptional: the two live in the same block of page data and share fate, and roughly a sixth of offers are products with no Amazon listing behind them, yielding neither. Absence therefore carries no diagnostic weight on its own and is not treated as an error — it propagates to routing, where the offer notifies as unverified.

## Doing no work for nobody

Enrichment exists to inform routing. Where a deployment configures no endpoint at all, nothing consumes the result, and requesting offer pages would spend proxy budget and sidecar capacity on values that are discarded. Offer pages are therefore requested only where some channel could receive what they yield.

## Failure behaviour

A failed page fetch is logged and yields absent values. It does not interrupt the polling cycle, and the offer still notifies — routing treats the missing values as unverified.

This is the segment's quietest failure, and deliberately so: an offer notified without a review count is better than an offer not notified. The cost is that a sidecar outage is invisible except as a shift in routing.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| TLS fingerprinting | Sidecar process | In-process library; reimplementation | Mature implementations are Go libraries with no Rust equivalent. A sidecar confines the dependency behind an HTTP contract. |
| Sidecar scope | Only fingerprinted requests | All outbound traffic | Routing everything through the sidecar adds a hop and a failure mode to requests that do not need it. |
| Proxy rotation | Round-robin, separate rotations per purpose | Random; single shared rotation; per-host affinity | Round-robin spreads load predictably; separate rotations keep notification volume from displacing read positions. [inferred] |
| Missing proxy file | Empty list, direct requests | Fail to start | A proxy list is optional for local runs. The startup count is the signal that it was not found. [inferred] |
| Sidecar auth key | One deployment value feeding both containers | Independent keys; no auth | Two independently configured values for one shared secret drift; the sidecar's env replaces its baked list rather than extending it. |
| Enrichment with no channels configured | Skipped | Always enrich, for uniformity | Nothing consumes the result, and the requests are the most expensive the system makes. |
| Page fetch failure | Log and continue with absent values | Retry; fail the offer | The offer is still worth notifying; verification is an enrichment, not a precondition. |
| Failure kinds | Not distinguished — every absence routes the offer to unverified | Treat a non-success response as an error; check for the surrounding page marker to separate a legitimately absent value from an unrecognised page; alarm on a collapsing success rate | The outcome is the same whichever kind occurred, and absence is common enough at baseline that separating the kinds buys diagnosis rather than behaviour. |
| Proxy cursor | Advances on issue, not on success | Advance only on success; retire failing proxies | Keeps rotation uniform and stateless; a failing proxy costs the requests that draw it rather than being detected. |

## Open Questions & Future Decisions

### Resolved
1. ✅ Only offer pages need the sidecar; the search API and Discord do not.
2. ✅ The sidecar's auth key is replaced, not extended, by its environment — verified against the published image.
3. ✅ Extraction failures of every kind — transport error, unsuccessful response, unrecognised page, or a product with no Amazon data — are treated alike and route the offer to unverified.
4. ✅ Rotation advances on issue rather than success; a failing proxy is not retired.

### Deferred
1. A sidecar outage is visible only as a routing shift, since a failed enrichment is indistinguishable from a product that has none.
2. The proxy list is read once at startup; changing it requires a restart.
3. Extraction patterns are pinned to Woot's current page structure. A change to it presents as every offer becoming unverified, not as an error.

## References

- `docs/high-level-design.md` — segment boundaries and tenets.
- [bogdanfinn/tls-client-api](https://github.com/bogdanfinn/tls-client-api) — the sidecar's upstream.
- `docs/intent/routing/routing-design.md` — how absent extraction values are treated.
