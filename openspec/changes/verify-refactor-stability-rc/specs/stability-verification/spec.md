## MODIFIED Requirements

### Requirement: FP-12 One real packaged source cohort closes runtime stability obligations
The validation SHALL verify all required stability observations on one current signed package and preserve any missing or failed case.

#### Scenario: real-four-game-cold-discovery-reading-drain-card-and-sleep-wake
- **GIVEN** one frozen signed RC with genuine games and the active profile
- **WHEN** the operator completes normal game reading, consumer lifecycle, natural exit and real system sleep/wake actions
- **THEN** independent OS, raw session, Host and passive trace evidence binds every required case to the same RC and rejects missing, stale, misordered or cross-session evidence.
