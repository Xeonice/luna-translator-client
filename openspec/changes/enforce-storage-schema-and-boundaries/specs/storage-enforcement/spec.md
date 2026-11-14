## ADDED Requirements

### Requirement: STE-001 storage acceptance enforcement
系统 SHALL 满足以下约束：真实候选库必须与独立冻结的schema契约逐对象一致，字段、关系和约束漂移必须拒绝。

#### Scenario: schema-drift
- **WHEN** 注册程序执行本轮合法输入及对应违规输入
- **THEN** 合法对照通过，违规实际被拒绝，缺生产adapter不转为产品通过

### Requirement: STE-002 storage acceptance enforcement
系统 SHALL 满足以下约束：每字段必须有完整适用边界和实跑断言，缺失、跳过或因其他错误而拒绝不能作为通过。

#### Scenario: missing-or-fake-field-boundary
- **WHEN** 注册程序执行本轮合法输入及对应违规输入
- **THEN** 合法对照通过，违规实际被拒绝，缺生产adapter不转为产品通过

### Requirement: STE-003 storage acceptance enforcement
系统 SHALL 满足以下约束：现有产品用例必须链接存储子验收与Owner、实际运行/编译入口，不得把守卫测试冒充存储模块验收。

#### Scenario: unexecuted-storage-acceptance
- **WHEN** 注册程序执行本轮合法输入及对应违规输入
- **THEN** 合法对照通过，违规实际被拒绝，缺生产adapter不转为产品通过

### Requirement: STE-004 storage acceptance enforcement
系统 SHALL 满足以下约束：生产存储验收SHALL实际调用代码注册的adapter，缺失/失败/过期/伪造结构或不完整证据必须阻断。

#### Scenario: missing-or-forged-product-adapter
- **WHEN** 注册程序执行本轮合法输入及对应违规输入
- **THEN** 合法对照通过，违规实际被拒绝，缺生产adapter不转为产品通过

### Requirement: STE-005 storage acceptance enforcement
系统 SHALL 满足以下约束：本轮仅授权设计与校验工具，保持产品源码/原需求/整稿/计划状态，并以实际回执和报告展示边界。

#### Scenario: report-or-source-drift
- **WHEN** 注册程序执行本轮合法输入及对应违规输入
- **THEN** 合法对照通过，违规实际被拒绝，缺生产adapter不转为产品通过

