# SR-V迭代卡

日期2026-10-27，父基线FP1-20261027-R49。用户授权：开始处理，直至整个重构计划完成。

追溯：FP-12；现行设计docs/design/first-product-v1/interaction-review.html#01-window-preview；模块M15；组件C35。QA工具实现；AP独审；Root仅精确规范/清单/基线，不能自动批准新图片。并行协作者不得覆盖或回退他人。

产品精确路径：
- apps/desktop/tests/visual/design-source-versions.mjs
- apps/desktop/tests/visual/capture-catalogue.mjs
- apps/desktop/tests/visual/verify-ui.mjs
- apps/desktop/tests/visual/design-source-versions.test.mjs

依赖：SR2 R49完整实际回执；visual-version-contract-review只读306项核对和技术设计已完成。原来源失败历史保留。。复用既有端口与模型，保持公开行为和原断言。Root拥有本change、现状清单/导出、当前阶段/进展、技术说明及新基线审阅包。允许路径以verification为准，无整目录产品授权。

验收：先表征、迁移、独审，再实际运行完整注册检查、编译、边界/五负例及视觉检查。失败保留，不改批准SHA/PNG/token/阈值或测试预期迁就实现。最终同RC游戏/原生与模型条件单独验证。
