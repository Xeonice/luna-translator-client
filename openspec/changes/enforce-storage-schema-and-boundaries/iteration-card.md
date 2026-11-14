# 迭代卡：存储验收程序化

| 项目 | 本轮范围 |
|---|---|
| 身份/依据 | enforce-storage-schema-and-boundaries；R5→R6，父基线原字节留存 |
| 授权 | 用户要求存储模块验收及字段边界一并纳入、程序确保一致性；仅工程设施与设计阶段 |
| 工作项/需求 | STE-001～005；FP-01～12的存储部分、AI-CFG-01～16的配置/授权部分；既有TP/MC/CR沿用 |
| 设计状态 | 现行高保真整稿不修改，完整布局/新数据库字段细节仍pending；无产品UI实现 |
| 模块/组件 | M10 AP存储、M09 AI配置、M08 JA辞书、M16 PL凭据、M15 QA；不新增前端组件 |
| Owner/复核 | root结构比较/整合；field_boundary_audit边界；storage_contract_explore验收执行；storage_context上下文；独立code review |
| 允许路径 | verification.json精确授权；无apps/crates/native/resources产品实现 |
| 复用 | database_design元数据、冻结DDL和37业务正反例、现有128/153测试来源、OpenSpec/verify/HTML |
| 依赖 | 真实生产迁移和Rust Repository未实施；PRE13/14、TE04/06/08/10/12按原规划准备，不默认为已完成 |
| 可观察结果 | 字段/关系更改、漏边界、失败或缺真实adapter由程序拒绝；HTML说明实跑范围与产品阻断 |
| 验证 | 隔离SQLite逐字段、完整DDL业务、schema变异负例、生产执行器防伪/缺项、统一工程门禁和TS/Rust编译、Playwright报告 |
| 完成条件 | 程序和登记身份一致、必要断言真实执行、真实生产验收继续缺项阻断、产品计划状态/源码不变 |
