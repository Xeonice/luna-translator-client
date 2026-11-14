## MODIFIED Requirements

### Requirement: FP-12 Stable source integration preserves runtime evidence truth
The implementation SHALL preserve existing business authority while independently validating actual changed source and runtime observations.

#### Scenario: private-observation-public-test-entry-and-actual-impact-closure
- **GIVEN** reviewed observation and test entry changes with real source identity
- **WHEN** current-source checks or the final packaged runner collect evidence
- **THEN** public flow and bounded passive observation are used, missing or conflicting states fail, and only genuine same-RC runtime evidence may complete SR6.
