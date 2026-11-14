## MODIFIED Requirements

### Requirement: FP-12 Declared lifecycle closure is verified after genuine execution
The gate SHALL validate declarations only after the corresponding mandatory closure check has executed.

#### Scenario: late-executed-lifecycle-check-is-recorded-before-declaration-validation
- **GIVEN** a change explicitly declares the mandatory lifecycle closure check
- **WHEN** the gate finishes current-source validation
- **THEN** actual closure is recorded before the declaration audit, while truly absent checks and failed closure still prevent a passed gate.
