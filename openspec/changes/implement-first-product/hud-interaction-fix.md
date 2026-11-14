# HUD 菜单与滚动修正

- 授权：用户本轮报告「三个点需要一直点着」「整个视图都能滚动」，属于既定首版缺陷修复。
- 需求：FP-04/05/07；组件 C06/C18/C19/C34；Owner root，独立复核 architecture_context。
- 设计：`docs/design/first-product-v1/index.html` 的 HUD 与更多菜单，遵循原操作栏固定、正文区域滚动、短词卡独立浮层的交互，不改设计源/token/批准截图。
- 允许路径：`apps/desktop/src/ui/primitives/Overlays.tsx`、对应 CSS、`src/ui/reset.css`、`src/windows/hud/styles.module.css`、`src/features/language/styles.module.css`、`apps/desktop/tests/visual/verify-ui.mjs`。
- 修正：菜单从完整按压动作触发，保留松开后展开、菜单选择/Escape/外部关闭及键盘访问；窗口根不滚动，阅读区/浮层按实际可用高度滚动，不撑大视口。
- 验证：Playwright 真实鼠标 down/up、移动后保持、菜单操作/Escape/外部关闭；长文/小窗口/菜单打开时 wheel，断言页面 scrollTop 不变、操作栏边界不变，仅正确内部滚动；实际重包后原生 HUD 回归。
- 原计划及失败证据保留，不提前填通过。源码变化后重新构建候选包并记录新身份。
