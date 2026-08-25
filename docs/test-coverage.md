# Specs without tests

23 of the 78 implemented specs are verified by a test. The other 55 are cited only by the code that realizes them.

| Segment | Implemented | Tested | Untested |
|---|---|---|---|
| detection | 23 | 4 | 19 |
| routing | 22 | 9 | 13 |
| fetching | 19 | 5 | 14 |
| config | 14 | 5 | 9 |

This is a snapshot. To recheck, compare the `**ID**` entries in each `docs/intent/*/‌*-specs.md` against the `// @spec` annotations sitting above `#[test]` functions in `monitor/src`.

## Why the gap is shaped this way

The tests cluster around functions that take values and return values — price conversion, keyword matching, the paging boundaries. Everything reached through a network call is untested, because the behaviour and the I/O carrying it live in the same function.

That makes the coverage figure flatter the code. Watchlist keyword matching has seven tests; the routing decision consuming it has none. The well-tested part is the part that was easy to test, not the part most likely to be wrong — every defect found in this codebase so far has been in polling or in routing, both in the untested majority.

## Needs no design change

Reachable from a test today; they lack tests, not testability.

**Proxy list handling** — `FETCHING-001`, `FETCHING-002`, `FETCHING-003`, `FETCHING-005`
Parsing entries from a file, skipping malformed lines, tolerating an absent file, rotating round-robin. A temporary file and a handful of assertions.

**Browser identity** — `FETCHING-016`
The header sets are built by functions returning maps. The assertion is that the user-agent and the client-hint headers name the same browser version.

**Novelty and the high-water mark** — `DETECTION-030`, `DETECTION-032`, `DETECTION-033`, `DETECTION-040`, `DETECTION-041`
Recording an offer, never removing one, ignoring changes to a recorded offer, advancing the mark only forwards.

**Configuration edges** — `CONFIG-006`, `CONFIG-011`, `CONFIG-013`, `CONFIG-020`, `CONFIG-021`
Webhook entries coming only from the file, an empty list being accepted, omitted fields being absent rather than errors, a malformed file yielding an error, unrecognised keys being ignored. The existing configuration tests are the pattern.

## Needs a seam first

Each needs a way to supply an input or observe an output currently reached only through the network.

**The routing decision** — `ROUTING-001`, `ROUTING-010`–`ROUTING-013`, `ROUTING-020`–`ROUTING-022`, `ROUTING-026`, `ROUTING-030`–`ROUTING-033`
Deciding which channels an offer reaches and delivering to them happen in one function, so the decision cannot be exercised without sending. Separating *which endpoints match* from *sending to them* makes the first half a pure function over an offer and a configuration, leaving only delivery needing a fake.

The largest and most valuable group: the whole of routing's decision logic, entirely unverified.

**Catalogue paging** — `DETECTION-001`, `DETECTION-002`, `DETECTION-004`, `DETECTION-005`, `DETECTION-010`, `DETECTION-013`, `DETECTION-020`, `DETECTION-023`–`DETECTION-025`
The paging loop issues its own requests, so page sequences cannot be supplied. The boundary arithmetic is already tested in isolation; the loop using it is not — whether it stops at a short page, what it does when the cutoff is never reached, and that a failed read leaves recorded offers untouched. A boundary for "fetch one page" makes the loop testable against scripted sequences.

**The sidecar contract** — `FETCHING-010`–`FETCHING-014`, `FETCHING-017`, `FETCHING-024`
What is sent to the sidecar, that the session is released afterwards, and that a failed page read yields absent values without interrupting the poll.

**Log-carried behaviour** — `DETECTION-012`, `DETECTION-031`, `DETECTION-050`, `DETECTION-051`, `CONFIG-030`, `FETCHING-004`, `FETCHING-006`
These specify what the system reports rather than what it computes. Verifying them means capturing tracing output — a smaller seam than the others, but still deliberate.

**Outside the unit-test boundary** — `CONFIG-001`, `CONFIG-004`, `CONFIG-040`
Reading from a fixed path, loading a local environment file at startup, and the published image containing no configuration. Properties of the process and the image, verifiable by starting a container rather than from this crate.

## Suggested order

1. The group needing no design change — closes the cheap distance.
2. The routing seam — most unverified intent per unit of work.
3. The paging seam — more invasive, covers the weakest segment.

Log-carried and image-level behaviour is worth deferring until those are done; it verifies reporting rather than decisions.

## Separately: specs with no implementation

Nine specs describe intent the code does not yet have. These are gaps in the arrow rather than in its testing, and are marked `[ ]` in their spec files.

- `DETECTION-060`–`DETECTION-064` — the liveness timestamp and the monitor image's health check
- `CONFIG-005` — an empty environment variable should mean unset; it is currently taken literally
- `CONFIG-023` — a malformed webhook endpoint should be rejected at load rather than failing on every delivery
- `CONFIG-041` — the monitor image should declare a health check
- `FETCHING-025` — offer pages should not be requested when no endpoint is configured to consume the result
