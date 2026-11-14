# STORAGE-08：真实隔离卷磁盘满实施卡

- Owner：`storage_acceptance_impl`；关联 FP-11/FP-12、STORAGE-08、TP-03@INFRA。
- 授权范围：仅新增 `crates/app-infra/tests/storage_disk_full.rs` 与本卡；若需要额外测试驱动，先向 root 给出确切路径。生产模块、已冻结的其他源码、独立合同及 authority 不修改。
- 方法：macOS `hdiutil` 创建独有 64 MB 可写 HFS+ 镜像并挂到本轮 `.build/storage-disk-full/<run-id>/mount`；以 attach plist、设备号、挂载点及 `df` 容量同时核对与根卷不同后，才可在该卷内部创建填充文件和测试数据库。禁止填根盘、用户配置目录或未知挂载点。
- 行为：使用实际 `SqliteStore` / `ProductRepository` 执行业务命令触发真实磁盘满；不使用 `PRAGMA max_page_count`、假 IO 异常或测试替身。比较失败前后的阅读/来源/operation receipts/revision 及已提交对象；释放本次填充文件后，以同 operation ID 重试，必须恰好提交一次并可重开恢复。
- 证据：保存 hdiutil/平台容量与设备回执、实际 SQLite 错误码/文字、失败前后事务状态和恢复结果；成功 marker 只能在所有真实断言完成后输出，不能当作完整 22 项通过。
- 安全：目录名与 volume 名使用本轮随机身份；填充前再次校验 mount 的真实设备号。挂载不允许时记录 blocked，不提权或改安全设置。正常 detach 自己的设备，保留镜像及回执；不操作用户进程、数据库或 Keychain。
- 验证：锁定 Cargo 编译/真实运行该 target，Clippy；本项之外的完整存储 adapter 与 PRE-14 authority 审阅仍独立。

## 本波真实执行

- 最终 `cargo test -p app-infra --test storage_disk_full --locked -- --nocapture --test-threads=1`：1 passed，0 failed/ignored；Clippy `-D warnings` 通过。日志 `.build/storage-disk-full-real-run-final.log` 与 `.build/storage-disk-full-clippy.log`。
- 回执 `.build/storage-disk-full/13156-1792355146663834000/receipt.json`；自建 image `/dev/disk6`、卷 `/dev/disk6s1`，实际容量 67,067,904 bytes，设备号 16777241，与根/工作树设备号 16777232 不同。
- 自有填充文件实际返回 ENOSPC；同卷原生 SQLite 真实写入得到 `DiskFull`、extended code 13、`database or disk is full`。生产 Repository 也实际返回 `storage_failure` 与同一 SQLite 原始错误文字；Repository 当前错误 DTO 不暴露数值 SQLite code，因此原生 code 的证据明确归于同卷独立真实探针，不冒称来自 DTO。
- 失败前后 typed 来源行、8 张表计数、operation receipts、commit revision 完全一致，已提交对象保留。移除自有 filler 后，同 operation ID 恢复为 revision2，再次调用 replay=true，正文/会话/receipt 各恰好 2 条；重开恢复、integrity/FK 均通过。
- `/dev/disk6` 正常 detach exit0，且已核对原挂载路径回到工作树设备、`os.path.ismount=false`。镜像与证据保留，不触碰用户 DB/进程。
- 首轮驱动参数与管道 plist 解析错误留在各自失败回执；解析失败后附着的镜像已按该轮专属 attach 回执正常 detach，无遗留挂载。错误先修后重跑，未放宽断言。
- 仅 STORAGE-08 子项通过；未注册 adapter、未修改/采纳 PRE-14 authority 或抬升完整 22 项和发布状态。
