## MODIFIED Requirements

### Requirement: FP-03 Session preview ownership has one coordinator
The implementation SHALL preserve exact lease and association ownership while delegating IPC effects through one coordinator.

#### Scenario: late-candidate-cancel-and-old-session-stop-cannot-release-current-owner
- **GIVEN** a candidate transfers to a session or session B replaces session A
- **WHEN** old cancellation or stop arrives
- **THEN** CAS and generation checks isolate the current owner and actual release acknowledgement remains required.

### Requirement: FP-02 Cover capture is a read-only consumer of verified fresh frames
The implementation SHALL retain the first-frame cover policy and prevent cover work from controlling the producer.

#### Scenario: cover-failure-and-consumer-lifecycle-never-stop-shared-capture
- **GIVEN** a verified associated session with no existing cover
- **WHEN** cover preparation fails or a UI consumer detaches
- **THEN** Hook and shared capture remain independent while the original atomic commit and frame identity checks remain enforced.
