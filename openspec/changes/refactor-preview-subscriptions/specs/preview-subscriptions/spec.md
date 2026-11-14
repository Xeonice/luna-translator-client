## MODIFIED Requirements

### Requirement: FP-03 Preview subscriptions own only their context transport
The implementation SHALL preserve producer independence and isolate current consumer context from obsolete reads.

#### Scenario: obsolete-unsettled-context-never-starves-current-context
- **GIVEN** old context A has a read that never settles
- **WHEN** new generation B attaches to the same lease
- **THEN** B progresses independently while every individual context remains single-flight and old cleanup cannot cancel B.

### Requirement: FP-12 Transport failures preserve authoritative capture truth
The implementation SHALL store local transport health separately from Rust capture observations.

#### Scenario: metadata-success-after-local-failure-is-not-a-new-live-frame
- **GIVEN** a retained frame and a local read failure
- **WHEN** only metadata succeeds or a consumer remounts
- **THEN** the original observation time is retained and no Live image fact is manufactured; a valid fresh frame permits recovery.
