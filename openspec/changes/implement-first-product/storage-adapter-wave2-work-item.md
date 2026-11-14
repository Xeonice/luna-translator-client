# 存储运行验收第二波：字段与完整 adapter 收敛

- Owner：`storage_acceptance_impl`。需求 FP-11/FP-12；STORAGE-01～22。实现依据为既有数据库设计、冻结 schema、431 个字段 ID 与 12,289 个边界 probe，以及 `storage-acceptance.json`；不更改它们的要求、预期或确认状态。
- 本波独占路径：`crates/app-infra/tests/storage_*`（不改 `storage_product.rs` 既有用例预期）；必要 `examples/storage_*`；`scripts/quality/storage_acceptance.py`、其守卫及事先告知的新增 adapter 辅助；本变更 `storage-*` 工作项。root 正在改业务契约、session/history 与 capture queries，不得覆盖。
- 首批确切新增测试路径：`crates/app-infra/tests/storage_field_boundaries.rs`、`storage_field_mapping.rs`、`storage_acceptance_runtime.rs`。辅助若使用将提前说明 `scripts/quality/storage_runtime_adapter.py` / `test_storage_runtime_adapter.py`，不以 JSON 临时命令注册。
- 已向 root 明确告知后续独立真实设施路径：`crates/app-infra/tests/storage_credential_crash.rs`。使用唯一测试配置档对应的真实 SystemVault namespace；子进程在新增凭据的四个跨存储边界暂停并被终止，重启调用实际 `recover_credentials`。Keychain 所有操作均委托真实 SystemVault；Hook/模型/语言端口若被意外调用直接失败，不伪造它们的实机成功。测试假秘密只驻留内存/测试进程环境/测试专用 Keychain 项，DB/日志/回执不得含明文；结束清理每个本轮测试项，不枚举或操作用户项目。

## 可独立实施的范围

1. 通过真实 SqliteStore/create_dictionary 得到生产 schema，从实际候选库读取 table/index SQL；Rust bundled SQLite 实跑全部字段 probe。隔离字段投影沿用既有契约的无 trigger/FK 模式，完整关系及事务另测。该层可用 SQLite 动态参数测试 SQL 本身，但绝不冒充生产 Rust DTO encode/decode 或输出 RUST-MAP marker。
2. 记录 Python oracle 中 226 个 i64 外整数的 native Rust 拒绝：实际 `u64` ToSql 或 checked i128→i64 转换；回执保留真实 Rust phase/error，不伪造 Python OverflowError。正负无穷、NaN、NUL、原始字节及默认值均真实执行并核对原始存取。
3. 梳理每字段真实写入输入、生产读取/用途、领域类型及具体 codec 路径；仅在实际业务 encode/decode 往返与可空/范围测试覆盖后记 RUST-MAP。没有真实 mapping 的字段明确 blocked，先向 root 提出公共 codec/Repository 需求，禁止测试自造 431 个结构体顶替生产映射。
4. 扩展真实 Repository 的来源、外键、唯一与删除、封存/GC、幂等/重启、取消/提交与缓存/人工修正覆盖，逐项产生实际断言及独立回执；复用已完成限容卷证据但本波不重跑满盘。
5. 组装可运行的完整 receipt producer 前保留 partial。22 项、431 typed mapping、12,289 probe 与实际迁移路线未齐，不注册完整 adapter；PRE-14 authority 保持 pending，不修改基线。

## 验证与完成条件

锁定 Cargo 实际编译/运行和 Clippy；只有具体断言完成才输出对应唯一 marker，不遍历清单打印 PASS。独立字段/子项报告必须标注运行来源、生产 SQLite、输入 contract 哈希、具体错误与未执行项。新 guard 验证拒绝不完整 typed mapping/字段 probe/22 项及旧回执；完整 adapter 仅在全量实际覆盖和 root 独立政策评审后注册。

不提交、不推送，不改变批准图、schema、容量政策或计划 CSV 状态。
