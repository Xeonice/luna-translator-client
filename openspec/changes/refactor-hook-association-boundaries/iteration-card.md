# SR-4迭代卡

日期2026-10-27，父基线FP1-20261027-R52。用户授权：开始处理，直至整个重构计划完成。

追溯：FP-03, FP-05, FP-09, FP-12；现行设计docs/design/first-product-v1/interaction-review.html#01-window-preview；模块M02, M15；组件C13, C14, C35, C36。HK实现；AP/PL独审；QA执行；Root规范与exact authoritypacket。并行协作者不得覆盖或回退他人。

产品精确路径：
- crates/app-infra/src/hook_backend.rs
- crates/app-infra/src/hook_backend/window_association.rs
- crates/app-infra/src/hook_backend/preview_identity.rs
- crates/app-infra/src/hook_backend/journal.rs
- native/README.md

依赖：Main R52已采用SR1/SR2/SR3/SR5；本片独立私有target 671infra/158Native及39项代码独审通过，精确前像须匹配Main；合入后Main独占完整重跑，不能继承Sandbox为最终验收。。复用既有端口与模型，保持公开行为和原断言。Root拥有本change、现状清单/导出、当前阶段/进展、技术说明及新基线审阅包。允许路径以verification为准，无整目录产品授权。

验收：先表征、迁移、独审，再实际运行完整注册检查、编译、边界/五负例及视觉检查。失败保留，不改批准SHA/PNG/token/阈值或测试预期迁就实现。最终同RC游戏/原生与模型条件单独验证。
