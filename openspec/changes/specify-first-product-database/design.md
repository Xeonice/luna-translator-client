# 设计与职责

AP持有业务事务，AI持有任务/结果契约，JA持有只读资源契约，PL持有凭据生命周期，app-infra为唯一SQLite适配器。DDL放docs/architecture/database，绝非生产migrations目录。Rust依旧业务权威，Tauri/UI不得直接SQL。

三份注释DDL为字段唯一来源，SQLite introspection导出表/字段/FK/索引/触发器及SVG。每profile独立可写库、只读辞书与vault分离；十进制u64、UTF8 spans、严格类型、复合FK、配置封存、受控GC、CAS、授权epoch与结果归属有正反例。应用仍负责有效配置解析、请求语义摘要、网络/模型可运行性、故障恢复和正式迁移；设计检查不代表这些已实现。

HTML保留原完整设计、许可证、历史报告/证据；新增字段搜索、表目录、ER、原DDL下载和本轮设计检查边界。Playwright只验证报告可读与离线关系，不转为产品验收。变更Owner root；SQL与报告root，需求/测试审计db_requirements_audit，导出/复核db_schema_review，架构文档db_context。并行协作者不得回退他人修改。
