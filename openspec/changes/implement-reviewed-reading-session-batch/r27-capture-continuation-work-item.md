# R27：慢分行正文过早封口修复

日期：2026-10-20；实施授权：用户要求“进行具体问题的定位和修正，同时，对照原有的 luna-translator 的客户端聚合设计”。本项属于既有首版 FP-04/05/09/12、REQ-005/025 与 RW-20/23/29 的正文连续性修复。R26/候选06及SR06b全部失败原件保留，不继承为新包通过。

## 依据与追溯

- 实机：SR06b 同06包5屏，2完整、3长屏只剩尾行；同源后行gap2584/2625/2602ms。raw/source_refs完整；1500ms idle先finalize，下一行NewUnit；Main/HUD取latest导致尾行覆盖。证据位于`.build/next-batch-20261020/game-regression/sr06b/`。
- 原版对照：`.build/next-batch-20261020/aggregation-fix/original-client-comparison.{md,json}`，原客户端可配置Host刷新、近邻多源collector、UUID隔离旧译文；没有可直接复制的跨2.6秒整页合并。保持独立Core、ABI1/JSONL2与固定资源，不重接完整Python客户端。
- 设计：现行整稿`#hud-window`/`#hud-source`/`#main-content`及当前会话正文历史。原文先更新、结果固定版本、全量raw和历史不丢；不改批准16源37图、token、视觉条件或零差异阈值。
- 模块：M02/M03/M05/M10/M13/M15；组件C14/C16/C17/C18/C34/C36。Owner：capture_continuation_impl；root负责规范集成/身份/封包/实机；独立code-reviewer和architecture-reviewer复核。UI不拼历史，Rust policy是唯一聚合权威。
- 前置审阅：`.build/next-batch-20261020/aggregation-fix/architecture-proposal.md`为Medium影响、有条件采纳；不是实机通过。

## 版本化修正规则

1. 具名V1与V2 pure policy共用真实实现；旧1600ms NewUnit等V1断言保持原义。正式仓储入口统一显式V2，禁止测试和产品采用隐蔽不同入口。
2. 新processing revision为`hook-bounded-continuation-v2`；snapshot明确具名策略、未完成文本idle不封口、完成判定版本及原硬限。旧unit/revision/snapshot不重写；遇旧策略open时先封口或按其存储版本执行，不在同unit混版本。
3. V2未完成片段1500ms静默不结束，原文仍立即可见；unit创建起6000ms仍为硬上限，UTF8<=65536B、revisions/refs<=128及事件幂等/合法重复/前缀增长/回退边界保留。
4. 仅带原有尾部句末标点、且支持的日中引号/括号正确配对时完成；未闭合引号内的句号/省略号不封口。不单凭任意闭引号或游戏名/词面判断完成，ASCII apostrophe不当作无条件引号。具体配对集与ASCII双引号规则在实现测试中显式列出。
5. ingest到达fold、立即完成seal、无输入timer三处统一政策；选流/暂停/断开/shutdown/异常与容量仍硬封口，同writer/CAS/raw-first顺序不变。不重新打开已finalized正文。
6. 保留finalized+latest自动任务授权、版本精确的词法/译文/精读，原文更新立即发布；不新增stable-ready状态、DDL/DTO/资源/依赖或UI配置。不对open启动模型请求。

## 有限保证与代价

目标是修复本次慢分行在现有硬上限内的过早误切；不能声称可靠识别任意游戏整页。无标点单句可能等待到6000ms才自动翻译，原文不等待。无页信号的同source快速无标点换页仍有误合风险，超过6秒的慢输出仍可被硬拆；保留为明确限制，不用调大阈值或捏造reset信号掩盖。原版刷新延迟本身不提供页保证。

## 允许修改范围

- 产品：`crates/app-core/src/domain/capture.rs`；`crates/app-infra/src/storage/queries/capture.rs`。
- 测试：`crates/app-infra/tests/storage_product.rs`、`crates/app-infra/tests/application_runtime.rs`、必要的新版本精确字段预期`crates/app-infra/tests/storage_acceptance_support/fields.rs`。领域测试放现有capture.rs。
- 规范：本change的proposal/design/specs/tasks/verification及本卡；`docs/architecture/reading-window-session-technical-design.md`、implementation-status机器/生成表；必要受保护源码身份登记、R26原基线精确归档及独审后的R27清单。禁止刷新旧债务hash、自动放宽范围或接受未审漂移。
- 运行产物、对照与独审只写独立`.build/next-batch-20261020/aggregation-fix/`及新候选07目录；旧06包/报告/失败不覆盖。不提交或推送。

## 测试语义迁移与验证

- 保持V1纯规则的旧断言；新增V2相反边界及<6000的多间隔，包括1600、2~3秒、边界值，不能只测SR实际三个数。
- `capture_growing_snapshot_replaces_prefix_without_losing_old_revision`、`capture_idle_timer_finalizes_growth_without_waiting_for_another_callback`与`translation_off_cancels_waiting_and_running_hook_tasks_without_new_gateway_calls`中旧无标点idle前提显式迁到hard-age6000；前两者明确改名，原数据/原业务断言保留。迁移不是原idle仍通过。若有其他受影响前提先列出原因，不删断言。
- 新真实SQLite：1500后timer不seal，2~3秒后同unit rev2精确全文、旧rev1不变、有序refs重构；6秒硬界只提交一次；配对/不配对/引述/省略号/Unicode/重复/回退/旧版本open及生命周期。
- 新runtime必须冻结逻辑clock并直接等待有界真实watch/调用；不得使用会暗增clock的Harness.call掩盖1500未提交。原文先到、open零网络，完整rev2只发一次、旧结果不覆盖。
- 执行焦点红/绿、既有相关集成、Clippy；独立审阅实际diff后更新必要身份；新07包严格签名/源seal/EXE/ZIP固定，完整`npm run verify -- --change implement-reviewed-reading-session-batch`含五类负例。
- 07实机从SR重新建立SID，至少5屏含慢分行，逐屏Game/Main/HUD全文、raw/revision/译文核对；随后同包MO/Never7/Sakura，排除Ever17。未执行not_run，失败保留，不把受控测试或旧06b作为07产品通过。

状态：planned；尚未实施，不预填通过。

## 执行中发现的既有Lint收尾

全量Clippy实际拒绝`crates/app-infra/tests/scoped_preferences.rs:304`的`assert_eq!(bool, false)`，原失败保留。Root负责该额外一行的等价断言样式修正`assert!(!bool)`，不改变输入、期望、断言数量或业务行为；此文件纳入本项精确允许范围与独审/源身份。数据库/DTO/技术框架不变。完整cargo曾在非串行的资源验收出现profile_in_use，单项原断言复跑通过；并发原因尚不作为已修复，最终必须按登记适配器约束实际复核，不覆盖原失败。
