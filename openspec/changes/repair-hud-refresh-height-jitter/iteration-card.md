# R66 HUD状态刷新稳高迭代卡

| 字段 | 值 |
|---|---|
| iteration / baseline | R66-HUD-REFRESH / FP1-20261028-R65；SHA 693da6d19024252692da9b920d28c9460d17afbf72c60b6052ab9236adb5b520 |
| 授权 / 范围 | 已授权阅读缺陷最小隔离候选；父任务明确仅本目录写入、不扰用户Luna51958；Main不修改 |
| work-item / task | repair-hud-refresh-height-jitter / HJ-001〜003 |
| 产品 / 验收 | FP-04 连续阅读、FP-05 阅读控制窗口、FP-12 故障恢复；RBC-15与RBC-38实际组件及既有100义务 |
| 设计页面和状态 | docs/design/first-product-v1/index.html?scenario=reading；interaction-review.html#25-toolbar-560；正常/读取/后台刷新/failed/unavailable/retry/stream；20261019已确认整稿保留，本次瞬态呈现待独审，产品实测not_run |
| 模块 / 组件 | M03 reading / M13 product-ui / M15 QA；C17 TranslationStack / TranslationTaskStates、C18 HudWindowControls；Rust仍唯一业务权威 |
| Owner / 审阅 | UI执行协作者hud_refresh_height_fix；Root另行安排独立code-reviewer与治理审阅；QA注册adapter验证 |
| 精确生产路径 | apps/desktop/src/features/reading/ReadingPanel.tsx；apps/desktop/src/windows/hud/HudWindow.tsx |
| 精确测试路径 | apps/desktop/tests/visual/verify-reviewed-reading.mjs，仅扩既有RBC-15，保留旧断言/100ID/expected图 |
| WI路径 | openspec/changes/repair-hud-refresh-height-jitter/**；共享authority overlay另包待Root审阅，不刷新pin |
| 输入 | 完整1960文件before树 c4d96ac9a9564f511f4e7bcbe4e3ec3536c58358a10907364f5ee131c482a7d3；诊断manifest ab9e37235fdbd4e4e37dd1004f59d575941b0139f5b58eb532cba7181ec13177；Chromium141；真实Rust/SQLite fixture |
| 依赖 / 门禁 | 当前scope/设计/架构规则已读；统一verify需Root共享authority独审采用后才可完整通过；native正式包与四游戏单列 |
| 可观察结果 | 同原译的无关commit、held query保持正文自然高度和fit次数；查询反馈仍可访问；错误unavailable/retry可见可操作；默认主窗仍可见刷新Notice |
| 验证 | 未修复真实组件回归失败→修复registered product-reading-component-tests完整100；TS、边界/五负例；独立候选截图；正式包native/四游戏not_run |
| 完成条件 | 源码diff精确、before/after完整、实际测试回执及manifest闭包；Root独审采用与同源包实测前不宣称产品已修复通过 |

不改整稿/PNG/token/阈值/guard/查询hook/native autoFit/配置/运行App/游戏。流式换行、ruby就绪等真实内容高度变化保留。历史R65运行blocked与旧通过不继承。

独审细化：2026-10-29 Root同意refreshing-only stale前缀稳定；修改前四组真实边界反例已留证。持续挂载status槽可访问，播报not_run。真实query error/unavailable仍可见，原RBC38与100ID保持。
