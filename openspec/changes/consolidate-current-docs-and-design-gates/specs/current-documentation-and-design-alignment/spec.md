# 当前文档与最新设计对齐

## ADDED Requirements

### Requirement: DOC-001

工程系统 MUST 为当前PRD、设计目标、架构、实施计划和验证规则提供唯一明确入口与职责，历史段落不得形成并行当前规范。

#### Scenario: current-entry-and-documentation-roles-are-unambiguous

- GIVEN R75混合当前与历史叙述的稳定文档路径
- WHEN 收敛当前入口并校验文档合同
- THEN docs/README和机器职责登记精确连接现行说明，当前目标及其pending状态明确；产品ID路径与完整义务保留，工具拒绝缺失、重复或冲突权威。

### Requirement: DOC-002

工程系统 MUST 原字节归档20份R75旧说明并保留所有产品、测试和证据义务，不能通过清理删减范围或重写历史身份。

#### Scenario: exact-archives-preserve-source-and-full-obligations

- GIVEN R75 raw、完整2102源map及20份待重排旧说明
- WHEN 创建归档manifest并整理稳定路径的现行内容
- THEN 每份original/snapshot/bytes/sha256与冻结来源一致，归档索引不覆盖原README快照；原始PRD/License、产品ID、旧计划和历史结果保持，义务缩减或归档漂移真实负例被拒绝。

### Requirement: DAL-001

工程系统 MUST 将最新guided目标的能力、需求、精确来源与适用验证层形成可执行对齐合同，并接入统一verify及实际release拒缺项路径。

#### Scenario: latest-design-capabilities-have-executable-alignment-gates

- GIVEN 当前guided候选、当前PRD及既有产品覆盖合同
- WHEN 对齐登记、静态检查、独立负例和只读就绪评估实际执行
- THEN 新五项检查绑定真实命令与断言，拒绝目标能力或必需层遗漏、来源漂移及伪造通过，同时保留旧R75覆盖/readiness、编译和五类负例，不替代旧97/128/153或70来源附加闭包。

### Requirement: DAL-002

工程系统 MUST 独立判断静态合同、工程检查、设计确认、真实产品执行与完整发布；缺真实层、当前设计确认或同包证据时保持阻断。

#### Scenario: engineering-checks-never-promote-design-or-product-acceptance

- GIVEN guided仍pending_review且部分真实产品能力、adapter与同包证据缺失
- WHEN 工程守卫通过或只读readiness完成
- THEN 评估明确blocked/not_run和逐项缺口，JSON/adapter/旧设计批准不能授予最新设计确认；旧包、fixture、设计截图或有限runner不能继承完整真实产品覆盖，实际release继续拒绝缺项。

### Requirement: DOC-003

工程系统 MUST 通过闭合Owner清单、独立审阅和可恢复事务精确采用R76文档与工程工具差异。

#### Scenario: exact-r76-adoption-preserves-frozen-r75-authority

- GIVEN 原字节R75父基线、2102源冻结map、1228pins及预先闭合60个唯一源路径
- WHEN 独立审阅最终docs/tooling/change/helper并由Root prepare和采用R76
- THEN prepare只写candidate，packet绑定精确manifest/generator；R75归档和非授权源/metadata保持，M15仅唯一hash重绑和确定性CSV；先持久回滚前像再写Main，完整树验证与实际receipt共同成功，失败记录恢复情况，不改变phase/activechange/approval/产品状态。
