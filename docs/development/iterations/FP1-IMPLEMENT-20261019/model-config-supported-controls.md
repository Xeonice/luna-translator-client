# 模型配置已支持能力补充实施卡

- iteration_id：FP1-IMPLEMENT-20261019 / UI-MC-SUPPORTED；baseline_id：FP1-20261019-R7。
- 授权：用户“开始动工，按照事先规定的架构和设计进行开发，完成后进行对应的测试”；本批由主责分配首版已有 Rust 契约支持的模型配置编辑，不包含提交或推送。
- Owner：product_ui_impl；应用/运行语义与独立审阅：Root。协作者共享工作区，不覆盖其他模块改动。
- 修改路径：`apps/desktop/src/features/model-config/**`、`apps/desktop/tests/**`、本实施卡。复用生成 DTO、ApplicationStore、C26..C31 与中性材质 tokens；不改公共 DTO、Rust、Tauri、设计源、已批准图和门禁阈值。
- 门禁缺陷修复范围（Root 发现真实失败后追加）：各 feature 的公开 `index.ts`、`bridge/projections/index.ts` 与对应 UI 导入、`app/bootstrap.tsx`、`apps/desktop/index.html`；移除非法根入口 `src/main.tsx`。不放宽任何 guard，按现有公开边界修复；新增实际生产 bundle 启动检查，确保入口搬迁仍真实挂载。
- 来源：本轮 change 的 `source-snapshot.json`；工作区未提交，产物以本次真实构建与截图回执的 sourceSha256 标识。

| 工作项 | 产品/验收 | 设计入口与状态 | 模块/组件 | 可观察结果与验证 |
| --- | --- | --- | --- | --- |
| UI-MC-21 引擎编辑 | FP-06 / AI-CFG-03、12 / MC-07、12 | first-product-v1 的 settings-window → 翻译与模型 → 用途；草稿、禁用、首末排序、保存失败 | M13/M04/M05；C27/C31 | 逐引擎开关和上下排序保留稳定 ID/参数；只保存草稿，不触发应用或请求。Playwright 操作真实组件并核对命令与原对象身份。 |
| UI-MC-22 调度边界 | FP-06 / AI-CFG-12 / MC-07、12 | 同用途页；默认、修改、数值超界 | M13/M04/M05；C27 | 并发 1..8、队列 1..128、重试 0..3，匹配运行读取；0 重试明确不自动重试。保存真实 DTO 字段，超界反馈，Rust 保持最终权威。 |
| UI-MC-23 独立代理 | FP-06 / AI-CFG-01 / MC-01、03 | settings-window → 服务连接；inherit、override、disabled、未填与错误 | M13/M04/M06；C26 | override 编辑 networkMetadata.proxyUrl，保留其他元数据；不填写明文代理凭据，不声称配置保存代表连通。Playwright 保存/切换/重新打开核对字段。 |
| UI-MC-24 术语草稿 | FP-06/08/09 / AI-CFG-09 / MC-09、12 | settings-window → 翻译与模型 → 术语表；空、新建、全局/游戏、开关、缺词面/译法、保存失败 | M13/M04；C27/C28/C31 | 建集、编辑范围/名称/启用，逐项原词/译法/启用与草稿移除，稳定身份。说明只发送启用且匹配当前原文的词项，语言字段不冒称运行过滤。Playwright 核对全局/游戏范围键、取消和未应用。 |

视觉依据：只读 `first-product-v1/previews/31-dark-settings.png` 及当前模型配置整稿，保持既有材质、字号、线性图标；候选独立 `.build/product-ui-candidates/<run>/`。整体布局仍待评审，像素或尺寸差异继续 failed，不刷新基准。

范围边界：不添加预算、供应商回退、删除影响闭包、导入导出。模型 DTO 不持有模板引用，模板属于用途/引擎；本批不把简单复制冒充 PRD 完整复制语义。

前置核对：现有 generated Configuration/Engine/PurposeBinding/Connection/TermSet/Term DTO 与 storage 保存均存在；Runtime 已读取 concurrency/queueCapacity，provider 已读取 override/proxyUrl。Rust 调度重试上限校验由 Root 补齐。UI 不解析秘密、HTTP 或数据库。

检查计划：生产 `tsc`/Vite build、测试 strict `tsc`、既有 UI-01..20 及新增 UI-21..24 Playwright 行为，追加设置候选截图与原图比较。正式包/代理实际网络/调度任务/词项进入真实请求由 Root 的原生与模型回归验证；受控 ApplicationPort 测试不替代这些结果。

追加 UI-25：用本轮 Vite 生产构建的原始 JS/CSS，经路径调整的 HTML 副本加载；不注入测试端口，断言真实挂载并显示缺少 native 核心的明确错误，防止只测受控入口遗漏生产入口不执行。

完成条件：生产构建与新增行为有本轮独立真实回执；视觉失败、原生和后端未运行项如实分列。此卡只定义计划，未预填 passed。
