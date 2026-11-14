## MODIFIED Requirements

### Requirement: NOP-001 Natural terminal state covers session and operations
The implementation SHALL commit a successful same-SID Hook session terminal transition and all its active capture operations in one existing transaction, using the same terminal time.

#### Scenario: natural-terminal-transition-closes-all-same-session-active-operations
- **GIVEN** an immutable Hook session lifecycle and its durable operation records
- **WHEN** the production terminal transition or its stated failure/isolation boundary is exercised
- **THEN** the defined atomic, SID-scoped and independently verifiable contract holds.

### Requirement: NOP-002 Operation persistence failure rejects a successful drain acknowledgement
The implementation SHALL roll back body sealing, session state, operation state and commit revision when operation persistence fails, and SHALL not acknowledge a successful drain.

#### Scenario: operation-write-failure-rolls-back-and-rejects-drain-ack
- **GIVEN** an immutable Hook session lifecycle and its durable operation records
- **WHEN** the production terminal transition or its stated failure/isolation boundary is exercised
- **THEN** the defined atomic, SID-scoped and independently verifiable contract holds.

### Requirement: NOP-003 Terminal changes remain SID scoped and idempotent
The implementation SHALL preserve other sessions and make repeated terminal barriers leave commit revision and terminal times unchanged.

#### Scenario: late-or-repeated-terminal-events-preserve-other-sessions-and-revisions
- **GIVEN** an immutable Hook session lifecycle and its durable operation records
- **WHEN** the production terminal transition or its stated failure/isolation boundary is exercised
- **THEN** the defined atomic, SID-scoped and independently verifiable contract holds.

### Requirement: NOP-004 Terminal policies preserve existing durable identities
The implementation SHALL only close same-SID active operations and preserve immutable identities, legacy markers, pre-existing terminal operations and unbound opening operations.

#### Scenario: terminal-policy-preserves-existing-terminal-and-legacy-identities
- **GIVEN** an immutable Hook session lifecycle and its durable operation records
- **WHEN** the production terminal transition or its stated failure/isolation boundary is exercised
- **THEN** the defined atomic, SID-scoped and independently verifiable contract holds.

### Requirement: NOP-005 Source repair does not promote old game acceptance
The implementation SHALL retain the existing schema, public contracts, Core and collector versions and failed R59 evidence; controlled source tests SHALL not substitute for a new signed package and actual game verification.

#### Scenario: existing-schema-contracts-and-old-runtime-failures-remain-unchanged
- **GIVEN** an immutable Hook session lifecycle and its durable operation records
- **WHEN** the production terminal transition or its stated failure/isolation boundary is exercised
- **THEN** the defined atomic, SID-scoped and independently verifiable contract holds.

### Requirement: NOP-006 Complete storage adapters retain the actual compiled review closure
The implementation SHALL register the actual existing preview_identity.rs test input in the common v1 adapter test_sources list inherited by v2, retaining all prior registrations, required Cargo commands, checks and assertions.

#### Scenario: complete-storage-adapters-review-the-existing-preview-identity-test-input
- **GIVEN** the actual app-infra library test closure includes preview_identity.rs
- **WHEN** either production complete storage adapter validates its reviewed input inventory
- **THEN** that source is present once and no prior source, required check, compile/run command or assertion is removed.
