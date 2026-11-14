# 存储运行验收实施卡

- Owner：`storage_acceptance_impl`；关联 FP-11、FP-12、STORAGE-01～22。
- 对照：首版范围、数据库字段设计第 13 节、`storage-acceptance.json` 的独立 22 项定义；不提升待审字段方案或发布状态。
- 本波允许路径：`crates/app-infra/tests/storage_acceptance*.rs`、`tests/support/storage_acceptance/**`、`scripts/quality/storage_acceptance.py`、`scripts/quality/test_storage_acceptance.py`、本卡及同变更内候选合同与执行说明；不修改生产业务模块、已批准设计或 authority 基线。
- root 后续明确授权增加封闭快照 inspector 修正：`scripts/quality/storage_schema.py` 与 `scripts/quality/test_storage_schema.py`。该入口只用于 candidate/版本快照比较，不是 live DB 诊断；先拒绝非空 `-wal` / `-shm`，再使用 immutable 只读读取，避免由检查本身生成 SHM。不得 checkpoint 或删除活跃用户库的 sidecar。
- 必要共享测试设施：`crates/app-infra/tests/storage_schema_contract.rs` 的比较 helper 把已完成的生产 backup 主文件逐字节复制为测试专用独立快照，确认不存在非空 WAL，再交给严格 sealed inspector；原 backup 及其 sidecar 不改动。该路径已向 root 明确告知。
- 设计影响：PRE-14 候选合同将生产 Rust 源文件 SHA 与实际 SQL 迁移内容 SHA 分开；允许显式 fresh/schema1 重开、同版本备份恢复及未知/无版本/损坏/迁移身份不符拒绝。拒绝路线不要求伪造一个合法旧版本，不构造不存在的 v2 升级能力。不自动冻结候选矩阵。
- 实施顺序：先写真实生产 Repository 测试 target 和可审阅候选合同、合同负例；再补逐字段 typed 映射、真实 SQLite 边界与 22 项运行覆盖。缺项必须保持 blocked/failed；不得遍历所需 ID 打印通过。
- 验证：锁定 Cargo 测试目标实际编译和运行；每个成功 marker 必须在对应真实断言后输出；Python 守卫负例应拒绝源/内容 SHA 混用、无版本来源被写成正整数、拒绝后原件变化。独立 schema 检查只读比较真实生产建库。
- 隔离：所有候选库、故障文件、受限卷及 Keychain 名称都必须是测试生成的专用命名空间；不触碰现有用户配置或唯一文件。真实限容/Keychain 不具备时报告 blocked。
- 完成条件：本波交付真实执行日志、候选路线结果和剩余矩阵；只有 22 项、全部边界与 431 个 Rust typed 字段映射实际通过且候选 PRE-14 独立评审冻结后，才可注册完整生产 adapter。此前统一产品存储门禁继续拒绝。
- 协作：共享工作树，禁止覆盖其他协作者修改；不提交、不推送。

## 独立审阅修正：所有 immutable reader 共用封闭条件

root 在本波冻结后的独立审阅指出：receipt route 分支只检查 `exists() && size`，无法识别悬空或指向零长文件的 WAL/SHM 链接；`source_schema=null` 的未知/无版本/损坏拒绝路线会跳过结构比较器的严格检查。root 已明确要求先修复再进行磁盘满子项。

修改仍仅限已列出的 `storage_schema.py`、`storage_acceptance.py` 及对应守卫文件：抽出 `sealed_snapshot_errors`，让 schema 比较、版本 ledger、拒绝来源检查和 receipt route/candidate 全部在 immutable 打开前共用同一个前置条件。新增 source_schema=null 的完整 executor 链接负例，以及三个拒绝来源状态对 WAL/SHM 零长/悬空链接的直接 reader 负例；未经前置通过不得打开 SQLite。未改独立 schema、阈值、adapter 实施状态或 authority。
