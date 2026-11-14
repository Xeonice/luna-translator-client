# SR-2迭代卡

日期2026-10-27，父基线FP1-20261027-R48。用户授权：开始处理，直至整个重构计划完成。

追溯：FP-03, FP-04, FP-12；现行设计docs/design/first-product-v1/interaction-review.html#01-window-preview；模块M12, M13；组件C35。UI实现；AP/PL独审；QA执行；Root持有文档和精确authoritypacket。并行协作者不得覆盖或回退他人。

产品精确路径：
- apps/desktop/src/bridge/tauri-port.ts
- apps/desktop/src/bridge/preview-subscriptions.ts
- apps/desktop/tests/visual/verify-reviewed-reading.mjs

依赖：SR1代码/157原生和独审通过；R48完整检查实际回执另记，原视觉来源失败不得继承为通过。复用既有端口与模型，保持公开行为和原断言。Root拥有本change、现状清单/导出、当前阶段/进展、技术说明及新基线审阅包。允许路径以verification为准，无整目录产品授权。

验收：先表征、迁移、独审，再实际运行完整注册检查、编译、边界/五负例及视觉检查。失败保留，不改批准SHA/PNG/token/阈值或测试预期迁就实现。最终同RC游戏/原生与模型条件单独验证。
