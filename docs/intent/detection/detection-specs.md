# Detection — Specs

## Catalogue reads

- [x] **DETECTION-001**: The system shall request offers from Woot's search API ordered newest-first by start date.
- [x] **DETECTION-002**: The system shall request offers in fixed-size pages, advancing by page offset.
- [x] **DETECTION-003**: While paging, the system shall stop before issuing any request whose offset plus page size would exceed Woot's search depth ceiling.
- [x] **DETECTION-004**: When a catalogue read stops at the search depth ceiling, the system shall report the number of offers read and the total the search reports as matching.
- [x] **DETECTION-005**: When a page returns fewer offers than requested, the system shall stop paging.

## Startup

- [x] **DETECTION-010**: On startup, the system shall read the catalogue to the search depth ceiling and record every offer without notifying.
- [x] **DETECTION-011**: If a startup catalogue read fails, then the system shall retry it after a wait that doubles on each successive failure up to a ceiling, and shall continue retrying without limit.
- [x] **DETECTION-012**: When a startup catalogue read fails, the system shall report the error, the attempt number, and the wait before the next attempt.
- [x] **DETECTION-013**: If a startup catalogue read fails, then the system shall leave its record of seen offers unchanged.

## Bounded polling

- [x] **DETECTION-020**: While a newest recorded start date exists, the system shall read the catalogue back only as far as that date less the lookback margin.
- [x] **DETECTION-021**: While reading back to a cutoff, the system shall stop at the first page whose oldest offer started strictly before the cutoff.
- [x] **DETECTION-022**: While reading back to a cutoff, the system shall not stop on a page whose oldest offer started exactly at the cutoff.
- [x] **DETECTION-023**: While no newest recorded start date exists, the system shall read the catalogue to the search depth ceiling.
- [x] **DETECTION-024**: When a bounded read completes, the system shall report the number of requests issued, the number of offers read, and the cutoff used.
- [x] **DETECTION-025**: If a bounded read reaches the search depth ceiling before reaching the cutoff, then the system shall stop at the ceiling and report that it did so.

## Novelty

- [x] **DETECTION-030**: The system shall treat an offer as new when its identifier has not been recorded before.
- [x] **DETECTION-031**: When an offer is new, the system shall record its identifier and report it under a log target carrying detected offers, whether or not the offer goes on to be notified.
- [x] **DETECTION-032**: The system shall never remove a recorded offer identifier.
- [x] **DETECTION-033**: The system shall not treat a change to an already-recorded offer — its stock, price, or title — as new.

## High-water mark

- [x] **DETECTION-040**: When an offer is read, the system shall advance its newest recorded start date to that offer's start date if it is later than the current one.
- [x] **DETECTION-041**: The system shall never move its newest recorded start date backwards.

## Steady state

- [x] **DETECTION-050**: After completing a poll, the system shall wait the configured interval before beginning the next one.
- [x] **DETECTION-051**: If a poll's catalogue read fails, then the system shall report the error and continue to the next poll.

## Liveness

- [ ] **DETECTION-060**: When a catalogue read completes successfully, the system shall record a liveness timestamp.
- [ ] **DETECTION-061**: The system shall record the liveness timestamp at a location writable by the unprivileged user the monitor runs as.
- [ ] **DETECTION-062**: The system shall not record a liveness timestamp on the outcome of notification delivery.
- [ ] **DETECTION-063**: While the liveness timestamp is older than a margin derived from the configured poll interval, the monitor's container shall report itself unhealthy.
- [ ] **DETECTION-064**: While no catalogue read has yet succeeded, the monitor's container shall not report itself healthy.
