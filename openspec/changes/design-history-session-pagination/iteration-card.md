# 本轮迭代卡：历史页会话与分页设计

| 字段 | 本轮约束 |
| --- | --- |
| iteration_id / baseline | design-history-session-pagination；父FP1-20261031-R77，目标R78 |
| 用户授权 | “补充一下，完成后我看一下新设计稿”；仅补历史页会话筛选与分页设计候选，未批准新稿或要求产品开发 |
| 冻结来源 | R77 raw8c461c9fd548ca204956349c9c1a62402003db7186840e03205ba1824358ef43；2160源树6616520bd5ace71ce46183af5d963f7bf7f0a44a97f2d7d0ca45cb9f087607e1；1287pins |
| 需求与验收 | HDP-001范围与会话；HDP-002独立双分页/加载/失败/空态/末尾；HDP-003选择/草稿/迟到隔离；HDP-004真实HTML执行及批准边界；HDP-005精确R78采用；产品来源FP-09、FP-07 |
| 页面与状态 | candidates/history-session-paging-20261031/index.html；manifest review_states：default、initial、records-loading、records-error、records-end、sessions-loading、sessions-error、sessions-empty、empty、long；筛选/Select、记录详情、修正、固定词卡、长文/窄窗/200%/浅色/键盘 |
| 视觉依据 | 沿用已批准guided只读57源/material-tokens；新候选pending_review，无新增批准PNG；候选截图独立生成，禁止覆盖旧图或阈值 |
| 模块/组件 | M13设计语义、M15合同/执行；C23、C24、C04、C08、C09、C19；M10/Rust权威、窄Tauri/Core/worker边界保留 |
| Owner / 审阅 | design Owner写候选4；Root写docs6、接verify与M15重绑；tools Owner写工具3（Root接第4）；governance Owner写change7和helper。设计4/docs6/tools4/governance8均由非作者读最终字节；独立packet审阅；Root采用 |
| 允许路径 | ownership.json预先闭合23source与verification逐项列举；额外仅exact-delta/baseline-review/R77archive/R78baseline四authority，不从diff推授权 |
| 依赖 | 冻结Main/parent raw、57源和旧批准身份、候选manifest、独立设计审阅、当前全页与产品阻断、实际runner与四组最终审阅 |
| 可观察结果 | 选择游戏/会话；分别加载更早；等待/失败保留内容；空态/末尾清楚；范围切换重置对应窗口；旧请求失效；旧句/词卡固定；草稿保护及失败重试；当前句显式定位 |
| 验证 | 保留R77全部有效check并新增history-design-contract/history-design-tests/history-design-execution；npm run verify -- --change design-history-session-pagination；实际TS/Rust编译、PRD/设计/模块/目录/未批准框架五负例；Playwright真实HTML操作与候选截图 |
| 完成条件 | 四组独审绑定精确源，packet独审后Root可恢复采用；全部真实检查回执另存work；没有执行不预填pass，旧计划/CSV/phase/active/approvals/产品/PNG不变；用户能打开待评审新稿 |

计划与执行分离。任务勾选不是执行结果；候选设计未获本轮批准，独立参照与真实产品/原生/游戏/服务/发行保持各自not_run/blocked。没有Git提交或推送授权，不执行提交或推送。
