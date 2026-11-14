## MODIFIED Requirements

### Requirement: FP-07 Frozen word-card geometry uses one native reason authority
The implementation SHALL preserve the existing native geometry cause policy when extracting its private source module.

#### Scenario: content-fit-preserves-card-and-user-geometry-invalidates-exact-generation
- **GIVEN** a frozen card and owner stamp
- **WHEN** new reading content requests fit or the user changes owner geometry
- **THEN** existing causal classification and exact-generation rules remain unchanged, without adding a browser resize authority.
