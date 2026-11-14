# 正文选择集合与保存说明：schema5技术设计

本目录是绑定旅程P1的目标字段、关系与边界候选，**生产尚未迁移、未验收**。前置为现行schema4及完整批准的正文候选/记住选择/开始阅读交互；它不批准新视觉稿、不重写001～004迁移或旧批准原件。Root需在计划独审、门禁确实拒绝旧产品之后，才开始实施对应迁移及Repository。

## 选择模型与职责

当前`source_selections`每版只有一个source，正文revision与source_ref严格引用该真实成员；不能把source_id改成任意代表性来源来伪装多选。新增两表：`source_selection_sets`代表一次明确确认，`source_selection_members`把集合关联到原per-source选择版本。原FK、`source_ref_selected_source`和历史行保持原义，不把不同来源拼进同一个阅读单元。

M03/AP负责确认语义与跨会话匹配，M10单writer负责事务与持久化，M13仅持勾选草稿，M12在提交后独立执行窗口效果。现行Core ABI、collector/filter/Fence不变。

[独立DDL](005_source_selection_sets.design.sql)可用于内存结构和边界核对；[字段边界](field-boundaries.json)区分SQL约束与Rust权威。[完整结构合同](schema-contract.json)仅由冻结独立v4设计与本DDL构造，包含83表/748个profile字段及未变的32个dictionary字段；全部旧table/view形状保留。新结构与生产迁移必须逐字段独立比较，不能从生产DDL学习期望或把设计SQL自检当生产通过。

## 字段与关系

| 表.字段 | SQLite类型、NULL及边界 | 归属与作用 |
|---|---|---|
| sets.session_id | TEXT NOT NULL，非空、无NUL | FK sessions；当前实际Hook会话 |
| sets.selection_set_revision | INTEGER NOT NULL，>0，checked递增 | 与session组成PK；新IPC用规范十进制字符串避免精度丢失 |
| sets.operation_id | TEXT UNIQUE；legacy为NULL，explicit非空无NUL | 延迟FK既有operation_receipts；与同一选择事务提交，复用原请求hash/结果身份 |
| sets.origin_kind | TEXT NOT NULL，explicit_set/legacy_singleton | 区分真实用户确认和无损迁移，不给历史行伪造operation |
| sets.persistence_kind | TEXT NOT NULL，temporary/saved_snapshot | 临时选择不覆盖此前保存说明 |
| sets.match_snapshot_json | TEXT NOT NULL，合法JSON object、无NUL | carrier；内部schema/version、字段类型、来源描述和匹配能力由typed Rust核对 |
| sets.selected_at_ms | INTEGER NOT NULL，>=0 | 保留实际确认时间；legacy保原时间 |
| sets.member_count | INTEGER NOT NULL，1～64 | 明确有限提交资源上限；新集合不可空或截断 |
| sets.state | TEXT NOT NULL，building/sealed | 仅完整成员数、连续ordinal可seal；sealed不可修改或删除 |
| members.session_id/set_revision | TEXT/INTEGER NOT NULL，FK完整集合 | 不能跨会话或孤立写成员 |
| members.source_id | TEXT NOT NULL，非空无NUL | FK capture_sources(source_id,session_id)；完整来源身份 |
| members.selection_revision | INTEGER NOT NULL，>0，FK原source_selections | 继续保留正文真实来源血缘；每行最多归属一个集合 |
| members.member_ordinal | INTEGER NOT NULL，0～63 | 集合内唯一、从0连续至count-1 |

集合内source、ordinal都唯一；member插入须匹配building header、真实source、原selection的同session/source/persistence。只读`current_source_selection_members`取每session最新sealed集合，building不可被正文/任务消费。member_count=1～64与member_ordinal上限为统一有界策略；UI、domain、storage、错误呈现须使用同一个冻结定义，不为测试另设例外。

SQL是持久carrier，不证明完整session正在有效运行、review未过期或matcher可移植。Rust另核完整当前session/目标代次、expected set head、非空无重复集合、当前来源归属、operation请求摘要一致、版本加法不溢出、游戏归属与typed JSON。相同operation重放返回原set；相同ID不同规范请求拒绝。SQL合法但业务不合法的记录不能变成可用选择。

## 原子提交与自动任务资格

一次`beginReading`在单writer事务内：先核operation幂等与session/head CAS；封口旧open单元；为全部所选成员分配新的原selection_revision；写building set与members；核完整成员数并seal；写既有operation_receipts后提交。任一步失败全部回滚。A→B→A或仍选A的新确认仍分配全新成员版本，防止历史旧句重新获得自动任务资格。旧`SelectSource`委托同一单成员集合路径，不保留第二套单流head权威。

以下三处必须使用相同current-members语义：正文事件消费、`database.rs`工作任务准备、旧自动任务准备/重试。writer在同一作业里核finalized、最新text revision、有效session及当前集合包含该member revision。不能继续要求正文selection_revision等于session最大单流revision，否则多选只最后一项翻译。切换集合后旧revision不自动请求；显式历史重译继续按固定旧对象的独立意图处理。

不同成员分别形成阅读单元与source_refs，交替事件保持各自来源；不按Hook名合并，不把UI勾选顺序当游戏文本页边界。原文、译文、词法和processing快照不被集合切换改写。

## 保存与跨会话匹配

saved集合保留版本化匹配说明，按当前profile真实game归属及兼容目标身份读取最近一次明确保存；后续temporary选择不清除它。无game归属时禁止remember并给出明确错误，不能静默降为temporary。旧saved行通过legacy_singleton保留原snapshot字节；它不获得新的自动匹配能力。

当前来源sourceId带session，address/context可能受ASLR影响，identity_extensions也没有已证明的可移植模块相对身份。因此本阶段保存描述符并返回`requires_review`：展示上次来源与不能安全匹配的原因，不按名称、PID、旧sourceId或绝对地址自动预选/提交。未知matcher版本原字节保留并需重新核对。未来可移植matcher需独立证明和完整唯一匹配；任何预填仍须用户显式开始阅读。

现行retention只清raw payload、历史单元与缓存，不删session/source/selection；新增set/member同样不能被payload清理移除。若以后新增整段会话清理，必须先迁出独立偏好快照或明确保护saved集合，不能级联清掉记住选择。规则不是截图、台词历史或自动注入授权。

## 迁移、兼容与故障

追加005；001～004的源、ledger与hash保持。每条旧source_selections行构造同revision的legacy_singleton header和一个原成员，保留source、persistence、snapshot及时间。迁后每session最新set对应原最新单流；不重写reading_unit_revisions、source_refs、原文译文等历史。无历史selection的会话不造空集合。历史迁移不得伪造operation_id或可移植匹配证明。

迁移须同事务完成DDL、backfill、完整计数/外键/来源复核和ledger。坏库、未知schema、更高版本、非法历史引用、类型或版本越界先拒绝并保留原库；不在原库上尝试“修到能过”。新客户端须实测0/1/2/3/4→5、重开5，旧客户端拒绝5且不写库。失败回滚、外部kill、只读/busy/ENOSPC及备份还原分别留证。当前SQL设计的executescript自检不能代替生产writer迁移故障验收。现行测试中固定的current4版本、pre-v4备份名和整库v4比较须按机器计划列出的精确依赖适配，不能只改4为5：实际冻结Store4及原测试继续验证81表/734字段、原升级/备份/故障；当前Store5使用独立v5完整期望另验。原v1～v4设计/比较器、001～004、字段/关系/route/故障断言与原始fixture不减，v4比较器仍精确拒绝5。新生产增量由source_selection_sets测试输出同源同nonce的实际SQLite副本，独立检查器复核shape、ledger、字段、关系与路由。迁移故障Scope保持database.rs私有cfg(test)，父integration producer和实际libtest child是不同executor：必须消费同源locked编译artifact/源与Cargo.lock、实际binary、--list和固定exact执行、PID/nonce/output/时间、fault arming与native错误、callback时间和DB/备份/ledger精确关系。父工件不替子工件，不能只附自由refs或以relay完成时间冒充故障时刻；readonly/busy即使共享child执行，也各有独立fault record/DB。ENOSPC还需独立限容卷设备/容量/OS28/SQLite13及create/attach/detach原命令。test exit0或自由JSON标记不授存储通过，不增加debug/public故障入口。通用complete-v3执行器不代表完整v4/v5adapter已落地。

### 私有故障时间事实与当前缺口

当前`database.rs::source_selection_migration_fault::Scope::new`的`started_at_ms`是测试控制器创建时刻，不是`SqliteStore::open`入口；`armed_at_ms`也只证明arming。`record_fault`的`backup_completed_at_ms`借用`reached_at_ms`，仅PauseBeforeWrite写入，AfterDdl路径为0，不能证明备份完成。`rollback_observed_at_ms`当前只是record_fault取时，不能证明此前完成了回滚核验；现有字段不足不能在P2被解释成通过。原生产与测试源码本阶段冻结，这些明确为P3待补缺口。

机器合同`binding-journey-plan.json#resume_epoch/observation_contracts/storage_parent_child/production_timeline`固定各route事件适用性、真实生产callback位置与子进程/线程/DB身份、严格event_sequence/monotonic次序及非倒退正wall时间。P3仅在私有cfg(test)边界观察实际API进入、worker进入、备份经校验原子交付及sync完成、BEGIN尝试/成功、DDL、arming/native SQLite失败、API返回与真实回读核验。测试业务时间100不作执行时间，Scope/父relay不能顶替。按route未到达的事务/DDL/return事件必须null＋not_reached/applicability与真实failure stage；适用事件缺失或0拒绝，不能把未知写成not_applicable。外部kill没有返回/native错误时保留null，以实际kill/wait和独立恢复证明，不能伪造API返回。

rollback-after-DDL必须有备份完成→真实BEGIN→005DDL→native19→API错误返回→旧库/ledger/schema/原文回读核验；readonly/busy/ENOSPC必须有备份完成与before-write barrier，随后实际只读权限/锁/限容填满生效与arming再resume，native8/5/13先于错误返回与旧库核验。BEGIN成功/DDL是否发生按真实stage决定；失败前未进入事务不能伪称rollback，改记原库不变的实际核验。ENOSPC另保OS28/隔离设备/容量与create/attach/detach原件。每route有独立DB与fault事件；同名子test含readonly/busy仍两份证据。原父子编译/list/exact、Store4及Store5结构/ledger/备份/恢复断言全部保留。P2消费者与负例先落，真实writer观察必须P3准入后再补，缺口保持not_implemented/not_run。

## 完成与准出

计划和字段独审、门禁正反例、真实Rust/Repository迁移与集合测试、生产组件旅程、最终同源签名包窗口效果和四游戏各自出具原件；缺任何适用项保持incomplete。窗口效果在选择提交后先show HUD成功再hide Main，失败只重试绑定当前session/set/operation的窗口效果，不重新生成集合或remember规则。

要求至少包括：两个成员都能自动翻译；旧集合/旧revision不能自动请求；同来源重新选择获得新版本；重复请求不重复写；并发head变化拒绝；成员跨session/重复/缺失/空/超界拒绝；失败全回滚；临时不覆盖saved；payload清理不丢保存说明；旧版本不被篡改；无匹配证明不自动勾选；窗口失败重试不重复提交。
