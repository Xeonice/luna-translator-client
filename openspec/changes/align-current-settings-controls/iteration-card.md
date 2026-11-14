# Settings controls slice

Baseline FP1-20261104-R85, source7de78b2ffc815741f4bcd6dd45d8b971e73822824e0a79fec5c187a7158a8ef1. User: 先补充并替换控件，然后基于这个基础上，去做后续的几个事情. Additional answer: 始终保存，改为保存状态与历史入口（推荐）.

| Work | Requirement / acceptance | Design | Modules / components / owner | Verification / completion |
|---|---|---|---|---|
| GSC-001 | FP-03/04/05/11; RW-17..23 | guided settings/general existing reading behavior, current-session pause | M11/M12/M13; C10/C32/C34; platform worker | real pause/translation command payloads, role positives/negatives, no-session disable, resume/raw preservation |
| GSC-002 | FP-06; AI-CFG-01/03/04/10/13; MC lifecycle | guided settings service list + advanced/learning disclosure | M04/M13; C22/C26/C27/C28/C29/C30/C31; model UI worker | actual save/test/CAS/copy/remove/runtime/pool/context/template/terms, no obsolete six tabs |
| GSC-003 | FP-09/11; RW-17..23 | general history explicit user replacement | M10/M13; C32; platform worker/UI | status not editable, real history navigation, no retention changes |
| GSC-004 | FP-11 | general login startup native status | M12/M13; C32/C34; platform worker | DTO generation, native state and error guards, no OS changes during tests |
| GSC-005 | FP-06/11/12 | exact current target + finite controlled states | M15; QA root; independent non-author reviewer | genuine production builds and browser candidates; unchanged pixel threshold, missing references/Native remain blocked |

Allowed paths are exact in verification.json and preimages were saved before implementation. Root owns docs, OpenSpec, baseline and QA registration. Model worker owns model feature and SettingsWindow. Platform worker owns preferences, bridge/DTO/native and test transport. Shared files need explicit coordination. No concurrent worker may revert another's edits. Existing full PRD/802 obligations and old37 failures remain; no overall acceptance is pre-filled.
