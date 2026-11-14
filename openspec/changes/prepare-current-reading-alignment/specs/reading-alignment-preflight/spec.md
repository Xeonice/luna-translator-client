## ADDED Requirements

### Requirement: RAP-001 Exact independent design and real React capture
The tooling SHALL capture four fixed variants from approved design sources and actual existing production React components with the same reading data, without modifying either source or granting acceptance.

#### Scenario: capture-current-reading-from-independent-producers
- **WHEN** the preflight is run with a fresh output directory
- **THEN** it binds exact source/command/nonce, actual DOM and independent PNG for dark/light/narrow/zoom200, reports real differences and retains blocked product acceptance.

### Requirement: RAP-002 Independently recompute raw evidence
The tooling SHALL reject stale, forged, partial, same-producer, wrong-data, wrong-viewport, skipped or modified evidence and independently recompute pixels and structural obligations.

#### Scenario: reject-invalid-reading-capture-evidence
- **WHEN** evidence is incomplete or tampered
- **THEN** the raw-evidence inspector rejects it and existing product layers/approval obligations remain intact.

### Requirement: RAP-003 Stage boundaries and exact review
The tooling SHALL require the existing engineering gate and independent non-author whole-tree review; no diagnostic success SHALL approve references, production alignment, SR6 exit or release.

#### Scenario: stop-before-unmet-product-gate
- **WHEN** read-only capture succeeds but actual production prerequisites remain missing
- **THEN** tool completion and actual product gaps are separately recorded and the next product stage does not start.
