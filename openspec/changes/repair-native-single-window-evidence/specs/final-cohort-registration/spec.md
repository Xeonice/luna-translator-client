## ADDED Requirements

### Requirement: FIN-001 Registry-selected final cohorts
The lifecycle checker MUST select the original nine obligations, fourteen invariant links, maintained cases and complete SR6 checks for each authority-registered final cohort even on an empty diff.

#### Scenario: registered-final-id-retains-the-full-original-cohort
- **GIVEN** 当前已审schema2合同登记附加终验ID，旧primary及原phase规则仍在
- **WHEN** 运行原primary或新登记ID且本轮只有own文档差分
- **THEN** 两者full selection完全一致，缺runtime或组件检查不能跳过；生产源码不硬编码新ID

### Requirement: FIN-002 Exact registration authority
The contract reader MUST require strict schema2, nonempty canonical distinct additional IDs and one exact independent authority pin for the contract.

#### Scenario: invalid-or-unapproved-final-registration-is-rejected
- **GIVEN** 附加列表可能空/非法/重复/撞primary或phase，或合同被修改但未采用独审新pin
- **WHEN** 读取登记或选择正式终验
- **THEN** 全部明确拒绝，CONTRACT pin恰一且两种重复顺序均拒绝，不信任metadata approved/final开关，不降级schema1

### Requirement: FIN-003 Unknown runtime identities fail closed
A development change declaring the packaged stability runtime MUST be an authority-registered final cohort; ordinary changes and existing release coverage MUST retain their original semantics.

#### Scenario: unregistered-runtime-cohort-cannot-be-lightly-validated
- **GIVEN** 未登记的新ID声明packaged runtime，或未知普通change按真实差分工作
- **WHEN** 执行development选择
- **THEN** 前者拒绝，后者仍按实际差分；release原强制九义务保持，不用于冒充本轮development终验

### Requirement: FIN-004 Original checks and evidence remain
The repair MUST preserve original final identity, all phase rules, nine obligations, fourteen links, cases, historical selections, original guards and old failed evidence.

#### Scenario: original-contract-and-guard-obligations-are-preserved
- **GIVEN** schema1合同升级至严格schema2并增添登记，原R59失败仍在
- **WHEN** 采用候选及复跑真实纯守卫
- **THEN** 除版本与追加ID外原JSON一致，原30 test方法及3helper AST保持；不改旧WI/oracle/预算/设计图/阈值或旧失败状态

