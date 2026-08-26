---
parent: high-level-design
prefix: DETECTION
---

# Detection

## Context and Design Philosophy

Detection decides *when* the monitor looks at Woot, *how much* of the catalogue it reads, and *what counts as new*. Everything downstream — routing, notification — is driven by the set of offers this segment declares novel.

Woot offers no change feed, so novelty is established by difference: read the catalogue, compare against what has been seen, treat the remainder as new. The design problem is that the catalogue is larger than the search API will paginate and larger than the proxy budget wants to read every few seconds. Detection therefore reads a bounded prefix of the catalogue rather than all of it, and the correctness of that bound is this segment's central concern.

## Catalogue ordering

The search API is queried with `Sort: NewestFirst`, which orders offers by `StartDate` descending. Three properties of that ordering are load-bearing and have been verified against the live service across the full reachable range:

- **Monotonic.** `StartDate` never increases as the offset grows.
- **Stable.** The same offset returns the same offers on repeat requests, and a single large page equals the concatenation of the smaller pages covering it.
- **Coarse.** Offers are listed in batches sharing one timestamp; a batch may hold several hundred offers, and order *within* a batch is arbitrary.

Monotonicity is what makes a date-bounded read sound: all offers at or after any given timestamp form a contiguous prefix. Coarseness is what makes a novelty-bounded read unsound — see *Bounding a poll*.

## Search depth ceiling

Woot rejects any search whose `Skip + Limit` exceeds 10,000, inclusively: `Skip: 9800, Limit: 200` succeeds and `Skip: 10000, Limit: 200` fails the whole request rather than returning a short page. With a page size of 200 the deepest readable page therefore begins at offset 9,800, and paging reaches exactly 10,000 offers.

The catalogue is larger than that. Offers beyond the ceiling are unreachable by any query, so the sort order determines *which* offers are lost: newest-first makes the unreachable tail the oldest offers, which are by definition not new. Under any other sort the unreachable tail could contain newly listed offers.

## Bounding a poll

The monitor records the newest `StartDate` it has observed. Each poll reads back to that mark less a fixed **lookback** margin, stopping at the first page whose oldest offer predates the cutoff.

The stop test is *strictly older*, never *equal*. A batch sharing one timestamp can straddle a page boundary, so halting on equality would abandon the remainder of that batch — and an offer's position within its batch is arbitrary, so the remainder is exactly where a new offer may sit.

The lookback exists because an offer can appear carrying a `StartDate` earlier than one already recorded. A cutoff drawn exactly at the high-water mark would sort such an offer below the boundary and never read it. The margin is the difference between catching those and silently dropping them.

Bounding by novelty instead — stopping once a page contains only already-seen offers — is unsound for the same reason equality-stopping is: within a large batch, a page can be entirely familiar while a new offer sits deeper in the same batch.

## Startup and steady state

Startup reads the whole reachable catalogue and records it without notifying. This establishes both the seen-set and the initial high-water mark; without it the first poll would treat 10,000 existing offers as new.

Startup failure is retried with exponential backoff, indefinitely, rather than being fatal. The interval doubles to a ceiling and then holds; there is no attempt limit, so a durably broken upstream leaves the monitor retrying at that cadence rather than exiting. The steady-state loop already tolerates a failed read and continues to the next poll, so treating the identical failure as fatal at startup only converts a recoverable blip into a process exit — and, under a container restart policy, into a restart loop that reports as running.

The high-water mark only advances. An offer leaving the catalogue does not retract it, so the cutoff never moves backwards and a poll never re-reads ground it has already covered.

A poll can also terminate on depth rather than date: when the cutoff lies beyond the search depth ceiling, the read stops at the ceiling and says so. That is the same bound startup works under, reached from the other direction.

Steady state alternates: read the bounded prefix, record and report novelty, wait. The wait follows the read rather than being a fixed period, so the cycle is read time plus the configured interval.

## Liveness

Retrying indefinitely keeps the process alive through failures it may never recover from, which would leave a monitor that has stopped working indistinguishable from one that is idle. Detection therefore emits a liveness signal: a timestamp recorded after each successful catalogue read, including the startup read.

The signal is consumed as a container health check, which fails once the timestamp is older than a margin derived from the configured poll interval. A startup that never succeeds never becomes healthy; a monitor whose polls have begun failing goes unhealthy without exiting. Neither case restarts the process, so the log stays continuous across the whole failure.

Success means a completed read, not a completed notification. Notification failures are per-channel and tolerated by design, and treating them as liveness failures would report the system as broken when it is doing its job.

The timestamp is written to `/run/woot-monitor/alive`, a location the unprivileged runtime user can write and deliberately not the working directory the configuration is mounted into.

The staleness margin is recorded beside the timestamp rather than recomputed when the check runs. The monitor has already resolved the poll interval through the configuration layering, so the derivation happens once, in the only place that knows the answer, and the check needs nothing but the signal itself.

Reporting unhealthy is not the same as restarting. A container restart policy triggers on exit, and this design deliberately does not exit — so an unhealthy monitor stays up until an operator or orchestrator acts on the signal. That is the intended division: the system reports its own state honestly and does not decide what should be done about it.

## Novelty

An offer is new when its identifier has not been recorded before. Only first appearance matters: an offer's later state changes — selling out, returning to stock, changing price — are not tracked and produce nothing. The record is in-memory and unbounded — offers are never evicted — so an offer that drops out of the readable window and later returns is not reported twice.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Ordering | `NewestFirst` | `BestSelling` | Under a depth ceiling the sort decides which offers are unreachable. Newest-first strands the oldest; best-selling strands the worst-selling, which is where a newly listed offer sits. |
| Poll bound | Date cutoff | Stop at first seen offer; full sweep | Batch-internal order is arbitrary, so novelty-bounding steps over new offers. A full sweep is correct but reads the whole catalogue every cycle. |
| Cutoff margin | Fixed lookback behind the high-water mark | No margin | Offers are observed appearing with start dates behind the mark; without a margin those are read past and lost. |
| Stop test | Strictly older than cutoff | Older or equal | A timestamp batch can straddle a page boundary; equality-stopping abandons its remainder. |
| Startup failure | Retry indefinitely with capped backoff | Exit and let the restart policy handle it; give up after N attempts | The steady-state loop already tolerates the same failure. Exiting turns a blip into a restart loop that is invisible in container status; an attempt limit would exit on exactly the failures a restart cannot fix either. |
| Reporting a stalled monitor | Liveness timestamp read by a container health check | Exit after N failed attempts so restarts are visible | Exiting reports only a failed startup, while the steady-state loop can stall just as silently. A liveness signal covers both, and keeps the log continuous where restarting resets it. |
| Notification lost to a crash | Accepted; record before notify | Record after notify | Recording first risks losing one notification to a crash; notifying first risks re-sending it on every poll until delivery succeeds. |
| Margin derivation | Recorded beside the timestamp by the writer | Recomputed by the health check from the configuration | Re-reading the configuration on each check adds a failure mode unrelated to polling: configuration that has become unreadable would report as a dead monitor. |
| Liveness trigger | A successful catalogue read | A successful notification; a completed loop iteration | Notification failures are per-channel and tolerated; treating them as liveness failures would report a working system as broken. |
| Seen-set | In-memory, no eviction | Persistent store; bounded LRU | Catalogue turnover makes a post-downtime backlog mostly stale, and no eviction means an offer re-entering the window is not re-reported. [inferred] |
| Page size | 200 offers | Larger or smaller pages | Interacts with the depth ceiling: the page size divides 10,000 evenly, so paging lands exactly on the ceiling. [inferred] |

## Open Questions & Future Decisions

### Resolved
1. ✅ Ordering is monotonic and stable enough to bound reads by date — verified against the live API across the full reachable range.
2. ✅ Offers can carry a start date behind the high-water mark — observed in production, which is what the lookback margin exists for.

### Deferred
1. The staleness margin is derived from the poll interval rather than measured against real read durations, so an unusually slow read could report unhealthy while still working.
2. The maximum backdating interval is unknown. The lookback margin is sized generously rather than empirically; an offer backdated beyond it would be missed silently.
3. The catalogue grows over time, widening the unreachable tail. There is no alert when it crosses a threshold, only a per-poll warning line.
4. Restart re-reads and re-seeds silently. Whether an operator wants missed offers replayed after downtime is unsettled; the current answer is no.

## References

- `docs/high-level-design.md` — segment boundaries and tenets.
- `docs/intent/routing/routing-design.md` — what happens to an offer once declared new.
