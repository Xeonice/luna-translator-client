# 同源多行与逐字原文聚合

## 身份、依据与边界

- work_item: IMP-CAPTURE-AGGREGATION-20261019；baseline: FP1-20261019-R12，不刷新权威指纹。
- 授权：用户要求继续收尾；主任务将真实 SR 分行缺陷纳入既有 REQ-005/025、FP-04/05/09/12 实施。
- 证据：`.build/client-regression/games-84a7b7f8-20261019/events/sr-state-05.runtime.json`；wire 358/361 已是两个独立原始回调，1307 ms 间隔，原文未丢字；不以特定文本或间隔硬编码规则。
- 设计：当前整稿 `#hud-window`、`#hud-source`、`#hud-more`、`#main-content`；原文先显示、固定对象及历史可追溯；不改材质、布局、批准图、公开契约或资源。
- Owner: recovery_backend；独立 code-reviewer；root 负责新包、模型和四游戏实测。
- 模块/组件：M02/M03/M05/M13/M15，C14/C16/C17/C18/C34；Rust 业务权威，UI 不拼接原文。

## 允许路径

产品仅：`crates/app-core/src/domain/capture.rs`、`crates/app-core/src/ports/mod.rs`、`crates/app-core/src/application/runtime.rs`；`crates/app-infra/src/storage/database.rs`、`queries/capture.rs`、`queries/mod.rs`、`queries/history.rs`、`queries/retention.rs`（queries 相对该 storage 目录）。

测试仅：`crates/app-infra/tests/application_runtime.rs`、`storage_product.rs`、`history_queries.rs`、`configuration_lifecycle.rs` 及独立 `capture_aggregation.rs`；领域纯规则测试放在现有 capture.rs。两个 Repository wrapper 仅增加真实委托，不复制存储/CAS。

追溯仅本 change 的 proposal、spec、design、tasks、verification 及本卡。公开 TS/Tauri/JSONL/Core ABI、DDL、schema-contract、资源、依赖、基线、批准图均不变。

## 内部协议和处理

既有 open/finalized、不可变 revisions 和多 source_refs 足够。新内部 ingest outcome 明确可见版本与稳定待调度版本；旧 receive 方法仅转发同一实现供现有存储调用复用。原文每次有效变化立即持久化和发布，自动翻译只对 finalized 的确切版本调度。

同 session/full source/selection 内有界聚合：静默 1500 ms、总持续 6000 ms、正文 65536 UTF8 bytes、最多 128 revisions/来源跨度；这些为通用版本化初始政策，需要慢速/快速实机回归，不宣称无页信号的 GDI 边界可完全判断。相同 event ID 幂等；不同事件同文保留为新句，不吞合法重复。当前未结束 open 的严格前缀增长用新快照；其他片段顺序追加；句末及闭引号、期限和容量触发封口。所有接收事件先完整保存，处理版本及策略进入 processing_snapshot_json；既有授权维护仅能清理已封口的负载，raw 身份及 source_refs 保留，open 引用负载受保护。

选流、暂停、断开必须在原命令 CAS 校验后的同 writer 事务封口，不单独先提交制造冲突；异常和重启封口并保留异常状态；退出阻止新自动请求后保存/取消。手动输入不进入 Hook 聚合。未选中候选不结束或混入当前正文。

历史每个 unit 只列最新版本，精确旧 revision 保留给词卡/AI/结果；不得重写原始事件或旧正文。删除维护拒绝清理 open。

## 验证与完成条件

- 领域：逐字增长/分行追加/合法重复/UTF8/边界/容量/超时，零游戏词面规则。
- 真 SQLite：多 refs、不可变旧版、最新历史、选流/暂停/断开同事务与双 CAS、异常/重启/open 清理保护。
- 真 runtime：直接 watch 先见原文、稳定前零自动模型调用、稳定后确切版本一次、迟到结果不覆盖、关闭/退出不补发。
- Rust 相关集成测试、Clippy、完整 verify；真实新包四游戏重新建立身份和记录，旧 84a7 证据不提升为新包通过。
- 当前状态 in_progress；未执行项 not_run；既有视觉和存储发布缺口仍单独报告。不提交推送。

兼容边界：latest-only 仅适用于本次 processing_snapshot_json 中明确 unitProjection=latest 的聚合内部修订；既有正式处理器/手动修订历史继续保持原有多修订分页语义，旧断言不变。

内部自动任务权威另经同 writer 的 prepare_capture_tasks 核验：当前 active session、当前 selection、finalized unit 的 max revision。生命周期后晚到调度不得创建新自动任务；显式历史重译继续使用独立通用端口。正常退出持久 stopped；退出失败恢复 closing 闸门且 consumer 保活，以便继续/重试。

身份登记补充：R12 原文与全部批准范围保留；独立审阅后的 R13 仅更新 storage-acceptance.json 所绑定的实际 Rust 源码身份及对应一条指纹，详见 capture-source-identity-work-item.md 与 baseline-review-r13.json。迁移 DDL/字段关系不变；重启恢复封口 DML 确有新增，未声称整个 open 函数不变。
