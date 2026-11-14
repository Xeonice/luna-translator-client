# 迭代卡

| 字段 | 内容 |
|---|---|
| Change / epoch | align-approved-binding-journey / r3显式续写，产品阶段不推进 |
| 用户授权 | “先更新计划，然后更新门禁，最后再开始实现”；R90首版范围及已批准设计继续 |
| 历史前像 | R92 / 4ed54c6a01701e429ccc3e6c1f282d610a185916ec8044b227bd7529ef143432；原source-snapshot不改 |
| 已成立祖先 | 原P1r1、未签P2r1、P1r2/P2r2独审及2026-11-09T18:54:23.901601+00:00首次P3原件；不授新oracle |
| 当前续写前像 | 03检查点54c378f69a06a752a9382156e444c9880c67ca7fe22038f4f35f0238b52c9a4e／2809全源／738产品／47合法差异；独审bb0fa367…仅preservation_for_plan_resumption_only，P1r3已发布待独审，不授门禁或实现续写 |
| 需求/设计 | FP-03/05/09/12、GL/RW、BIND-001～006；完整61源/C1/C35及批准74闭包原字节 |
| 模块/组件 | M02/M03/M10/M12/M13/M15；C13/C14/C15/C35 |
| Owner | Root封存/编排/实机；计划作者覆盖文档；gate Owner窄观察与程序；backend/storage/UI各层；独立非作者checkpoint/P1/P2/source审阅 |
| 允许路径 | 原23 plan / 16 gate及历史62 implementation保持；r3 implementation仅精确新增window_preview.rs成为63项，checkpoint合法差异仍按原62核验；各阶段只能写对应组，不新增框架 |
| 观察修订 | sourceId/pending/candidates_ready正确语义且Node/Python独算；私有fault父子真实执行链；BJ13 before/completed及loss-free真实milestones；BJC05精确同run discharged_by |
| 产品待修 | 明确接入重试、picker受控终态、旧Native bool零副作用与预览/日志隔离、最终包/视觉/存储闭包 |
| 固定边界 | Core ABI、collector/filter/Fence、游戏尺寸、001～004、83表/748字段合同、BJ14整页零RGBA、原153/RW/901不变 |
| 验证顺序 | checkpoint独审→plan-only独审→gate-only fresh32hex真实RED/守卫/原五类负例及独审→新目录fresh admission→实现→最终同包Native/四游戏/真实翻译 |
| 完成条件 | 全绑定旅程和所有适用层/状态有真实原件；阶段许可不授产品，缺项保持incomplete/next_stage_allowed=false |
