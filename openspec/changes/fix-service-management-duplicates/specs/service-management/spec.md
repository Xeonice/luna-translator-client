## ADDED Requirements

### Requirement: SM-001
The system SHALL present one management row per current logical resource using authoritative draft references and current Rust selection references, without treating immutable history versions as additional services.

#### Scenario: one-current-logical-resource-per-management-row
- **GIVEN** ten immutable revisions of one model and three purposes
- **WHEN** the service management page is opened
- **THEN** the current logical resource appears once with truthful purpose and legacy status
- **AND** distinct resources sharing a display name remain distinct

### Requirement: SM-002
The system SHALL preserve the complete historical catalog for selection, fallback and source validation, and SHALL NOT rewrite a selected historical source into a guessed current source.

#### Scenario: preserve-versioned-selection-and-exact-edit-sources
- **GIVEN** distinct draft and applied roots with an explicitly selected older source
- **WHEN** management rows are rendered or an editor is opened
- **THEN** each operation retains its exact source and purpose
- **AND** missing or stale references are not invented, used to merge names, or silently activated
