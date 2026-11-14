# 迭代卡：收轻会话目录的次要操作

| 字段 | 本轮内容 |
|---|---|
| iteration_id / baseline | design-history-session-action-weight；父FP1-20261031-R78，目标R79 |
| 用户授权 | “加载更早会话那个按钮重了”；只调整待审设计中的样式层级，不实施正式客户端 |
| 冻结来源 | 2180源树2fc3ebbdcc116c0fb224df84038d12ec266d8ada6b6fe1e180aacdd71af90338；R78raw3b51ca08efecd960d57c2d3a38c6fc23d8a71f21782406b39eebe72ce36cac10；1307pins |
| 需求/验收 | HSW-001文字操作层级；HSW-002既有交互/状态与纯样式边界；HSW-003精确R79与未批准边界；沿用FP-09、HP-01～12 |
| 设计页面/状态 | history-session-paging-20261031/index.html?scenario=history；#hx-sessions-more/.hx-directory；default/hover/focus-visible/disabled、sessions-loading/error/end、深浅色/窄窗/长文/200% |
| 视觉依据 | R78待审稿的实际截图作修改前对照，不变成批准参照；沿用只读guided57源/材质，候选截图输出独立工作目录，不覆盖旧PNG或阈值 |
| 模块/组件 | M13候选CSS设计语义；M15复用既有QA执行；C02 Button、C04 Select、C08状态和C23 HistoryList/HistoryDetail；不改Rust/Tauri/Core/worker/Repository |
| Owner / 审阅 | Root负责CSS/manifest2源与采用；governance Owner负责change7+helper；非作者独审design2及governance8，另一个非作者独审完整packet |
| 修改范围 | ownership预先闭合9source，verification逐项列举；另外仅4authority；index.html/history.js、verify/M15/catalog/CSV、PRD、旧change/plan/测试预期/批准图/正式产品禁止修改 |
| 依赖 | R78完整冻结与1307pins、最终CSS及仅SHA更新的manifest、既有history合同/实际执行器、两份非作者最终文件独审和packet审阅 |
| 可观察结果 | 会话加载为轻量次要文字操作，默认无常驻底色/可见边框；hover/focus/禁用及加载/错误仍明确；数据/分页/编辑/选择/滚动行为不改变 |
| 验证 | 既有history-design三检查真实执行，HP12计划/断言保留；Playwright修改前后截图与交互；完整verify含TS/Rust实际编译、五类负例；实际结果另存work |
| 完成条件 | 9source+4authority精确闭合；独审与packet绑定最终字节；Root可恢复采用；待审/原批准/产品blocked与旧记录均保持，未执行不预填通过 |

仅样式层级修正，全部产品执行与验收标志false；没有本轮新批准、提交或推送授权。
