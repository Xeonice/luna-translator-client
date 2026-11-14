# 模型服务设计文档增量

## ADDED Requirements

### Requirement: MSD-001

系统 MUST 满足MSD-001，保持既有产品边界和本轮文档范围。

#### Scenario: explicit-full-protocol-scope

WHEN 本轮设计被检查 THEN 13协议和21预设须与目录完全一致，TMT排除、TokenHub两路保留，未执行适配/实服不得标通过。

### Requirement: MSD-002

系统 MUST 满足MSD-002，保持既有产品边界和本轮文档范围。

#### Scenario: versioned-extension-and-auth-boundaries

WHEN 阅读或导入扩展契约 THEN 精确绑定协议/profile/preset版本；用户数据预设和新Rust协议分开；OAuth/候选字段不被表述为已批准DDL或已实现。

### Requirement: MSD-003

系统 MUST 满足MSD-003，保持既有产品边界和本轮文档范围。

#### Scenario: same-source-html-and-drift-rejection

WHEN 导出HTML THEN 同一冻结字节完成校验、渲染和hash；任何源变化、矩阵漂移、畸形目录必须失败；实际产品检查结果不被文档工具提升。

