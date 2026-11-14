# SR-QA-FIX迭代卡

日期2026-10-27，父基线FP1-20261027-R54。用户授权：开始处理，直至整个重构计划完成。

追溯：FP-12；现行设计docs/design/first-product-v1/interaction-review.html#01-window-preview；模块M12；组件C35。QA只拥有两工具文件；Root拥有此change/authority/AP独审；其它协作者不得覆盖回退。。并行协作者不得覆盖或回退他人。

产品精确路径：
- scripts/quality/verify.py
- scripts/quality/test_lifecycle_impact.py

依赖：R54完整当前源63/66已结束；新工具顺序错误有原始回执 .build/quality/20261027T010022.601484Z.json，本轮先修实际问题，再继续重构目标与紧凑预览候选。。复用既有端口与模型，保持公开行为和原断言。Root拥有本change、现状清单/导出、当前阶段/进展、技术说明及新基线审阅包。允许路径以verification为准，无整目录产品授权。

验收：先表征、迁移、独审，再实际运行完整注册检查、编译、边界/五负例及视觉检查。失败保留，不改批准SHA/PNG/token/阈值或测试预期迁就实现。最终同RC游戏/原生与模型条件单独验证。
