# R68 HUD翻译占位迭代卡

| 字段 | 值 |
|---|---|
| iteration/baseline | R68-HUD-TRANSLATION / FP1-20261028-R67 / 4040be8a7fcc017d2564d79580c5d709fe8ef0648a5bd21af3d72371a938ba59 |
| 授权 | Root指派已授权HUD阅读缺陷隔离候选；用户正在R67实机观察，当前Luna/游戏禁止触碰 |
| 需求/验收 | FP-04/05/06/12；RBC38扩展真实状态与空态；全部既有100ID及RBC15/R66稳高保留 |
| 设计/状态 | 当前整稿index.html#main-content与interaction-review.html#25-toolbar-560；20261019确认保留；pending/running/partial/completed/failed/cancelled/unrequested/unavailable/unordered/query-error/late-owner |
| 模块/组件 | M03/M13/M15；C17/C18；ReadingUnitProjection仍由Rust选择head/status |
| Owner/独审 | UI r68_translation_placeholder；Root另派独立源码/治理审阅 |
| 精确路径 | apps/desktop/src/features/reading/ReadingPanel.tsx；apps/desktop/src/windows/hud/HudWindow.tsx；apps/desktop/tests/visual/verify-reviewed-reading.mjs；本OpenSpec目录 |
| 复用/契约 | 唯一useReadingUnitProjection、TranslationStatusContent、TranslationStack、readingPartialText；无公共DTO/协议/数据库变更 |
| 输入 | 完整before1973文件 tree 1d566ae00ca9ceba223ac622a9ce50e456ee431bec18ec138bf71ff489f4a859；基线、源、source-snapshot先冻结，禁止作者刷新authority/pin |
| 依赖/门禁 | schema2 rust-storage-complete-v2从初始verification声明，真实注册adapter必须执行；完整verify需Root独审authority采用，不将待采用漂移标绿 |
| 可观察结果 | HUD处理中不再并列通用尚无译文，真实进度/partial可见；空态只述已知事实并可到设置；失败/取消/读取错误和恢复保留 |
| 验证方式 | 原树Red→候选注册Rust/SQLite+React完整100；TS/边界/5负例；真实storage v2；独立候选截图；新包Native/游戏not_run |
| 完成条件 | 精确diff、完整before/after/source identity、真实回执闭包、所有旧断言保存；Root独审采用；产品正式包实测另行 |

不改批准PNG/token/阈值、query hook、Native几何、流式/最终字号排版、不固定整窗高度、不虚构配置原因。无提交/推送。
