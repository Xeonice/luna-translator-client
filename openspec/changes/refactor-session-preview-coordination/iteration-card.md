# SR-3迭代卡

日期2026-10-27，父基线FP1-20261027-R50。用户授权：开始处理，直至整个重构计划完成。

追溯：FP-02, FP-03, FP-05；现行设计docs/design/first-product-v1/interaction-review.html#01-window-preview；模块M02, M12；组件C13, C14, C35。PL实现；AP/HK独审；QA执行；Root独占规范与authoritypacket。并行协作者不得覆盖或回退他人。

产品精确路径：
- apps/desktop/src-tauri/src/bootstrap.rs
- apps/desktop/src-tauri/src/ipc/reading.rs
- apps/desktop/src-tauri/src/ipc/commands.rs
- apps/desktop/src-tauri/src/platform/mod.rs
- apps/desktop/src-tauri/src/platform/session_preview_coordinator.rs
- apps/desktop/src-tauri/src/platform/session_cover_coordinator.rs

依赖：SR1/SR2源码与维护已通过；SR-V R50 source解析已独审采用，原37/77真实比较已执行但仍有库存/scroll/behavior/pixel失败，独立诊断并保留。SR3 Sandbox attempt03仅在AP终审通过且before字节一致后合入，随后Main实际重跑。。复用既有端口与模型，保持公开行为和原断言。Root拥有本change、现状清单/导出、当前阶段/进展、技术说明及新基线审阅包。允许路径以verification为准，无整目录产品授权。

验收：先表征、迁移、独审，再实际运行完整注册检查、编译、边界/五负例及视觉检查。失败保留，不改批准SHA/PNG/token/阈值或测试预期迁就实现。最终同RC游戏/原生与模型条件单独验证。
