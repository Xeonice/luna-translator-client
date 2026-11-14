## MODIFIED Requirements

### Requirement: DRV-001 Preview path resolution depends on its requested drive
The implementation SHALL resolve a canonical Windows preview target using only the mapping required by that target, without making unrelated unavailable drives its prerequisite.

#### Scenario: unrelated-broken-drive-does-not-block-valid-preview-target
- **GIVEN** a valid target on one mapped drive and an unrelated unavailable mapping
- **WHEN** the production preview context freezes or verifies that target
- **THEN** its resolved executable and complete identity remain usable without changing either mapping.

### Requirement: DRV-002 Requested target failures remain refusals
The implementation SHALL refuse invalid requested mappings, missing target files and unsupported absolute-path syntax, while preserving existing accepted path normalization.

#### Scenario: requested-drive-and-path-failures-are-not-silently-ignored
- **GIVEN** an unavailable requested mapping or an invalid Windows target path
- **WHEN** the narrow production resolver runs
- **THEN** it returns the existing explicit failure rather than substituting another drive or executable.

### Requirement: DRV-003 Frozen targets retain identity and legacy mapping semantics
The implementation SHALL preserve post-freeze identity checks and the existing semantics of callers that require all drive mappings.

#### Scenario: remapped-target-is-rejected-and-legacy-callers-retain-checks
- **GIVEN** a frozen preview target or a caller requiring all drive mappings
- **WHEN** the target mapping changes or legacy mapping checks run
- **THEN** the original identity refusal and all-drive validation remain effective.

### Requirement: DRV-004 New source does not inherit failed runtime acceptance
The implementation SHALL preserve R59 failures and require a newly sealed package and independent actual verification before promoting stability acceptance.

#### Scenario: engineering-repair-does-not-promote-old-game-evidence
- **GIVEN** a source repair and the archived incomplete R59 collection
- **WHEN** the repair passes engineering checks
- **THEN** the old runtime and visual results remain failed or blocked and the new package is not assumed verified.

### Requirement: DRV-005 Private worker schema changes are versioned and fail closed
The implementation SHALL carry the frozen canonical image path in private collector protocol version 3 and retain same-executable authentication and strict payload validation without changing Core or public contracts.

#### Scenario: private-worker-v3-rejects-old-or-incomplete-frozen-target
- **GIVEN** a version-3 same-package producer and worker with a frozen canonical target path
- **WHEN** an old version, missing path or unknown field is received, or the current canonical target path changes
- **THEN** it is rejected without compatibility fallback; public DTO/IPC, Core ABI1 and Hook JSONL2 remain unchanged.
