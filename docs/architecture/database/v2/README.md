# 会话记录与阅读偏好：schema 2

本批 RW-17～30 的增量。原 `001_profile.sql`、`001_config.sql`、旧 431 字段及其独立检查保持不变。新增 4 表、29 字段，总计 460 字段（含原词典字段）；不以旧适配器只通过 431 字段证明新字段已验收。

独立审阅发现并修正“系统时钟回拨时合法旧会话迁移失败”，之后冻结 `002_session_records.design.sql`：SHA256 `ec4d1470721b9d81e20a3c5e1bb12aa6f59a2b3c32f6310691f332394781ca2e`。本次实现授权不等于数据库全部产品验收或发布批准。

| 表 | 职责 | Owner / 需求 | 关键关系及边界 |
|---|---|---|---|
| capture_operations | 注入前就建立接入身份，串联前置失败、复用会话和正常断开 | M03/M10 / RW17～23 | 多个操作可关联一个 Hook session；NULL 仅能变成固定 session；时间允许回拨但不能负数 |
| hook_events | 不可变完整接入事件账本 | M03/M10 / RW17～23 | 操作必填；会话/来源可选，来源和会话使用复合 FK；完整 u64 用规范十进制串；安全元数据不存台词全文 |
| body_record_positions | 正文修订的持久排序 | M10 / RW18～22 | unit/revision/session 复合 FK；时间不是分页游标；插入触发器与历史确定性回填 |
| reading_preference_settings | 独立分词、显示模式、底板透明度 | M11 / RW24～30 | FK 指向不可变 preference_revision；bool、枚举、REAL[0,1]；不修改文字 alpha |

逐字段定义见 [JSON](field-dictionary.json)、[CSV](field-dictionary.csv)；完整字段、STRICT、NULL、CHECK、索引、复合 FK、触发器的机器规范见 [schema-contract.json](schema-contract.json)。该规范从旧设计 DDL 和已独审的增量设计生成，不从产品 migration 反推预期。`storage_schema_v2.py` 先严格验证旧规范，再核对增量，不能忽略新增字段或只比字段名。

`event_sequence` 与 `record_sequence` 是 SQLite `INTEGER PRIMARY KEY`：写入 NULL 表示请求分配新序号，不代表能持久化空身份。字段字典标为不可空，结构合同仍保留 SQLite 的原始 PRAGMA 结果；测试分别验证自动分配、错误类型、非正序号与序列精度边界。

迁移顺序是只读核验旧版本/原 hash/结构/FK/integrity → 独立 SQLite 备份及核验 → 事务执行 002 与版本账本 → 核验新结构。旧程序不会读写 v2；恢复须使用独立 v1 备份，不能改版本号假降级。既有结束时间可以早于起始时间，排序始终使用持久 sequence。

三数据集查询默认旧接口兼容 oldest_first；当前会话 UI 明确 newest_first：首屏最新 N 条、页内升序、继续分页向前补较早记录。opaque cursor 绑定配置档、会话、数据集、排序、筛选、高水位和生命周期；新事件只增加 newer_count，不移动暂停浏览的范围。导出从原浏览范围派生独立 oldest_first pin，逐页写入、全局升序，不混入之后收到的文本；取消与原子交付互斥。

校验分为：旧 431 字段保证、29 新字段类型/NULL/边界及复合关系、真实生产迁移与恢复、实际 Rust/SQLite 生产者读写、运行包会话与游戏实测。设计结构检查仅覆盖结构层，其他层以本批实际回执为准，尚未执行不得提前记通过。

## 可执行验收与维护

当前 schema 2 使用独立 `storage-acceptance.json` 和 `storage-review.json`，原 schema 1 的登记、431 字段、边界合同和 SQL 内容保留。`rust-storage-complete-v2` 的闭合 Cargo 命令仍执行原 22 项、431 字段、十条迁移/拒绝/恢复路线；同一回执再强制新增 29 条真实生产者映射、29 条 valid/type/NULL 列探针、CHECK/复合关系/不可变触发器、真实 v1→2 备份及恢复、固定范围 JSONL 文件交付。旧路线名称中的 `v1` 是历史 ID，v2 登记显式绑定实际版本 2，不将其解释成不存在的 v1 reopen。真正的 v1→2 由独立增量迁移证据核对。

```sh
# 独立设计结构与可读字段字典；不代表产品生产者已运行
python3 scripts/quality/storage_schema_v2.py
# 新旧守卫的正反例；不冒充产品验收
PYTHONPATH=scripts/quality python3 -m unittest test_storage_schema test_storage_schema_v2 test_storage_acceptance test_storage_acceptance_v2
# 实际生产 Rust 编译/运行与只读独立核对；最终包须再传 --artifact
python3 scripts/quality/storage_acceptance.py --runtime --adapter rust-storage-complete-v2 --output .build/storage-acceptance-v2
```

真实测试维护在 `crates/app-infra/tests/storage_session_v2.rs`、`session_export_delivery.rs`，由独立窄目标 `storage_product_acceptance_v2.rs` 复用原 `storage_product_acceptance.rs` 全量汇总，并额外执行 `scoped_preferences.rs` 的 5 个真实 SQLite 场景；v1 原目标和 argv 不变；原生产者和边界测试保留。QA 执行器维护在 `scripts/quality/storage_acceptance.py`，独立形状与可读字段核对在 `storage_schema_v2.py`，负例分别在对应 `test_*.py`。任何新字段、关系或迁移必须先经独立设计审阅，再增加生产者与负例；不从生产 DDL 自动反推期望。

文件交付使用真实 SQLite 范围和真实文件系统：分页后新增台词不混入、UTF-8/多行 JSONL/hash、读者仅见旧文件或完整新文件、取消移除临时文件并保留原件；macOS 写失败使用已验证独立设备的 64MiB 测试卷触发真实 ENOSPC，释放本轮 filler 后以原计划重试。只销毁本轮测试卷，不填充主机卷。SQLite 在线备份允许页头/事务计数变化，因此独立核对完整原始 schema 与逻辑行，恢复副本仍必须逐字节等于实际备份。

上述 adapter 的 nonce、源码、执行命令、断言、原始日志、数据库快照和最终包身份共同形成一次运行记录。没有实际执行的保持 `not_run`；独立 v2 结构检查、守卫通过或无包的组件实跑，不代表正式包、真实四游戏或完整首版发布通过。

范围偏好使用指定 global/game 的真实 saved/head/effective 与 Rust 派生 inheritedDraft；新保存命令仅比较该 scope 的 head，先重放 operation 幂等，再进行事务 CAS，不受持续取文的无关提交干扰。v2 adapter 对 5 个范围查询、隔离重开、false/zero/empty/legacy 继承、幂等与竞争写入场景强制核对同 run 断言；本轮独立审阅与焦点实跑见 `storage-review.json` 所引用的分离执行记录。它们不代替最终包完整 adapter 或窗口激活/四游戏实测。

窗口解绑另新增三个必须逐场景提供的 v2 语义断言：`V2-UNBIND-SERIALIZATION`、`V2-UNBIND-IDENTITY`、`V2-UNBIND-COMPENSATION`。它们分别验证关联/解绑与重放排序、会话或完整窗口身份陈旧拒绝、真实 SQL 提交失败补偿后重试仍保留采集及历史。聚合器与独立消费者都要求对应场景自己的非零无 skip 断言，不能借另一场景或全局汇总顶替；总检查 ID 从 12814 增为 12817，旧 22 项/431 字段与 29 新字段不变。
