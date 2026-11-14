# 收轻历史页“加载更早会话”的视觉层级

用户反馈“加载更早会话那个按钮重了”。本轮只将待审 `history-session-paging-20261031` 中该次要操作从常驻底色/边框的按钮收轻为文字操作，使其与会话筛选和记录分页的主次关系更清楚。键盘焦点、hover、disabled、加载中与错误重试语义保留。

本轮严格限制为该候选 `history.css` 与 `manifest.json` 两源、当前OpenSpec七件，以及精确R79采用四authority。manifest只更新 `owned_sources.history.css` 的实际SHA；候选仍 `pending_review`。不改变index.html/history.js、PRD产品行为、任何测试用例/预期/断言/阈值、旧change、旧计划/CSV、verify.py、实现catalog或正式客户端。

当前已批准目标仍为guided，原57源与独立批准记录、所有旧PNG/token/源版/零差异阈值、两项历史设计缺口、phase/active及全部approvals保持原样。样式交付、浏览器设计执行和工程通过不构成新候选批准、独立参照或产品/原生/游戏/服务/发布通过。

复用既有history-design三项实际合同/负例/Playwright执行及完整verify；截图输出新的工作目录，与本轮修正前设计比较，不覆盖批准图、不降低标准。Root在设计2源及governance7+helper8非作者独审、独立packet审阅后采用精确R78→R79差异。不提交、不推送。
