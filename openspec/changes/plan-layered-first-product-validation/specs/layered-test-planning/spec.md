## ADDED Requirements

### Requirement: TPG-001 Preserved acceptance traceability
测试计划 SHALL 完整保留首版CR55、MC22与GL20的来源身份和验收语义，逐项映射FP、AI配置、模块组件及可观察证据。

#### Scenario: missing-or-weakened-acceptance
- **WHEN** 来源用例遗漏、重命名、指纹漂移或未经受控替代而弱化步骤/预期
- **THEN** 计划检查返回非零，不产生产品通过结论

### Requirement: TPG-002 Layer and environment contracts
测试计划 SHALL 区分九层前置及受控替身、真实I/O、实际双架构、原生包和四游戏变体，不以低层证据替代真实运行。

#### Scenario: invalid-layer-or-environment
- **WHEN** 删除关键前置、引入依赖环或将真实游戏变体降级为受控单元测试
- **THEN** 计划检查返回非零

### Requirement: TPG-003 Planning is not execution
计划 SHALL 保留未执行、待设计评审及待政策冻结状态，并要求真实产品adapter与同RC执行证据另行实现。

#### Scenario: premature-product-pass
- **WHEN** 在计划中预填passed、批准设计或伪称adapter已实现
- **THEN** 计划检查拒绝，不以文档hash推导实际运行

### Requirement: TPG-004 Deterministic review exports
执行与覆盖CSV SHALL 从同一机器登记导出，默认检查拒绝手工漂移；定义数量不是实际通过数量。

#### Scenario: edited-execution-table
- **WHEN** 导出表被改动、漏项或与登记不一致
- **THEN** 统一门禁返回非零，显式导出不修改产品源码

### Requirement: TPG-005 Planning-only source boundary
本轮 SHALL 仅更新测试规划、规范及检查/报告设施，保留产品源码、原验收、许可和设计批准状态。

#### Scenario: product-source-mutated-by-planning
- **WHEN** 规划变更修改apps/crates/native/resources产品文件或擅自提升批准
- **THEN** 范围或基线检查拒绝；实际编译与守卫检查不代替产品回归
