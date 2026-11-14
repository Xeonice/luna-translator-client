# 组合关闭稳定性恢复与阅读截图供给

只组合已独审的六份 Native 源和一份 QA capture 源，不增产品能力。当前父权威为 R63 `aae00b78b178b70cf0d43ef7da5f7ac2307325d3786410488ba160899f76fac8`，Main1933 tree `f85da07881aaa7a5e0b534144ee34f60f1f834417d6f9dfca37bcc22eef41662`。

两个 leaf 原 WI/source-snapshot/freeze 按冻结哈希留在 `.build/stability-refactor-20261027/recovery-fixture-batch-r64/frozen-leaves/`；当前守卫不允许本 change 授权其他 change 目录。Main 拟集成七源与本组合自有 WI，不能伪称两 leaf WI 已装入 Main。

本准备不生成、pin 或采用 R64 authority；Root 实际集成与独立治理审阅是后续先决条件。完整视觉仍红，真实 SCK 与四游戏 not_run。

## 冻结输入与逐项承接

Root 已按现行正式规则确认：Main 只装七份 exact 已审源与本 batch 自有七WI；两 leaf 十五WI不装入 Main。`spec_contract._allowed_paths` 连 exact 跨 change 文件也拒绝；没有改守卫、改 kind、扩大 allowed_paths 或伪造 snapshot。

| leaf / 输入 | 原路径 | SHA-256 |
|---|---|---|
| native / manifest | `.build/stability-refactor-20261027/preview-geometry-recovery-r64/review-packet-r2/manifest.json` | `e9445d5bca74c7e24e4fba971aa39efc640901c2ad347b37474cc16eab23ae3b` |
| native / independent_source_review | `.build/stability-refactor-20261027/preview-geometry-recovery-r64/code-review/r2/review.json` | `39956a9548dff30b91538fb81704d21d20f740753b0d1c133f2a273270038831` |
| native / original_source_snapshot | `.build/stability-refactor-20261027/preview-geometry-recovery-r64/candidate/openspec/changes/repair-preview-geometry-recovery/source-snapshot.json` | `d4d76e41b51a3374428187c1c7d9940191b950069857c4ff1f206a7ec14b0620` |
| fixture / manifest | `.build/stability-refactor-20261027/r63-projection-fixture-repair/review-packet/manifest.json` | `9aed709c4eeab0403636f400d1965050e6500f12dbdbbc87dd633672187abd80` |
| fixture / independent_source_review | `.build/stability-refactor-20261027/r63-projection-fixture-repair/review-packet/evidence/independent-source-review01.json` | `ac53b65a438214d7bf83c57dbcabc62f74de0f5808ff0fac18a7b6cc86952110` |
| fixture / original_source_snapshot | `.build/stability-refactor-20261027/r63-projection-fixture-repair/review-packet/files/openspec/changes/repair-reviewed-reading-projection-fixture/source-snapshot.json` | `405080457db057f6ed14e1d503a6a025e20a5285296fc4689e1b170224cd6fd3` |

十条 leaf 场景由本 batch SRB-002/SRB-003 分别承接，并显式绑定本轮同一组完整注册检查；leaf 的旧通过不继承为 batch 通过。完整 JSON 映射在 verification.json.upstream_scenario_traceability。

| leaf requirement | 原 scenario | batch requirement |
|---|---|---|
| PGR-001 | bounded-filter-output-keeps-source-and-output-units-distinct | SRB-002 |
| PGR-002 | typed-refusal-does-not-itself-authorize-filter-retention | SRB-002 |
| PGR-003 | failed-capture-runs-complete-same-attempt-post-proof-before-retain | SRB-002 |
| PGR-004 | continuous-metadata-and-time-budget-enable-bounded-automatic-recovery | SRB-002 |
| PGR-005 | late-proof-and-geometry-never-cross-authority-epochs | SRB-002 |
| PGR-006 | private-worker-v4-rejects-old-or-incomplete-geometry-diagnostics | SRB-002 |
| PGR-007 | fresh-package-runtime-evidence-does-not-inherit-r62-results | SRB-002 |
| RPF-001 | independent-complete-input-supplies-explicit-projection | SRB-003 |
| RPF-002 | body-capture-waits-for-real-query-and-visible-state | SRB-003 |
| RPF-003 | preserve-isolation-and-historical-failures | SRB-003 |

## 独审批准边界与失败保留

A 独审的真实 decision 为 `approved_for_candidate_integration_review`，不是包、真实 SCK 或产品批准。Q1 真实 status 为 `approved_for_unchanged_registered_browser_v4_diagnostic_only`，只允许原样正式浏览器诊断，尚未批准 Main 集成。最终组合独审必须另外检查 Q1 exact 源、完整18项供给检查（2正例＋16拒绝/等待）、正式 v4 的 source/nonce/回执和失败，再明确决定七源是否可集成 Main。

Q1 正式 v4 为 exit1：body/top225→0 只关闭该隔离差异；其他14主状态20pixel变体、10scroll与2obsolete-off仍失败。分析01错误地要求body三图全零、分析02错误地要求其他PNG字节全同，原脚本/日志/失败回执保留；分析03仅纠正分析范围，正式断言、期望和阈值不改。原始225与 A unified verify 红项均留在原冻结包中，不回填或继承为本 batch 通过。精确外部证据路径/hash在 verification.json.independent_combination_review_prerequisites。
