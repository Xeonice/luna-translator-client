# 存储运行验收：本波进度与剩余条件

本波只补实际运行设施与 PRE-14 候选合同。`docs/development/storage-acceptance.json` 仍为待评审、adapter 注册表仍为空，完整存储与发布验收保持 blocked；未改计划 CSV 的 `not_run`。

## 已实际执行

- `cargo test -p app-infra --test storage_acceptance_routes --test storage_schema_contract --locked -- --nocapture --test-threads=1`：2 项新运行测试、4 项已有结构测试通过，0 failed/ignored。日志 `.build/storage-acceptance-real-routes.log`。
- `cargo clippy -p app-infra --test storage_acceptance_routes --test storage_schema_contract --locked -- -D warnings`：通过。日志 `.build/storage-acceptance-routes-clippy.log`。
- Python 合同与 executor 的负例守卫、封闭快照比较守卫分别保存在 `.build/storage-acceptance-guard-contract-candidate.log`、`.build/storage-schema-sealed-guard.log`。守卫 fixture 不进入生产 adapter。
- 生产 SQLite 实际版本 3.53.2，source ID `2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24`。
- 独立结构检查使用真实生产 `SqliteStore::open` / `create_dictionary` 输出。schema1 新建、同版本重开、生产在线备份后的同版本恢复、未知版本/无版本/损坏文件/迁移内容 hash/结构漂移/关系损坏拒绝均运行；每个拒绝核对实际生产错误码与原件逐字节保持不变。
- Unicode 罕见汉字、emoji、组合假名通过实际 Repository 存取与重开；NUL 原文按既有域规则返回 `invalid_text`，并核对 commit revision 不变。
- 活跃 WAL 的生产 writer 实际提交 100 条；在线备份恢复一个连续已提交前缀，核对原文、operation receipts 与 revision 一致。仅该独立子检查输出 `STORAGE-05` marker。
- 随包辞书的元数据已只读核对：index1，`wiktionary-zh-ja-20261018+jmdict-20261012-v1`；其 `manifest_sha256` 绑定 `dictionary-provenance.json`，不混用外层 language manifest。

## 合同修正边界

候选见 `storage-pre14-candidate.json`，状态 `pending_independent_review`。其中 `proposed_matrix.status=frozen` 只是供评审的目标 payload；`approval_ref=null`，当前不能通过正式冻结校验。root 独立评审后才能决定是否采用，不能自动刷新 authority 或指纹。

- `source_sha256` 校验实际 Rust 文件；`sql_sources` 逐文件校验实际生产 SQL；`migration_sha256` 按生产实现的有序 SQL 加换行内容计算，并与真实 ledger 比较。旧含混 `sha256` 格式拒绝。
- 通过 `source_state` 显式区分 supported、unversioned、unknown_version、corrupt、migration_mismatch、schema_mismatch 与 relationship_corrupt。无版本/损坏的 `from_version` 必须 null，未知形状不能冒称已批准 schema；拒绝要求实际错误码和前后原件 hash 相等。
- 只有 schema1/index1，没有历史 v2 升级或 PoC JSON 迁移。测试中 version99 是专门构造的未知输入拒绝样本，不是受支持数据库。
- standalone candidate inspector 先拒绝非空/符号链接 WAL/SHM，再以 immutable 只读查看；不能忽略活跃 WAL。生产 backup 可能残留只读探测的 SHM，本波仅归档测试自己的已完成备份或制作逐字节副本，不处理用户库 sidecar。

## 22 项完整运行覆盖仍未齐备

| 子检查 | 本波实际范围 | 完整验收剩余 |
|---|---|---|
| STORAGE-01 | 真实 Rust target 编译 | 431 字段的生产 Rust 输入/读取映射及无损转换证明；测试自造通用 ValueRef/codec 不可代替 |
| STORAGE-02 | 实际 profile 建库、dictionary constructor 结构 | 随包完整辞书资源身份/只读初始化；constructor 无 meta/词条，不输出完整 fresh_dictionary 路线 |
| STORAGE-03 | schema1 重开和同版本恢复、未知输入拒绝 | PRE-14 独立冻结及与完整执行器接入；不虚构历史升级 |
| STORAGE-04 | 原件不变的未知/损坏/不兼容拒绝、同版本备份恢复 | 受支持正式包与备份的配对身份回退验收 |
| STORAGE-05 | 实际 WAL 写入与一致备份/恢复断言完成 | 纳入完整 adapter 的本次独立 receipt |
| STORAGE-06 | 本波未重跑完整竞争设施 | 实际跨进程 busy、128 writer 背压及恢复 marker/receipt |
| STORAGE-07 | 本波未执行 | 只读失败、草稿/已提交数据保持及恢复 |
| STORAGE-08 | blocked：未创建真实限容卷 | 隔离卷实际 SQLITE_FULL、释放容量后的重试与原子性 |
| STORAGE-09 | 本波未执行 | 子进程各提交 barrier 崩溃与多表/revision 原子性 |
| STORAGE-10 | blocked：未执行真实 Keychain 崩溃矩阵 | 测试专用 namespace，pending/vault/DB 激活各边界，重启清理且无明文 |
| STORAGE-11 | 本波未执行 | 撤销/删除失败仍无授权，实际 socket 与重启证明 |
| STORAGE-12 | 本波未执行 | 取消/停用/切档/完成所有交错与单一终态 |
| STORAGE-13 | 本波未执行 | 缓存语义变形矩阵与人工修正独立保存 |
| STORAGE-14 | 本波未执行 | 并发分页、清理、固定来源保护与容量 |
| STORAGE-15 | 本波未执行 | 真实双进程 lease、跨配置档隔离与实例代次 |
| STORAGE-16 | 只读核对真实资源 meta | 完整 manifest/许可/写入拒绝及跨库引用版本不符处理 |
| STORAGE-17 | Python 独立设计边界守卫仍执行 | 所有 field-boundaries probe 由生产 SQLite/Rust 运行并逐条实际断言 |
| STORAGE-18 | 实际 FK 损坏拒绝、结构漂移负例 | 复合 FK、UNIQUE/NULL、RESTRICT/CASCADE/SET NULL 完整行为 |
| STORAGE-19 | 本波未执行 | 全封存表 INSERT/UPDATE/DELETE、head CAS、三态及显式 0 |
| STORAGE-20 | 本波未执行 | 全 operation/revision 原子幂等与重启 |
| STORAGE-21 | 合法 Unicode 实际无损往返、非法 NUL 拒绝 | 全 u64/FILETIME/source 复合身份与 span |
| STORAGE-22 | 本波未执行 | 配置和凭据池引用保护、墓碑、回收后禁止重用 |

原有组件测试不因本波新增文档被自动提升为这 22 项全通过。完整 adapter 仍须真实调用每项断言，产生非伪造 marker、封闭候选库与完整 receipt；缺项继续拒绝。
