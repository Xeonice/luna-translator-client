# SR-6-PRE迭代卡

日期2026-10-27，父基线FP1-20261027-R53。用户授权：开始处理，直至整个重构计划完成。

追溯：FP-03, FP-04, FP-05, FP-07, FP-12；现行设计docs/design/first-product-v1/interaction-review.html#01-window-preview；模块M12；组件C35。Root持有精确Main合入/本change/authority；PL及QA实现由独立Sandbox Owner完成，AP逐源码和整合独审，GUI/正式包由Root执行。。并行协作者不得覆盖或回退他人。

产品精确路径：
- apps/desktop/src-tauri/src/platform/runtime_observation.rs
- apps/desktop/src-tauri/src/platform/mod.rs
- apps/desktop/src-tauri/src/bootstrap.rs
- apps/desktop/src-tauri/src/platform/window_collector/mod.rs
- apps/desktop/src-tauri/src/platform/window_preview.rs
- apps/desktop/src-tauri/src/ipc/reading.rs
- apps/desktop/src-tauri/src/ipc/word_card.rs
- apps/desktop/tests/visual/capture-reviewed-states.mjs
- apps/desktop/tests/support/design-port.ts
- apps/desktop/tests/support/ReviewedSurfaceProbe.tsx
- scripts/quality/lifecycle_impact.py
- scripts/quality/test_lifecycle_impact.py
- docs/development/lifecycle-impact-contract.json
- scripts/quality/runtime_stability.py
- scripts/quality/runtime_stability_evidence.py
- scripts/quality/runtime_observer.swift
- scripts/quality/test_runtime_stability.py
- scripts/quality/product_execution.py
- scripts/quality/test_product_execution.py
- scripts/quality/spec_contract.py
- scripts/quality/test_spec_contract.py
- scripts/quality/verify.py
- docs/development/runtime-stability-evidence.md

依赖：Main R53已含SR1/2/3/4/5，完整62步61通过，唯一原视觉失败保留；本片10产品/测试文件已有独审，13工具待冻结独审后精确合入，未审前不采用authority。最终verify-refactor-stability-rc另执行唯一签名RC四游戏/真实睡醒，不继承旧绿灯。。复用既有端口与模型，保持公开行为和原断言。Root拥有本change、现状清单/导出、当前阶段/进展、技术说明及新基线审阅包。允许路径以verification为准，无整目录产品授权。

验收：先表征、迁移、独审，再实际运行完整注册检查、编译、边界/五负例及视觉检查。失败保留，不改批准SHA/PNG/token/阈值或测试预期迁就实现。最终同RC游戏/原生与模型条件单独验证。

追溯细化：私有word-card snapshot观察归属C19，原生窗口/启动接线归属C34，预览归属C35，会话读取归属C36。仅修正本片职责映射，不改产品行为、历史工作项或原设计。
