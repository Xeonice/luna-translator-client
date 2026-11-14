## ADDED Requirements

### Requirement: TCOV-001
测试覆盖 SHALL 从RW验收来源生成集合，并拒绝缺适用层、伪回执、零断言与跨层替代。

#### Scenario: layered-coverage
- **WHEN** 执行本轮对应检查
- **THEN** 有实际证据且缺项返回失败

### Requirement: TCOV-002
设计测试 SHALL 执行全部组合和入口，将当次源码、命令、nonce、断言及截图绑定。

#### Scenario: design-execution
- **WHEN** 执行本轮对应检查
- **THEN** 有实际证据且缺项返回失败

### Requirement: TCOV-003
发布 SHALL 验证每个case及variant，不允许扁平ID代替执行。

#### Scenario: release-execution-closure
- **WHEN** 执行本轮对应检查
- **THEN** 有实际证据且缺项返回失败

### Requirement: TCOV-004
技术文档 SHALL 对齐已评审设计，区分目标契约、现有实现与未验证能力。

#### Scenario: architecture-alignment
- **WHEN** 执行本轮对应检查
- **THEN** 有实际证据且缺项返回失败

### Requirement: TCOV-005
设计确认 SHALL 绑定具体源码和图片，保留原始基线及受保护内容。

#### Scenario: reviewed-baseline
- **WHEN** 执行本轮对应检查
- **THEN** 有实际证据且缺项返回失败
