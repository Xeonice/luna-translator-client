# SR-5迭代卡

日期2026-10-27，父基线FP1-20261027-R51。用户授权：开始处理，直至整个重构计划完成。

追溯：FP-04, FP-05, FP-07, FP-11；现行设计docs/design/first-product-v1/gallery.html#10-local-word-card；模块M12；组件C18, C19, C34。PL实现；AP/UI独审；QA执行；Root持有主工程合入和authoritypacket。并行协作者不得覆盖或回退他人。

产品精确路径：
- apps/desktop/src-tauri/src/platform/window_manager.rs
- apps/desktop/src-tauri/src/platform/owner_geometry.rs

依赖：Main R51已采用SR1/SR2/SR3；Sandbox R49实现已独审，精确前像须一致，Main完整Native与工程检查独占重跑；不采纳共享Cargo cohort的旧通过。。复用既有端口与模型，保持公开行为和原断言。Root拥有本change、现状清单/导出、当前阶段/进展、技术说明及新基线审阅包。允许路径以verification为准，无整目录产品授权。

验收：先表征、迁移、独审，再实际运行完整注册检查、编译、边界/五负例及视觉检查。失败保留，不改批准SHA/PNG/token/阈值或测试预期迁就实现。最终同RC游戏/原生与模型条件单独验证。
