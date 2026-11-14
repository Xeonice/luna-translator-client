# 工程维护设计与r2修订执行协议

2733只读协议独审和Root真实366804仅授本轮原7计划文件发布；本canonical计划本身不授后续gate/catalog/metadata写入。实际计划源尚待独审。合法已实施起点为 `4515c79a670da96f473fb0c4ed89aa6ea5b117a3c983782592cfdaeb1fec6c13` /2821/739。Root 的 PID37963 采用失败、完整回滚；`8909…` 只是失败候选，不是新 Base。

## 1. 结论与最小范围

继续同一 change `register-binding-impact-surface-mappings`，不换 ID 绕过阶段。保留 R93 实际父 seal/c655/2812、a0bc 计划批准、b11 原 QA 授权、d71 对4515已实施19项的独审、185项原执行及全部旧 P1/P2/P3、c72旧工具独审、c98旧packet批准和失败事务。

SourceAudit 的真实 render 诊断95db表明只须加三个 managed 路径：

1. `docs/architecture/implementation-status.json`
2. `docs/architecture/implementation-status.md`
3. `docs/architecture/engineering-modules.csv`

因此总 scope **29→32**，既有19 QA/docs和7 canonical类别不变，延后采用输出 **3→6**。`docs/architecture/engineering-components.csv` 不加入可写范围，保持4515精确字节；完整 render/validator仍检查它。6个候选是上述3catalog + 原 `iteration-baseline.json`、`baselines/FP1-20261109-R94.json`、`binding-engineering-integration-adoption-20261110.json`。六者必须在一个新namespace中原子采用。

这不是产品实现扩围。739产品源及78条职责的产品身份、status/owner/target/历史/视觉/正式包事实保持原R93。M15登记实际QA职责；M13只准将next_step里的累计义务901→944，不能改变行为、状态或准出。原901保留加43条阻断，不能改成覆盖完成。

## 2. 已实施父 checkpoint：两条历史线分开

`checkpoint/manifest.json` 是本方案附带的真实只读观察：完整4515图与Root input-freeze-01逐字一致，归档19项QA/docs、7项r1 canonical及4份原catalog/export小文件；产品只保存739路径/hash map，不复制运行包/GB资源。

- **产品/原工程父线**仍为实际R93/c655与原ParentSeal94983be、其独审36f760。managed `source-snapshot.json` 保持原905794…，其files仍是真实c655，绝不重拍4515或预测after。
- **本次计划修订起点**是已获a0bc/b11合法编写且由d71独审的4515。checkpoint用于证明这些19项不是尚未实施，也没有被r2重新吞成未准入产品差异。
- checkpoint收录原c72工具、c98packet、PID37963 raw/前后全图/recovery manifest；旧R94 receipt与两个新managed输出不存在。候选8909和failed transaction不授任何采用身份。
- 已归档的managed旧字节是历史快照，未来按新阶段合法改写其原路径不能破坏历史；原签字、raw、失败、旧tool/packet及checkpoint blobs则保持原件精确。
- ParentSeal两个ordinary notes的原有精确豁免原样继承，不能扩大到新raw、progress、旧R94目录或任意其他notes。

checkpoint本身不另造一次批准层，由本次ignored方案独审核对；实际canonical r2独审再次绑定其精确ref。

## 3. 有序阶段与权限

### P0：已完成只读方案独审

2733独审已审核原ignored协议、plan.json、checkpoint、95db诊断及scope32。Root366804只授权当前canonical7发布；19QA、Root5docs、gate3、3catalog和3metadata不在当前写入许可内。

### P1：Root明确仅7计划发布 → 实际计划独审

方案独审通过后，Root记录plan-only发布授权，限定原7个canonical路径；覆盖更新proposal/design/tasks/iteration-card/verification/delta spec，source-snapshot作为第7个被核验文件保持原字节。因此预期实际diff为6个文件，审核basis仍覆盖7个。

保留原8REQ/30scenario/全部累计义务，只补4个场景：H09（合法4515继承）、H10（r2阶段顺序/禁止降级r1）、P03（QA-only catalog语义与单一current proof）、P04（同批六输出与ComponentsCSV不变）。这些仍挂BIM-001/BIM-007，使用既有真实check IDs：binding-successor-handoff、binding-phase-guard-tests、implementation-status-tests/contract、generated-contract-drift、source-scope、baseline-integrity及change-review-contract，不增只写名字的check。

verification增加typed `plan_amendment`，精确列出checkpoint、旧a0bc/b11/d71 refs、r1 canonical archive refs、父R93seal、32paths/Owner、gate3、原子输出6、禁止写的ComponentsCSV、只读诊断95db和未来r2 authority路径。新`review_evidence.path`使用`final-source-review-r2.json`，不覆写r1记录。

发布前4515 fullmap和发布后实际fullmap均保存；差异只能是获准6个canonical文件。随后独审真实发布后的7个文件/全map；只读候选的预审不能代替这份实际批准。

### P2：Root r2 gate-only授权 → 三条门禁实现 → 独审新门禁

实际canonical r2独审通过后，Root签独占`qa-authorization-r2.json`。此时只允许改既有scope中的：

- `scripts/quality/binding_phase_guard.py`
- `scripts/quality/test_binding_phase_guard.py`
- `scripts/quality/verify.py`

不需要改readiness算法、storage、lifecycle、页面generator或新建managed QA文件。新门禁必须识别精确r2批准/Root授权，原算法/断言保留；新增自包含阶段/六输出反例，不重做已保留185来冒新事实。Root最终整轮执行时仍运行累计完整守卫。

P2独审绑定实际新门禁字节/全map和当前三个调用入口。若Root需更新原5个现行说明文档，须在r2授权中精确列出、在门禁阶段独审后更新，随后重新冻结实际输入并独审；不能隐含让全部19项任意重写。未授权更新的既有QA/docs按4515原字节保留。

### P3：新字节冻结后补catalog候选/独审 → 新原子六输出采用

SourceAudit只在ignored构造candidate。95db里的10个函数锚点只是r1提案，不能直接stamp。先冻结P2/现行文档的实际字节，再重新取M15三项真实职责：实际diff强制交接、requested∪auto真实派发、typed readiness与实际六输出/继承协议。旧四个SHA漂移只是触发原因，不是刷hash的许可。

catalog采用现有`independent-implementation-source-audit/v1`原validator合同，review_scope明确QA-only，产品739只保留R93；body hash及product hash按现函数独算。顶层只有**一份**current generic proof；原R93 proof保留在before-catalogue历史和新proof的nested prior ref，不并列第二份current proof、不更改validator。

新catalog候选源证据须先用**原未修改source-evidence检查函数**对实际Root逐条核SHA/quote/line，运行真实render对MD、modulesCSV和不变ComponentsCSV逐字对照。此为candidate专项检查，不谎称完整Main validate_catalog已通过；全量原validator仍必须在实际原子事务内执行且失败回滚。没有monkeypatch全局路径/错误结果、没有Main暂存写、没有GB克隆。

新packet独审同时绑定新工具、input源审、QA-only catalog proof、六候选、完整before/after map、typed pin delta与未变pin。通过后Root执行新namespace实际六输出事务；所有原postconditions照常运行，无错误白名单。任一失败回滚全部实际写入，保留新失败并停在原源，不凭packet批准宣称adopted。

### P4：整合后的真实执行

实际采用成功后再进行原计划要求的fresh fullBIND/strict v5/完整verify与实际编译。旧185不替新源整轮，fullBIND缺Native/service仍nested blocked，strict v5任何失败仍失败；release/require-product-complete非零、product incomplete/nextstagefalse。这里不重复或改写旧P1/P2/P3。

## 4. 实际前置与未来独占authority/namespace

- ignored方案独审：`amendment-r2-design-review-01/review.json`
- 实际plan-only发布授权：`plan-publication-authorization-r2.json`，SHA366804958b18597d124d6cd8952db087a5e75f4c0c16da502da2e26f19517a56
- canonical实际发布：`canonical-publication-r2-01/{before-map,after-map,packet}.json`
- 实际canonical独审：`canonical-plan-review-r2.json`
- gate-only Root授权：`qa-authorization-r2.json`
- 实际新门禁/source独审：`input-source-review-r2.json`
- 最终current源审：`final-source-review-r2.json`
- 新事务：`adoption-r94-r2/`，内含`tool/`、`catalog-source-audit/`、`packet/`、`packet-review.json`、`transaction/`、各actual-*独占目录。

以上均在`.build/binding-engineering-integration-20261110/`下；2733与366804已真实存在，当前publication正在形成，后续canonical-review-r2、Root gate授权、源审及新packet/receipt尚未获批。旧`adoption-r94/`不再prepare/改写/重用；旧c72/c98不批准新工具/新6输出；第一轮未采用，基线名仍可为R94，但事务目录与身份必须新建。

## 5. 新review/auth的最小机器协议

### 实际canonical review r2

kind建议`binding-successor-canonical-plan-amendment-review/v1`，schema1/statusapproved、独立非作者、原4实际authors保持且reviewer不在内。必须绑定：

- predecessor canonical a0bc、Root auth b11、d71 source review的精确refs；
- 本checkpoint及source4515/2821/product739/产品map；
- 原ParentSeal949/36及managed snapshot905/c655的身份；
- 实际plan发布before/after全map、7个basis hashes，合法6文件publication delta；
- scope32、原19继承清单、gate3、受授权现行docs精确清单、输出6、ComponentsCSV旧SHA；
- 所有旧30义务+新4控制，原ordinary2完整disposition；
- product_write_allowed=false、product_completion=incomplete、nextstagefalse。

Root r2 auth建议kind`root-explicit-qa-maintenance-amendment-authorization/v1`，精确绑定这份实际review的SHA和发布后实际source tree，并记录阶段权限。不是仅把allowed_paths从29改32：当前gate-only只3条；catalog输出6只有在新门禁源审、新QA-only catalog proof和新exact packet独审后才能原子写入。

### Gate入口：强制active r2，不fallback

原a0bc/b11常量及原r1三输出算法/历史控制保留；新加入**code-owned的实际r2 review/auth精确pins**，只能在真实批准存在后填写，不用latest文件扫描、不接受metadata自报hash、无zero占位许可。

增加当前入口（例如`validate_successor_current_handoff`）：verify真实diff强制路径和CLI `--successor-check`都调用它。生产active epoch固定r2；缺/错/被删除的r2authority一律fail，不回落r1，即使把7文件改回r1、metadata删amendment或换changeID也失败。旧r1算法只作为显式历史/guard API，不是当前准入fallback；旧unit fixtures继续明确验证其旧合同，不修改原断言迁就新状态。

r2继承核验使用4515档案和d71/原实际r1successor-check raw，不用当前Root重新执行旧gate以冒历史树。source-snapshot仍c655；新的current diff检查以实际r2 plan发布图和已批准阶段为界，不能reset/delete旧R93/P3义务。checkpoint中的可合法改写managed原路径只核历史archive；旧raw/签字/失败原件核original exact。

### 六输出消费者：新增显式版本，不静默改v1

新packet kind建议`binding-engineering-exact-adoption-packet/v2`，新review kind建议`independent-binding-engineering-adoption-packet-review/v2`。active r2只消费新namespace/v2；原v1 exact3仅留历史守卫，不可批准当前R2。

v2在原完整map/PID/候选/typed pin/source-review合同上增加amendment_review、Root_auth、implemented_parent、actual_gate_source_review、catalog_source_audit及protected_catalogue/comps_export原件。outputs path集合恰6、每份candidate与最终after map/实际字节精确绑定。可声明输出中存在字节不变者，但真正delta必须恰等`{path | before[path] != output.sha}`，不能因此漏验证任一输出。严格typed JSON leaves保留null/missing/bool区别；完整未变pin和legacy debt不变。

actual receipt仍必须actual adopted、相同after tree/count/六outputs/新packet-review引用；缺收据、旧c98、旧r1receipt、rolled_back或仅预测after都拒绝。执行前input map要等实际冻结源；执行后current要等exact after map；不把旧4515的d71当新门禁或六输出的源审。

## 6. 精确触点与责任

新managed范围只有catalog3（SourceAudit负责内容/导出语义，Root原子事务写入），现有32路径的其他Owner不变。P1仅canonical7；P2仅上述gate3（GateLead）；Root5现行docs若需更新必须在实际r2授权中列明；P3仅六输出；其余已实施QA源不得随手改动。readiness两文件、storage两文件、lifecycle/allpage源码无本次r2必要逻辑修改。implementation_status.py validator不改，不加框架，不动apps/crates/native/resources。

## 7. 必须新增的可执行反例

1. a0bc/b11/d71缺失或错SHA、自审/漏作者、checkpoint/product/fullmap漂移拒绝；合法4515继承通过。
2. 只预测8909、旧失败receipt、非完整回滚或旧c98充新批准拒绝。
3. R2 authority缺/改/删、metadata删amendment、7文件退回r1、换ID或直接走legacy-noop拒绝；CLI与verify都实调active r2。
4. P1改gate/catalog，P2提前改catalog或未授权19源，P3超六输出拒绝。
5. 只刷4SHA、不审新六输出语义；复用r1旧“exact3”锚点；给两份current generic proof；把QA审阅提升739产品/Native/包通过，全部拒绝。
6. M13除计数说明任何字段变、任一其他78产品字段/历史改变、ComponentsCSV有diff拒绝。
7. 输出少/多/替换、旧v1/旧namespace/跨nonce-source-review/after猜图；任一候选hash或pin proof不实拒绝。
8. 六输出后任一原Main validator失败应真实回滚；新后写字节被别人替换时沿用原工具late-writer保护，不覆盖他人。
9. 不变导出可列入outputs但必须核字节；actual diff与候选派生差集不符拒绝。
10. 原fullBIND/v5/readiness、五类CLI、18/EX9/153/RW/901+43/RBC/四游戏义务不减，R2不能发产品complete/nextstage=true。

## 8. Root 下一步清单

先交本方案+checkpoint给非作者；通过后只发布7计划并独审真实7源；再记录r2gate-only授权并交GateLead改3QA；冻结/独审新源后SourceAudit准备新catalog/工具候选和QA-only proof；精确6输出独审后Root实际新事务，最后才fresh整轮。本轮只执行已经获得366804授权的canonical发布；其后立即停止Main写入并交实际7源独审。

## 9. 现有I1～I4事实与不得回退的合同

I1已真实登记115规则（原85+30 literal，12入口与18子文件），queries保M12/C17并加当前真实C13/14/15/35只读职责；allpage944保原901和43新阻断。I2旧九suite immutable copy窄修及原current v5 strict保留。I3实际diff强制handoff/requested∪automatic真实派发、I4原fullCLI+独立typed readiness已经r1实现并获d71源审，185项守卫不是fullBIND或strictv5运行通过。r2只修其当前权限/六输出依赖登记，不重做产品、不改full错误与外部缺层资格。

新的active入口固定为validate_successor_current_handoff、当前六输出消费者successor_adoption_errors_v2或同等独立版本实现；原r1 exact3只留历史合同，不能成为当前r2兜底。共同ParentSeal和safe-ref可复用/窄提取，但旧算法断言、原phase分支和事实保留。M15最终锚点必须覆盖实际被CLI/verify调用的current入口与实际v2六输出函数；不能把旧诊断十项当数量上限或只引旧v1。

## 10. 完整Owner与阶段路径

| 精确路径 | Owner | 允许阶段 |
|---|---|
| `scripts/quality/test_lifecycle_impact.py` | /root/binding_source_audit | QA_contract_and_guards_after_plan_review |
| `scripts/quality/all_page_design_coverage.py` | /root/binding_source_audit | QA_contract_and_guards_after_plan_review |
| `scripts/quality/test_all_page_design_coverage.py` | /root/binding_source_audit | QA_contract_and_guards_after_plan_review |
| `scripts/quality/storage_schema_v5.py` | /root/storage_integration_fix | strict_storage_QA_after_actual_owner_assignment |
| `scripts/quality/test_storage_schema_v5.py` | /root/storage_integration_fix | strict_storage_QA_after_actual_owner_assignment |
| `scripts/quality/verify.py` | /root/binding_resumption_gate_implementation | QA_gate_code_after_canonical_plan_review |
| `scripts/quality/binding_phase_guard.py` | /root/binding_resumption_gate_implementation | QA_gate_code_after_canonical_plan_review |
| `scripts/quality/test_binding_phase_guard.py` | /root/binding_resumption_gate_implementation | QA_gate_code_after_canonical_plan_review |
| `scripts/quality/binding_readiness_assessment.py` | /root/binding_resumption_gate_implementation | QA_gate_code_after_canonical_plan_review |
| `scripts/quality/test_binding_readiness_assessment.py` | /root/binding_resumption_gate_implementation | QA_gate_code_after_canonical_plan_review |
| `docs/development/lifecycle-impact-contract.json` | /root/binding_source_audit | QA_contract_and_guards_after_plan_review |
| `docs/development/engineering-gates.md` | /root | current_doc_update_after_canonical_plan_review |
| `docs/development/current-product-test-coverage.md` | /root | current_doc_update_after_canonical_plan_review |
| `docs/development/current-client-alignment-plan.md` | /root | current_doc_update_after_canonical_plan_review |
| `docs/README.md` | /root | current_doc_update_after_canonical_plan_review |
| `docs/development/current-phase.json` | /root | current_doc_update_after_canonical_plan_review |
| `docs/development/all-page-design-coverage.json` | /root/binding_source_audit | QA_code_then_reviewed_generator_outputs |
| `docs/development/all-page-design-coverage.md` | /root/binding_source_audit | QA_code_then_reviewed_generator_outputs |
| `docs/development/current-design-alignment.json` | /root/binding_source_audit | QA_code_then_reviewed_generator_outputs |
| `openspec/changes/register-binding-impact-surface-mappings/proposal.md` | /root/binding_resumption_gate_implementation | canonical_plan_publication_only_after_Root_authorization |
| `openspec/changes/register-binding-impact-surface-mappings/design.md` | /root/binding_resumption_gate_implementation | canonical_plan_publication_only_after_Root_authorization |
| `openspec/changes/register-binding-impact-surface-mappings/tasks.md` | /root/binding_resumption_gate_implementation | canonical_plan_publication_only_after_Root_authorization |
| `openspec/changes/register-binding-impact-surface-mappings/iteration-card.md` | /root/binding_resumption_gate_implementation | canonical_plan_publication_only_after_Root_authorization |
| `openspec/changes/register-binding-impact-surface-mappings/verification.json` | /root/binding_resumption_gate_implementation | canonical_plan_publication_only_after_Root_authorization |
| `openspec/changes/register-binding-impact-surface-mappings/source-snapshot.json` | /root/binding_resumption_gate_implementation | canonical_plan_publication_only_after_Root_authorization |
| `openspec/changes/register-binding-impact-surface-mappings/specs/binding-engineering-integration/spec.md` | /root/binding_resumption_gate_implementation | canonical_plan_publication_only_after_Root_authorization |
| `docs/development/iteration-baseline.json` | /root | final_reviewed_next_baseline_transaction |
| `docs/development/baselines/FP1-20261109-R94.json` | /root | final_reviewed_next_baseline_transaction |
| `docs/development/binding-engineering-integration-adoption-20261110.json` | /root | final_reviewed_next_baseline_transaction |
| `docs/architecture/implementation-status.json` | /root/binding_source_audit | QA_catalogue_candidate_then_Root_atomic6_only_after_r2_gate_and_packet_review |
| `docs/architecture/implementation-status.md` | /root/binding_source_audit | QA_catalogue_candidate_then_Root_atomic6_only_after_r2_gate_and_packet_review |
| `docs/architecture/engineering-modules.csv` | /root/binding_source_audit | QA_catalogue_candidate_then_Root_atomic6_only_after_r2_gate_and_packet_review |

可由后续Root r2授权选择的现行文档仅以下五项，须明确子集且在gate源审后更新；空/漏列表表示不许更新，绝不允许整个19任意改写：

- `docs/development/engineering-gates.md`
- `docs/development/current-product-test-coverage.md`
- `docs/development/current-client-alignment-plan.md`
- `docs/README.md`
- `docs/development/current-phase.json`

## 11. 未来六输出机器合同

新v2 manifest/review在原完整input/after map、source-review、typed JSON leaves与未变pin合同上，精确追加amendment_review、Root_auth、implemented_parent、actual_gate_source_review、catalog_source_audit以及protected_catalogue与ComponentsCSV旧字节。packet outputs恰六路径，每份候选SHA必须同时等after-map和实际最终字节；actual delta恰为候选派生的changed集合，声明不变输出仍逐字核验。新的input-source-review-r2须绑定实际冻结gate/doc完整源与原4作者/独立非作者，不能复用4515的d71来审新代码。新catalog body proof单current且QA-only，旧R93 beforeCatalogue与nested prior refs精确保留。

Root实际adopted receipt必须绑定同after fulltree/count/六outputs/新v2 packet-review；新namespace旧c98、缺收据、rolled_back、仅after猜图一律拒绝。完整Main postconditions/当前artifact/目录边界/legacy-debt/完整pin检查原函数不改，任一失败整体回滚并保late-writer保护；之后还须独立post-adoption验证，才可进行fresh整轮。
