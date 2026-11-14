# 本轮数据库设计补全迭代卡

| 字段 | 内容 |
|---|---|
| ID / 依据 | specify-first-product-database / FP1-20261019-R5；R4原件归档 |
| 用户授权 | 架构文档缺少数据库字段设计；本轮补充规范、设计检查和HTML，仍未授权产品研发 |
| 产品映射 | FP-01～12；AI-CFG-01～16；文档§11列实际MC/GL/CR/TP，不新增产品用例 |
| 工作项 | DBD-001字段与导出；002身份；003快照；004权限；005清理；006范围 |
| 设计页面/状态 | 现行first-product-v1保持原字节；新增数据库设计candidate pending review。没有产品UI实现，视觉对照不适用 |
| 模块/组件 | M10/AP存储，M09/AI配置模型，M08/JA辞书，M16/PL凭据，M15/QA验证；不增加UI组件族 |
| 主责 / 复核 | root整合与DDL；db_requirements_audit需求/SQL测试/报告守卫；db_schema_review数据库/导出；db_context架构一致性；db_tooling_review独立代码复核 |
| 允许路径 | 本change verification.json明确列举，禁止apps/crates/native/resources实现变更 |
| 复用 | 既定SQLite/rusqlite/Repository/唯一writer；旧OpenSpec门禁和报告生成器；不引入新框架 |
| 前置 | 原PRD/架构和测试登记可读；实际SQLite构建和容量政策、正式包adapter留B0，不当作本轮已就绪 |
| 验证 | 隔离SQLite正反例、EXPLAIN FK/trigger、确定性导出、source-scope/指纹、五类违规门禁、实际TS/Rust编译；Playwright离线报告 |
| 完成条件 | 字段/键/关系/索引/生命周期完整可读，SQL合法正反例通过，导出一致，报告当前证据绑定且可离线查看，原产品源码与153项not_run保持 |
| 实际结果 | 见report quality-qa/database-design-verification.json和report-qa/verification.json；未执行产品迁移、用户库、游戏或模型 |
