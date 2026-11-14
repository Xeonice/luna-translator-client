## MODIFIED Requirements

### Requirement: ENG-RF-V001 Approved visual source identities remain immutable across physical versions
The verification tooling SHALL resolve only the exact reviewed historical source bytes through an independently pinned closed-domain version map.

#### Scenario: exact-historical-source-resolution-preserves-original-images-and-claims
- **GIVEN** the original v4 approval and frozen catalogue with five exact old source copies
- **WHEN** the current design source has advanced to later authorized behavior
- **THEN** only explicit same-hash historical aliases are accepted, current design remains untouched, original PNG and pixel checks remain mandatory, and any missing or changed evidence is rejected.
