## MODIFIED Requirements

### Requirement: FP-03 Preview identity and frame admission preserve current contracts
The implementation SHALL isolate existing pure state rules without changing ownership, permission, frame or public interface semantics.

#### Scenario: private-state-extraction-preserves-admission-and-control
- **GIVEN** the existing verified lease and ownership context
- **WHEN** identity resolves, a frame arrives, or explicit Pause/transfer/stop occurs
- **THEN** existing safety fences, observation timestamps and delivery envelope rules remain unchanged.

### Requirement: FP-12 Preview diagnostics do not fabricate capture truth
The implementation SHALL retain authoritative diagnostics and immutable image observations while separating pure policy from effects.

#### Scenario: metadata-and-errors-never-fabricate-live-images
- **GIVEN** a retained frame or an unresolved preview
- **WHEN** only metadata or a failure is observed
- **THEN** no new capturedAt or Live image fact is fabricated and existing diagnostics remain available.
