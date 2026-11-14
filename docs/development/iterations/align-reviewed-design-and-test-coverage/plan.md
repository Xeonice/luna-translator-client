# 迭代卡：已评审设计、技术同步与测试闭包

- 授权：用户“刚刚的设计稿进行review是没有问题的”，要求补测试逻辑并更新技术设计；本轮不改产品。
- 输入：PRD索引/FP范围/整稿/架构基线/迭代规则已读，R16与源码快照留存。
- 需求：FP-03/04/05/07/09/11/12，RW-01～34；设计为first-product-v1最新六项修订及Select间距稿。
- 模块/组件：M02/M03/M04/M05/M08/M12/M13/M14/M15，既有C ID按专项技术表对照；不重编号。
- Owner：Root（QA与集成/批准身份/HTML）、architecture_revision（5份架构文档）、interaction_coverage_guard（覆盖注册器与自测）、release_execution_guard（发布执行闭包）；coverage_context_review独审。
- 允许路径：本change verification.json精确清单。保护产品/apps/crates/native、原PRD、License、token、批准PNG、acceptance-v3、数据库冻结schema及原计划CSV。
- 依赖：本机现有Playwright/Chrome、TS/Rust离线缓存；真实新功能adapter尚未实施，发布必须拒绝该缺项。
- 复用：现有设计UI、运行器/receipt/source_files、153执行计划；不装依赖、不建空目录、不复制产品业务。
- 完成条件：34RW逐层适用性闭包；新设计检查真执行且每项有断言与证据；漏variant/伪证据/跨层替代会失败；技术HTML能查看目录和数据流；合法R17差异独审与原像保留；工程检查与编译通过、未实现层not_run。
