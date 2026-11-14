## MODIFIED Requirements

### Requirement: FP-12 current-approved-design-source-lock-is-explicit-and-closed
The technology contract MUST retain all other reviewed stack, manifest, and historical visual identities unchanged.
技术合同必须只登记已获用户确认、独立审阅且路径相同的现行设计源；框架、依赖与历史视觉身份不得随之漂移。

#### Scenario: five-approved-source-identities-are-adopted-without-weakening-guards
- **GIVEN** R56的5个技术design源锁仍为旧SHA，当前canonical源已有明确确认、冻结与独审
- **WHEN** 执行本次窄登记并运行实际技术合同和原正负例
- **THEN** 技术合同通过且仅5个SHA更新；其他字段逐值不变，未知篡改仍拒绝，旧视觉和新包实机未自动通过
