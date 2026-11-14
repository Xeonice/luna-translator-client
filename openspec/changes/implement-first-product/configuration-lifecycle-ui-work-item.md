# 配置即时停用与恢复 UI 工作项

- iteration_id：FP1-IMPLEMENT-20261019 / UI-MC-LIFECYCLE；baseline_id：FP1-20261019-R8。
- 授权：用户已授权首版产品实施与对应测试；Root 于本轮分配已有 Rust 权威配置生命周期的 UI 接线。不包含提交/推送、业务语义改写、设计图或 token 变更。
- Owner：product_ui_impl（M13）；业务权威/状态持久化：configuration_lifecycle_impl（M04/M05/M16）；Tauri 装配和 DTO 导出：Root（M01）。协作者共享代码，保留其他改动。
- 允许修改：`apps/desktop/src/features/model-config/**`、配置生命周期相关 `apps/desktop/tests/support/design-port.ts` 和 `apps/desktop/tests/visual/verify-ui.mjs` 新增测试、本文件。不改 generated DTO、ApplicationPort/Tauri 端口、SourceText/HUD/Overlays 或 Root 的 UI-29..31。

| 工作项 | 产品与验收 | 设计入口/状态 | 模块/组件与契约 | 验证与完成条件 |
| --- | --- | --- | --- | --- |
| UI-MC-32 已应用连接即时控制 | FP-06 / AI-CFG-13 / MC-13、19 | first-product-v1 设置窗 → 连接；开放、停用、恢复核对、被意图/凭据阻断、并发冲突 | M13/M04/M05；C26/C31；SetConnectionSuspended + ConnectionRuntimeView | 不经保存/Apply 直接发 CAS 命令；明确显示已应用名称、用途/游戏/引擎影响；恢复只用 applied，失败保留错误，不偷用草稿。 |
| UI-MC-33 已应用用途即时控制 | FP-06 / AI-CFG-13 / MC-13、19 | 设置窗 → 用途；独立安全闸、连接阻断、部分引擎授权、未应用对象、继承全局 | M13/M04/M05；C27/C31；SetPurposeRuntimeEnabled + PurposeRuntimeView | 区分 enabledIntent、runtimeEnabled、blockedConnectionIds、authorizedEngineIds；保存/测试不恢复；只显式恢复已应用用途。 |
| UI-MC-34 凭据撤销接线 | FP-06 / AI-CFG-11、13 / MC-13、19 | 设置窗 → 连接/系统凭据；脱敏身份、共享影响核对、取消、已撤销/删除失败 | M13/M16；C26/C31；Root 的窄 revokeCredential 应用端口 | 只使用投影中的非秘密引用；先真实后端撤销，错误不假报密钥删除成功；列共享连接与用途的真实已应用引用。端口缺失时不绕过 bridge。 |

前置已核对：Rust `contracts/mod.rs` 已定义两个运行控制命令与 runtimeConnections/runtimePurposes；字段来自 Rust，Root 统一 ts-rs 导出。`runtimeEnabled` 仅为独立安全许可，不等同服务连通或所有引擎可运行。Root 将补凭据撤销窄端口，后端 writer 取消与安全库删除归属 Rust。

交互边界：草稿中的开关标明“启用意图”，新建/未应用对象没有伪运行状态。即时控制独立于草稿脏状态，操作后仍保留编辑缓冲。确认对象捕获真实已应用版本和控制版本；等待确认时投影变化要求重新核对，不悄悄对新对象执行旧确认。停用字幕/词典不受影响的语义由既有业务实现保证。

设计与材质：复用当前 `first-product-v1/index.html#settings-window`、中性暗色 CSS Modules、线性图标、C26..C31；运行影响面板和确认 sheet 使用已有 Group/Notice/Sheet。整体布局仍待评审，不自动重拍已确认图或放宽差异阈值。

测试计划（结果另建运行回执，不预填通过）：保留 UI-01..31，新增受控 ApplicationPort 的连接停用/明确恢复/CAS、用途意图与阻断、未应用对象无按钮、失败/取消保留草稿、脱敏共享凭据撤销影响检查；实际生产与测试 tsc、Vite build、架构边界、Playwright 候选与设计对比。受控测试不替代 Rust 排队/取消、系统密钥库或原生正式包实测。

完成条件：生产编译和新增行为有当轮真实证据；源码冻结后通知 Root 重包。视觉差异、后端/原生未执行与依赖缺失分别报告，整体发布状态不擅自提升。
