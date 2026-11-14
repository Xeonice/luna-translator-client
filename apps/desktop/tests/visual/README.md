# 首版产品 UI 行为与视觉候选

`npm --prefix apps/desktop run test:ui`，或 `node apps/desktop/tests/visual/verify-ui.mjs --output <全新目录>`。

脚本执行严格 TypeScript（含测试适配器）、正式入口的 Vite 构建、独立 UI 测试入口的冻结构建，再用 Playwright 驱动本机 Chrome。正式构建和测试构建写进本次独立证据目录，避免更改正在打包的 `apps/desktop/dist`。只有 `tests/support/bootstrap.tsx` 装配受控 `DesignPort`；正式 bootstrap 固定装配 TauriApplicationPort。

`verification.json` 分开记录 `behaviorStatus` 与 `visualStatus`。两个状态同时通过，整体才通过。现有设计 PNG 只读复制，原文件 SHA 与候选、差异图 SHA 记录在回执；不同尺寸仍为失败，不缩放、掩蔽或调整零差异阈值。现行参考含模拟桌面及 HUD 遮挡，完整视觉仍需评审，不把截图存在当作一致。

测试覆盖真实 React 组件的入库审阅、单次确认接入意图、显式选流、草稿保留、失败反馈、参数三态、配置 CAS、固定对象、订阅恢复、分页、阅读字号及无样例生产导入图。受控端口不会访问游戏、网络、系统密钥库或语言资源；这些通过不等于正式包、Hook、模型、系统材质、轮换策略或数据库验收。

输出含 `index.html` 对照页、`verification.json`、各状态 baseline/candidate/diff、编译与预览日志。回执携带实际执行时间、可选执行门禁 nonce、源码/设计 hash 和运行中源码变化拒绝项。没有覆盖批准图，所有批准仍由项目既定流程决定。

`display-preferences` 在写候选 PNG 前校验实际 window role、viewport、可见菜单数及触发器展开状态，并将实测值写入 `captureConditions.verifiedConditions`。原 `32-large-opaque-hud.png` 实际保留展开菜单；不能仅从原设计脚本“关闭设置”的动作推断菜单已关闭。新增 UI-43 通过真实展开操作验证 open/closed 状态不匹配、错误视口及错误窗口角色均被拒绝，原有 42 项行为与 15 个视觉状态保持。
