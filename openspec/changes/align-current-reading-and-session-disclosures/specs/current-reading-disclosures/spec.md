## ADDED Requirements

### Requirement: CPR-001 Independent current reading surface
The client SHALL show the current reading object before historical disclosures using the existing annotated SourceText and exact unit/revision actions.

#### Scenario: current-text-and-fixed-learning-survive-new-dialogue
- **WHEN** dialogue or translation changes
- **THEN** current content follows its owner, annotated preferences work, and a previously opened word/learning object retains its original identity.

### Requirement: CPR-002 Independent retained session disclosures
The client SHALL replace mutually exclusive session tabs with three default-collapsed independent datasets retaining their query, range, page, scroll and follow state.

#### Scenario: closing-and-reopening-retains-independent-dataset-state
- **WHEN** users filter, page, collapse and reopen a dataset
- **THEN** that dataset retains its state and the other datasets and preview subscriptions remain independent.

### Requirement: CPR-003 Real implementation verification without release promotion
The product change SHALL retain approved design identity, architecture boundaries and existing assertions, and SHALL report actual candidate validation separately from complete release readiness.

#### Scenario: product-code-is-built-and-gate-gaps-remain-visible
- **WHEN** the changed production components are compiled and exercised
- **THEN** real behavior/captures are recorded and missing independent reference/native/game acceptance remains blocked.

### Requirement: CPR-004 Creator-owned pending file lease release
The existing runtime SHALL release its pending-launch file lease when the owning process ends the operation, even while an unrelated fork child retains inherited descriptors. A child SHALL NOT unlock a still-live parent owner. The existing file paths, reservation periods, discovery/run order and error semantics SHALL remain unchanged; this does not add a game-launch user feature.

#### Scenario: pending-lease-owner-release-survives-inherited-descriptors
- **WHEN** an unrelated child inherits the lease descriptor and the owning operation completes, or a child drops its inherited guard while the parent operation is active
- **THEN** owner completion releases the lock without waiting for child exit, child drop preserves the parent live lock, and independent contenders remain excluded until the owner releases. Actual deterministic fork handshakes and default-parallel infra regression must verify these facts without lock retries, sleeps, serializing or removing assertions.
