# 视觉状态与学习窗口收敛

## 身份与范围

- Work item：FP1-VISUAL-CONVERGENCE；变更 `implement-first-product`；基线 `FP1-20261019-R9`（只读，不刷新指纹）。
- 授权：首版实施及 root 本轮分派；Owner `product_ui_impl`，root 独立审阅与正式包验证。
- 依据：FP-01/02/03/04/07/08/11/12；AI 阅读 PRD 的固定原句、用户主动请求、词典与 AI 分离；现行整稿 `learning.js` 的 `lp-toolbar`、`lp-scroll`、`lp-footer`。
- 模块/组件：M13 product-ui；C16/C17 原文与译文、C19 词典、C20 固定原句、C21 引用、C22 请求状态、C34 独立窗口；业务状态仍来自生成 DTO 与 ApplicationPort。
- 允许路径：`apps/desktop/src/features/study/{StudyScreen.tsx,styles.module.css,index.ts}`（index 仅必要时）、`apps/desktop/src/windows/learning/**`、`apps/desktop/tests/visual/verify-ui.mjs`、`apps/desktop/tests/support/design-port.ts`。不改 history 端口，若必须适配先与 root 协调。
- 不改：批准图、设计源、tokens、差异阈值、既有行为断言、required references、HUD/SourceText/reader/history/primitives/reset/generated。不提交、不推送。

## 已读取的失败证据

- `.build/quality/20261018T203305.761332Z.json` 的真实 capture 位于 `.build/quality/product-execution/20261018T203410.624034Z-e88216dd1c-product-ui-capture/capture/verification.json`。
- 实际 35 个行为通过，15 个视觉失败。10 个比较几何不同；另有批量入库完成后对比审查中、设置页对比 HUD 菜单、连接页对比用途页等状态不一致。
- `empty-library`、`learning-fixed` 等 reference 是 1600×925 的多窗桌面，产品捕获为独立原生内容窗。不得贴参考图、画假系统装饰或忽略桌面区域来宣称像素通过；剩余构图差异继续失败并说明。

## 实现与验收

1. 捕获视口匹配真实内容窗口，root 位于视口原点；Portal 与窗口共用视口，避免旧 1600×1000 页面中裁切对话框。模态捕获明确使用实际 dialog，不重缩放 PNG。
2. 保留六个固定比较 ID/reference：`empty-library→38`、`add-game→40`、`target-review→42`、`settings-connections→31`、`reading-hud→09`、`learning-fixed→11`。通过真实入口进入同一业务状态；受控设计数据仅在 test transport，不进正式包。
3. 学习窗从双栏收敛为整稿的单列：顶部“本句精读/内置词典”页签及模型入口；中部固定原文、译文、新句提示、当前页签内容；底部来源及阅读返回。按真实 tokenIndex 决定初始页签。切页不请求模型；固定对象、结果引用、失败/取消、主动解释继续工作。
4. 保留 UI01..35 所有断言。追加 root 分派的 UI36：stopped/failed/crashed/unknown 不显示接收，active+paused 显示暂停，opening 显示接入中；终态禁用暂停但原文可读。UI37..38 仍预留 root。
5. 检查：严格 TypeScript、正式 Vite build、完整 Playwright 行为及截图差异。新增学习页签与配置空态检查；窄窗/200% 原有用例必须通过。应用 react-best-practices 检查 hooks、语义页签、可访问性与稳定对象身份。
6. 候选写独立 `.build/product-ui-candidates/<run>/`；每图保存 reference/candidate/diff/hash/实际尺寸/ratio，maxRatio=0 不变。单个状态未通过仍标 failed，不能以截图存在代替相符。

## 限制与完成条件

浏览器使用真实生产 React 组件和受控端口，不证明正式包材质、真实游戏、分词或模型通过。完成需实际 UI01..40 行为与编译回执、视觉失败分类与可读截图、源码 freeze 通知。多窗桌面背景/原生系统装饰未同态的比较保留失败，不自动重拍批准图。结果另写本次执行目录，不预填计划通过。

## 平台装配配套（root）

- 按当前整稿恢复原生主窗默认1040×720、学习窗默认600×670；学习窗最小390×480以支持现有窄窗候选。只改 `apps/desktop/src-tauri/src/platform/window_manager.rs`。HUD尺寸与菜单/滚动结构本批不变。
- 视觉测试记录实际原生尺寸来源；正式包窗口尺寸与焦点回归另外执行，不以浏览器尺寸冒充原生结果。

## 本轮追加的 root 回归范围

- UI37：真实历史端口对111条以上记录先过滤后分页；55个同游戏会话的目录分页；游戏/来源/会话组合；scope游标拒绝；显式悬挂旧请求再切换范围，拒绝迟到覆盖。受控端口只调度响应时机，不替代产品分页实现。
- UI38：同unit不同revision分别可选，首屏外当前句显式定位不重复，内部ID/hash不进入可见详情；人工修正保存失败保留，筛选和记录导航禁用，离开页面再返回仍保留编辑。
- UI39：学习页签的点击/键盘语义与固定对象保持；仅明确点击解释后发出固定token请求。
- UI40：使用现行设计只读样句，核对SourceText/译文18/24逻辑px及200%后的36/48；560×147默认HUD句末标点与前一词同行。字号实现由root修正，本Owner只写回归。
- 新增ReadingUnit来源摘要由Rust生成；测试fixture同步真实来源摘要，目录仅驱动筛选，不能把目录首50项当记录来源权威。

## 本次实际结果

- 最终执行：`.build/product-ui-candidates/2026-10-18T21-21-15-807Z/verification.json`；结束时间 `2026-10-18T21:21:46.130Z`。
- 严格 TypeScript（产品与tests）、真实production Vite build、受控transport build均由runner实际执行并保存日志；40/40行为通过；`sourceDrift=[]`、`browserErrors=[]`。
- 独立边界检查实际 `52 files / 0 findings`。未发现本次路径中的debugger。没有安装框架、改token/批准图/阈值或提交推送。
- 回执SHA256：`6b71267345e3d0d2e5aa07e33e34adf0b9e97178be5b672b49e2ffd91c5e8687`；sourceSha256 map按key排序的紧凑JSON摘要 `2f27de9bb584816aa8f427994380dd4ed318960b39341dd639a0e1140dfb3eef`（85源文件、95设计文件）。Chromium141.0.7390.37 / Playwright1.56.1，未覆盖浏览器可执行路径。
- 两轮中发现的严格locator scope错误仅修新断言的可见范围；UI28确实捕获窄窗标点缺陷，root修排版后原断言通过。中途root来源标签修改被sourceDrift真实拒绝，未当作最终通过。

### 视觉结果与剩余范围

15/15仍为failed，`maxRatio=0`保持。9项几何不同，6项同尺寸但像素不同。所附HTML逐项包含captureConditions（真实viewport/selector/bounds/state/limitation）、只读基准副本、候选、差异图与hash。

| 固定比较 | 基准 / 候选尺寸 | 实际差异 | 已修正状态 / 剩余差异 |
|---|---|---|---|
| empty-library | 1600×925 / 1040×720 | 62.5743% | 空库同态；完整桌面与独立主窗构图不同 |
| add-game | 760×966 / 688×648 | 45.0685% | 通过选程序进入Sakura资料核对；实际表单/窗口容纳几何不同 |
| target-review | 760×880 / 688×574 | 45.9529% | 同游戏选择后确认前；产品独立核对步骤缺参考中的选择列表与宿主映射 |
| settings-connections | 1000×720 / 1000×720 | 7.2447% | 保留历史ID，实际捕获reference31的翻译用途页；字段/导航/原生装饰差异保留 |
| reading-hud | 560×147 / 560×147 | 11.9060% | 同首句原文+译文、注音和控制行；逻辑字号与句末禁则通过，背景场景/行距/来源密度仍不同 |
| learning-fixed | 1600×925 / 600×670 | 92.6153% | 当前整稿单列页签、旧句固定、新句提醒和未配模型卡；参考为含游戏/HUD的完整桌面 |

其余9项同样保留失败：批量入库已改在保存前捕获，但产品逐项核对与设计并列批次视图不同；显示偏好已改为实际大字减少透明度HUD菜单；模型/连接已从错误滚动位置回到对应实体编辑顶部。词卡、日语禁则和术语的新增行为状态没有同态批准参考，回执明确记录，不把比较换名当作通过。历史参考与当前产品仍有布局和受控数据差异。

下一步最小修正需分别处理：真实桌面多窗口捕获条件；主窗附属表单的设计几何/滚动约定；设置与批次核对的真实布局；学习窗译文16逻辑px与全局阅读默认24逻辑px的作用范围。以上涉及其他Owner或设计评审，未通过本次擅改共享primitive、偏好语义或批准基准。正式包材质/焦点、真实游戏/模型回归仍由root独立执行；本回执不提升其状态。
