## ADDED Requirements

### Requirement: TCS-001 latest-request-replaces-only-the-same-stable-engine-current-status
同一固定阅读版本及稳定scope/binding/engine的真实新admission SHALL 以版本化因果边替代当时已存在heads的当前状态，旧失败task/attempt事实仍保留；配置或语义hash变化不阻止该显式恢复。

#### Scenario: latest-request-replaces-only-the-same-stable-engine-current-status
- **WHEN** 先制造connection_failed，再按当前已应用配置显式重译成功
- **THEN** 只显示新请求当前状态；旧task/attempt仍failed且有原错误；operation重放仍返回原receipt

### Requirement: TCS-002 other-engines-and-reading-identities-remain-independent
不同engine/binding/scope或unit/revision/purpose SHALL 独立，不得按显示名或已有任意译文合并状态。

#### Scenario: other-engines-and-reading-identities-remain-independent
- **WHEN** 两个同名provider的独立引擎一个完成另一个失败，并推进其他句/版本
- **THEN** 失败引擎及其他对象状态不被清除，同lane恢复只影响自身

### Requirement: TCS-003 full-database-causal-heads-determine-current-members
当前归属 SHALL 从完整request_tasks的typed/versioned因果graph heads及该固定unit.game_id的当前applied有效engine成员决定，并经独立按对象projection供给，不从旧task100窗取状态，不得由rowid/clock/hash序猜测或被100条展示裁剪改变。legacy无边并列保留，新显式admission合并既有heads；旧binary后追加无边head必须保留。

#### Scenario: full-database-causal-heads-determine-current-members
- **WHEN** 同lane真实admission建立因果边，制造超过100个终态让旧失败仍可见而新head被裁出，重开/逻辑复制临时数据库
- **THEN** 替代关系保持，真实任务因果不因裁剪或时间排序复活旧失败；非法metadata/跨lane引用/环必须拒绝

### Requirement: TCS-004 new-progress-cancellation-and-old-callbacks-remain-truthful
新请求的queued/running/failed/cancelled状态 SHALL 真实表达，旧callback不得夺回当前状态；自动retry保留同task的attempt事实。

#### Scenario: new-progress-cancellation-and-old-callbacks-remain-truthful
- **WHEN** 旧失败后新请求执行、失败/取消并释放旧回调，再执行同task自动retry
- **THEN** 新状态准确、旧回调按既有授权拒绝、旧attempt历史保留

### Requirement: TCS-005 shared-reading-consumes-rust-current-status-and-keeps-history
共享C17 SHALL 消费按固定阅读对象查询的Rust独立状态projection，保留原译文、人工修正和历史，不自行按providerLabel推导恢复。

#### Scenario: shared-reading-consumes-rust-current-status-and-keeps-history
- **WHEN** 生产组件收到旧失败与新成功/运行投影；另一路引擎仍失败
- **THEN** HUD/会话/历史当前状态与Rust一致，旧译文/修正和原有效断言仍保留


### Requirement: TCS-006 invalid-history-lane-isolated-with-typed-unavailable
坏lane SHALL 只在按对象projection中成为typed unavailable，不使AppSnapshot/其他lane失败；坏组新admission仍全事务拒绝。

#### Scenario: invalid-history-lane-isolated-with-typed-unavailable
- **WHEN** 旧历史元数据出现非法版本/引用/环，另一个unit或engine健康
- **THEN** snapshot正常、坏lane错误可见、健康状态与独立admission正常；坏多engine组无部分新增事实

### Requirement: TCS-007 on-demand-old-history-object-state-bypasses-bounded-tasks
状态供给 SHALL 按确切unit/revision读取完整DB并有界按engine输出，不依赖snapshot50/tasks100；ReadingUnit/frozen/serializedUnit不挂瞬态字段。

#### Scenario: on-demand-old-history-object-state-bypasses-bounded-tasks
- **WHEN** 真实historyPage首50后再翻页选旧句，读取原detail和新projection并实际重译，使真实head failed/cancelled落100窗外
- **THEN** 生产C17看到该旧句真实状态，其他对象不污染；旧任务与receipt保留

### Requirement: TCS-008 real-failure-revision-and-attempt-scoped-partial-proof
真实任务失败 SHALL 与commit bump同事务，noop不bump；Runtime部分输出按真实attempt隔离并与DB identity核验。

#### Scenario: real-failure-revision-and-attempt-scoped-partial-proof
- **WHEN** running失败、自动retry同task新attempt后再调用旧delta，新RPC先于新source snapshot到达
- **THEN** 状态query能刷新失败，旧buffer不显示/不覆盖新buffer，旧DB采样不能删新attempt buffer

### Requirement: TCS-009 active-owner-read-coalescing-and-late-reply-isolation
共享view query SHALL 以固定对象owner单flight，request-start floor/monotonic applied和newestDirty合并，inactive停查询且新owner不等旧未settle。

#### Scenario: active-owner-read-coalescing-and-late-reply-isolation
- **WHEN** 快速换旧句/切档、旧resolve/reject/finally迟到，连续raw commits与token流
- **THEN** 不串对象、不紧循环或永久loading，状态有as-of标识并最终收敛；原文/编辑/Hook/preview保留


### Requirement: TCS-010 Registered history sidecar identity
The registered component adapter MUST retain its original four scenario payloads, all one hundred RBC IDs, fixed producer and browser commands, PNG/CSP/source checks. It MUST register only the fixed fresh history/status sidecar with exact metadata, nonce, byte hash and all eight current source identities, and independently re-inspect it after browser consumption. Guard fixtures MUST NOT become product evidence.

#### Scenario: registered-history-sidecar-is-exactly-bound-without-weakening-original-obligations
- **GIVEN** the fixed Rust/SQLite producer generated the original four snapshots and one declared history/status sidecar in the same fresh run
- **WHEN** the adapter binds and later re-inspects the files and browser receipt
- **THEN** every canonical path/hash/source/nonce identity matches and all original checks actually execute
- **AND** missing, malformed, duplicate-key, unsafe, stale, unregistered, tampered or source-mismatched data is rejected without manufacturing product/native/game/pixel success.


### Requirement: TCS-011 Explicit positive reading revision
The new object-scoped ReadingUnitProjection query MUST require a positive explicit text revision. Revision zero MUST return typed invalid_reading_revision at stage reading rather than a resolved unit paired with an unresolved lane. The original raw readingUnit zero/latest contract MUST remain unchanged. Rejected reads MUST NOT mutate reading facts, tasks, attempts, commit revision or frozen learning objects.

#### Scenario: object-projection-rejects-zero-revision-without-changing-legacy-reading-or-facts
- **GIVEN** an actual reading revision has a failed current translation task and a frozen learning object
- **WHEN** the new repository or Runtime projection query receives revision zero
- **THEN** it rejects with the typed error and returns no success projection
- **AND** explicit positive projection still returns the actual failed task while the original raw zero/latest read and every fixed fact remain unchanged.
