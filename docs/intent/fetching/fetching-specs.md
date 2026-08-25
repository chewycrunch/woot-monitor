# Fetching — Specs

## Proxy rotation

- [x] **FETCHING-001**: On startup, the system shall read its proxy list from a file, accepting entries as host and port, or host, port, username and password.
- [x] **FETCHING-002**: When a proxy list entry does not parse, the system shall skip it and continue reading the remaining entries.
- [x] **FETCHING-003**: If the proxy list file is absent or empty, then the system shall proceed with no proxies and issue requests directly.
- [x] **FETCHING-004**: On startup, the system shall report how many proxies it read and the file it read them from.
- [x] **FETCHING-005**: The system shall select proxies in round-robin order, advancing on each request issued rather than on each request that succeeds.
- [x] **FETCHING-006**: The system shall maintain the rotation used for catalogue and page reads separately from the rotation used for notification delivery.

## Egress paths

- [x] **FETCHING-010**: The system shall request the search API directly through a proxy.
- [x] **FETCHING-011**: The system shall request offer pages through the tls-client sidecar, which performs them with a browser TLS signature.
- [x] **FETCHING-012**: The system shall deliver notifications directly through a proxy.
- [x] **FETCHING-013**: When requesting through the sidecar, the system shall pass the target URL, the headers to send, and the proxy to use.
- [x] **FETCHING-014**: When requesting through the sidecar, the system shall present the configured sidecar auth key.
- [x] **FETCHING-015**: The system shall express a proxy to the sidecar as a URL carrying any credentials between the scheme and the host, with the scheme appearing exactly once.
- [x] **FETCHING-017**: When a request through the sidecar completes, the system shall release the session the sidecar opened for it.
- [x] **FETCHING-016**: The system shall present a browser identity whose user-agent and client-hint headers name the same browser version.

## Page extraction

- [ ] **FETCHING-025**: While no webhook endpoint is configured on any entry, the system shall not request offer pages.
- [x] **FETCHING-020**: When an offer page is retrieved, the system shall extract the product identity and the total review count from the page text.
- [x] **FETCHING-021**: The system shall extract the product identity only from within the page's rating summary data, so that an identifier belonging to another part of the page is not taken.
- [x] **FETCHING-022**: The system shall extract values whether the page arrives decoded or JSON-escaped within the sidecar's response envelope.
- [x] **FETCHING-023**: If a page yields no product identity or no review count, then the system shall report the value as absent rather than as an error.
- [x] **FETCHING-024**: If an offer page cannot be retrieved, then the system shall report the failure, treat both values as absent, and continue processing the offer.
