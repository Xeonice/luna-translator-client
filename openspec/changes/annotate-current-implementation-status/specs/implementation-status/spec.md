## ADDED Requirements

### Requirement: ISTAT-001
技术文档 SHALL 逐项区分实现、对齐和验证状态，引用实际源码与历史记录。

#### Scenario: source-backed-status
- **WHEN** 核对当前实现并生成报告
- **THEN** 完整状态与差距有可追溯依据，缺项或漂移时拒绝

### Requirement: ISTAT-002
技术表格与HTML SHALL 与同一状态来源一致，不把planned覆盖已有实现，不提升未测能力。

#### Scenario: synchronized-report
- **WHEN** 核对当前实现并生成报告
- **THEN** 完整状态与差距有可追溯依据，缺项或漂移时拒绝
