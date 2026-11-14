# 组合迭代卡

| 字段 | 内容 |
|---|---|
| iteration / baseline | close-stability-recovery-and-reading-fixture / FP1-20261028-R63 SHA aae00b78b178b70cf0d43ef7da5f7ac2307325d3786410488ba160899f76fac8 |
| 授权 | Root 只授权 ignored 候选；Main 集成、authority 生成与采用均未由本 Owner 执行 |
| Owner / 独审 | /root/r64_batch_governance；组合治理与authority独审另安排 |
| 产品需求 | FP-03/04/05/06/09/12；原阅读窗口与连接预览行为，无新增能力 |
| 设计 / 状态 | interaction-review.html#01-window-preview；index.html#main-content；已确认整稿不改，原图/token/阈值保持 |
| 模块 / 组件 | M12 native adapter、M15 QA；C35/C33/C17；公共 ABI/Hook/DTO保持 |
| 工作项 | SRB-001 frozen exact source；SRB-002 native recovery；SRB-003 independent projection fixture；SRB-004 complete lifecycle |
| 允许路径 | verification.json 逐项七源、自有 WI 与builder实际十一外部authority路径；不授权其他 WI、carrier或lifecycle |
| 复用 / 依赖 | 只复制已审七源；两个冻结 manifest 和两份独审原字节；真实 Main1933 snapshot |
| 检查 | 全SR6声明、实际scope/spec/lifecycle选择、五类负例、真实编译；Root集成及authority采用后完整verify |
| 完成条件 | 源审阅与治理独审、精确Root集成/authority审阅采用、完整真实回执分别可追；任何红项不得宣称本轮完整通过 |
| 当前实测 | 本Owner只做候选结构/引用核对；正式包、SCK、四游戏、finalR63 not_run；完整视觉旧红保留 |

候选外部的 `source-delta.json`、`frozen-leaf-paths.json` 与 `catalog-preview/source-reference-remaps.json` 给出逐文件和逐引用证据。叶片自有十五WI不进入候选Main闭包，避免违反现行跨WI范围守卫；其原快照与冻结身份不改变。

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
