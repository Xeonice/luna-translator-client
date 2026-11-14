## MODIFIED Requirements

### Requirement: FP-03 Hook association and preview proof retain complete target identity
The implementation SHALL preserve complete environment, process, bitness and window identity through private module extraction.

#### Scenario: private-association-proof-extraction-preserves-no-window-and-stale-target-refusal
- **GIVEN** a discovered candidate or associated Windows target
- **WHEN** the private association/proof helpers are invoked
- **THEN** existing full identity checks remain and a process-only target never fabricates a WindowRef.

### Requirement: FP-12 Journal adapters preserve facts without taking session lifecycle ownership
The implementation SHALL keep session atomic lifecycle and FIFO acknowledgement in the facade while extracting factual journal adapters.

#### Scenario: old-session-drain-and-late-events-cannot-clear-new-session
- **GIVEN** session A naturally exits and session B starts
- **WHEN** A trailing raw events or completion arrive
- **THEN** expected SID and full target checks preserve B and acknowledgement requires the original real drain chain.
