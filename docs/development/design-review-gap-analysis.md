# 六项设计评审缺口：为什么回检没有阻断

日期：2026-10-19。依据用户本轮六项反馈及当前源码只读检查。本文是设计修订与后续验收整改方案，不是产品修复、实机故障定位或发布通过记录。

## 结论与现有回执边界

旧版51/51表示当时登记的研发检查执行通过；视觉15状态/44变体与57项行为只覆盖注册输入及浏览器组件。它不证明整稿所有页面、所有组件状态、真实Core→Rust→词法worker→HUD、全局快捷键或macOS合成器。原生记录确为blocked，旧回执不得因本轮文档变化被改写或沿用为新稿通过。

问题发生在三层：第一，需求到测试的映射检查了ID与命令存在/执行，却缺少逐需求可观察结果的覆盖约束；第二，受控替身绕过了真实输入与平台集成；第三，视觉参考采用用户确认的同态内容窗口和有限状态，未包含所有原稿组件/展开菜单/真实背景变化。零像素算法没有漏算已拍的像素；未拍、未登记、替身绕过的差异根本没有进入比较。此前把有限状态的“视觉通过”呈现得过于接近整产品一致，是结果沟通和验收范围的缺口。

## 逐项证据与修订

| 用户问题 | 当前代码与回检事实 | 缺失的可执行验收（后续产品阶段） |
|---|---|---|
| 真实运行窗口的实时截图 | `apps/desktop/tests/visual/capture-catalogue.mjs:22–38,81–107` 为固定背景并截取client selector；`docs/design/first-product-v1/acceptance-v3/catalogue.json:1140–1162` 明确不拍实际桌面包裹。设计既有19张画面为历史配对，不是实时抓取。 | 绑定实际窗口ID/目标身份，记录帧时间和权限；在游戏画面改变时证明预览更新；拒绝权限/最小化/退出/旧帧均有状态，旧图不标live。 |
| HUD快捷键与拖动 | `apps/desktop/src/features/preferences/PreferencesScreen.tsx:17` 编辑保存快捷键；平台配置仅存储不足以证明注册成功。`apps/desktop/src-tauri/src/platform/menus.rs:25–27` 另有固定菜单键。`apps/desktop/src/windows/hud/HudWindow.tsx:42` 已有footer拖区、capabilities已有拖动权限，不能说完全没有代码。旧检查没有真实跨位置/多屏移动断言。 | 正式包从系统按配置键触发并查状态/冲突/失败；拖动前后原生bounds真实改变，屏幕边界与恢复正确；点词、按钮、正文滚动不误触拖动。 |
| Select不是Apple风格展开菜单 | `apps/desktop/src/ui/primitives/Controls.tsx:14–15` 实际返回原生select；`controls.module.css:1` 只控制收起外观。`verify-ui.mjs:249,338,342` selectOption改值，没有展开菜单快照与键盘旅程。 | 对共享Select的触发/展开/选中/键盘/焦点/禁用/长标签逐项断言并拍图；在所有使用窗口复用，不能仅改appearance或使用原生弹层。 |
| 当前会话缺全量日志和提取文本入口 | `features/capture/CaptureSheet.tsx:55` 只展示每流sample；`crates/app-infra/src/storage/queries/mod.rs:625` 每流最新LIMIT1。`queries/capture.rs:185–203` 已保存raw events/payload；`features/history/HistoryScreen.tsx:60,93,123–130` 已有按会话游标分页。缺的是当前会话原始事件查询/入口/实时跟随及不截断总量的展示。 | 产生超过两页且含未选中流/同页增量/重复/恢复事件的真实回调；逐页集合与后端序列一致、无漏无重；阅读筛选不删原始事件；翻旧页不被新数据抢走，回底部恢复跟随。 |
| 半透明及注音分词开关无效果 | `apps/desktop/tests/support/bootstrap.tsx:21–38` 接DesignPort；`design-port.ts:8–9` 与 `verify-ui.mjs:187–205` 直接构造完整tokens/readings并publish。`features/language/SourceText.tsx:5–21,42–57` 遇tokens空或span校验失败回退纯文本，无明确词法状态。`verify-ui.mjs:1331` 明示没有真实tokenizer/dictionary/native。 | 同包真实日文→词法worker→span/reading→HUD DOM，切换开关前后有可观测变化；空/未知reading/非法span/worker故障不损坏原文且有解释。原生窗口放两种真实背景并改变背景验证透色，减少透明度实底单验。 |
| 组件、图标、按钮仍有差距 | 实际对照只比较注册的15状态/44变体；没有逐组件状态与原稿覆盖清单，新增/未登记弹层不会自动变成差异。共同token只约束数值，不能证明间距/层级/交互状态一致。 | 每个首版页面及共享组件映射approved-reference和状态，缺项本身失败；按钮普通/悬停/按下/焦点/禁用、菜单展开、长内容及异常状态分别检查，变化必须人工评审。 |

表中省略的前端路径统一位于 `apps/desktop/src/`，脚本为 `apps/desktop/tests/visual/verify-ui.mjs`。行号对应本轮修改前产品源码，产品源码本轮保持不动。对静态回退分支的确认不等于已确定用户真实词法故障由哪一条坏输入触发；根因需要后续读取本次真实事件和worker状态确认。

## 本轮独立审阅又发现的断言缺口

首轮新增设计QA曾得到26/26，但独立审阅仍发现：空Select菜单按Enter/Space会抛错；暂停跟随后虽然行数不变，末尾切片仍替换可见序号；重试成功后错误标记残留。这说明“有检查名称/计数通过”也不等于检查足够。候选已要求补空态×键盘组合、比较真实事件序列与scrollTop及新到计数、成功后清错误状态。所有先前回执保留，修复后必须按新源hash重跑，不能删除旧失败或继承旧成功。

用户随后明确要求常用操作前置到HUD且菜单不重复；本轮将工具栏和更多内容清单一并纳入RW-32与紧凑布局检查。

## 新验收如何避免同类问题

1. **覆盖清单必须检查“缺项”**：RW每条验收→设计入口/状态→实际组件→数据来源→浏览器/worker/原生/正式包验收层。只填FP ID或检查名称不足；没有真实adapter则not_run/blocked，不能接受截图守卫替代。
2. **同一行为保留两种证明**：UI可控输入验证布局和故障态；真实输入链验证事件/任务身份、token跨度、偏好生效和平台动作。两者结果字段分开，禁止以fixture成功填原生/真实管线passed。
3. **动作与结果都需断言**：Select必须真的展开后选择；拖动必须比较窗口bounds；快捷键必须从系统输入注册键；预览必须检查新帧ID和时间；分页全量必须对集合与序列。不能只检查配置存储、按钮文字或偏好值。
4. **材质与内容拆开检查**：浏览器内容截图仍可用零差异；原生背景采样、减透明度、焦点、置顶、多屏单独留证，不用固定壁纸覆盖原生背景。
5. **设计修订不能追认旧包**：本轮新增截图全部为candidate；原PNG/token/catalogue和R16保持只读。新设计造成的baseline drift如实阻断，待用户评审和独立差异审阅后才产生新revision/参照。
6. **验收状态可读**：报告分别列设计覆盖、组件行为、真实worker/服务、原生窗口、四游戏和发布，任何必需not_run/blocked均阻断相应放行；不得汇总51/51后隐去范围。

本轮仅实现上述PRD和设计演示、增加设计QA与追溯；没有在产品中实施新功能，也没有改门禁消费者、批准图或阈值以获得通过。新RW验收和旧153产品组合保留not_run。后续将该清单接入产品真实运行/发布adapter时，必须实际注入这些负例并确认失败，不只新增登记文件。

## 评审入口和依据

- [当前可交互设计](../design/first-product-v1/index.html)
- [本轮PRD](../product/reading-window-session-prd.md) · [RW候选验收](../product/reading-window-session-acceptance.csv)
- [本轮OpenSpec](../../openspec/changes/revise-first-product-interaction-gaps/proposal.md) · [迭代卡](iterations/revise-first-product-interaction-gaps/plan.md)
- [原两项工程验收](../../.build/acceptance-closure-20261019/report/runs/20261019T121014.101158Z-attempt-02/index.html)（保留旧身份，不承接新稿）
- Apple [Pop-up buttons](https://developer.apple.com/design/human-interface-guidelines/pop-up-buttons)、[Menus](https://developer.apple.com/design/human-interface-guidelines/menus)：用于当前值、选择后关闭、菜单层级与状态规范；本项目继续采用用户确认的中性磨砂材质。
