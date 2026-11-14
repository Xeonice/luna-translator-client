## ADDED Requirements

### Requirement: DBD-001 database design contract
系统 SHALL 满足如下数据库设计约束：源DDL与每字段说明、SQL实际结构、表目录、字典和关系图必须一致。缺字段注释、结构错误或导出漂移时拒绝。

#### Scenario: field-export-drift
- **WHEN** 对本轮设计执行注册检查并输入对应反例
- **THEN** 合法设计通过，违规被实际拒绝；不以设计DDL检查替代产品回归

### Requirement: DBD-002 database design contract
系统 SHALL 满足如下数据库设计约束：必须在隔离SQLite验证严格类型、复合作用域、版本身份、无损uint64、UTF8跨度和跨会话反例。

#### Scenario: invalid-persistence-identity
- **WHEN** 对本轮设计执行注册检查并输入对应反例
- **THEN** 合法设计通过，违规被实际拒绝；不以设计DDL检查替代产品回归

### Requirement: DBD-003 database design contract
系统 SHALL 满足如下数据库设计约束：已保存配置/池版本必须封存；未封存不可应用，封存后不可改语义，head与授权更新遵循CAS。

#### Scenario: mutable-applied-config
- **WHEN** 对本轮设计执行注册检查并输入对应反例
- **THEN** 合法设计通过，违规被实际拒绝；不以设计DDL检查替代产品回归

### Requirement: DBD-004 database design contract
系统 SHALL 满足如下数据库设计约束：取消、代次过时、停用引擎或连接、撤销凭据及旧epoch不得写正式结果；合法完成仍可落盘。

#### Scenario: late-or-unauthorized-result
- **WHEN** 对本轮设计执行注册检查并输入对应反例
- **THEN** 合法设计通过，违规被实际拒绝；不以设计DDL检查替代产品回归

### Requirement: DBD-005 database design contract
系统 SHALL 满足如下数据库设计约束：清理payload保留正文和来源身份；配置GC须无外部引用且有回执，人工修正独立；词典可只读打开，秘密只引用。

#### Scenario: destructive-retention-or-secret
- **WHEN** 对本轮设计执行注册检查并输入对应反例
- **THEN** 合法设计通过，违规被实际拒绝；不以设计DDL检查替代产品回归

### Requirement: DBD-006 database design contract
系统 SHALL 满足如下数据库设计约束：本轮SHALL仅修改授权设计和工程工具；产品目录、原PRD、整稿和旧测试执行状态必须保留。

#### Scenario: unauthorized-product-implementation
- **WHEN** 对本轮设计执行注册检查并输入对应反例
- **THEN** 合法设计通过，违规被实际拒绝；不以设计DDL检查替代产品回归

