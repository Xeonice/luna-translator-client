# 即时配置安全控制工作项

- 需求：FP-06、FP-08、FP-12；AI-CFG-11/13；MC-05、MC-12、MC-13（即时停用子项）、MC-19。配置实体删除/复制/导入不在此项实现范围；补上凭据主动撤销与失败后清理闭环。
- 设计：`docs/design/first-product-v1/index.html?scenario=settings`，连接详情与用途配置；沿当前整稿及材质，整稿待评审状态不提升。模型无独立启用开关。
- 模块/组件：M04/M05/M09/M10；C26/C27/C29 配置相关组件（实际 UI 由根任务接入）。Owner configuration_lifecycle_impl；独立审阅由根任务安排。
- 路径：`crates/app-core/src/contracts/mod.rs`、`application/runtime.rs`；`crates/app-infra/src/storage/queries/configuration*.rs`、`queries/mod.rs`、`queries/tasks.rs`、`queries/credentials.rs`；`crates/app-infra/tests/configuration_lifecycle.rs`。不修改迁移 DDL、schema 合约、基线、UI 或生成 TS。
- 复用：现有 writer 事务、operation_receipts 幂等、authorization_epoch/control_revision CAS、任务失效和运行时 abort_invalidated、既有请求授权检查。
- 语义：已应用 `purpose_bindings.enabled` 表示用户启用意图；`purpose_runtime_states.is_enabled` 是独立运行安全许可，实际授权同时检查二者及连接/凭据；运行许可开启不单独表示整体可运行。普通 apply 保留已有安全许可，不能重开停用；新绑定首次显式 apply 初始化许可。明确恢复要求当前已应用意图开启，沿已应用版本且不读取草稿。连接安全闸独立于连接的 availability_intent。global 控制影响继承它的游戏，独立 game override 分别控制；复合 FK 禁止创建不对应实际绑定的继承游戏状态。
- 验证：真实 SqliteStore/Repository + 受控网关；连接停用与完成竞争、迟到片段、跨游戏/并行引擎、用途隔离、保存/测试/apply 不恢复、重启保留、CAS/幂等、明确恢复不借草稿；不声称是真实服务/最终包验收。日志 `.build/configuration-lifecycle-tests.log`。
- 完成条件：上述命令与投影编译；用例实际通过；根任务接入 UI/IPC 后再做正式包验证。未执行或环境阻塞如实记录，不改既有产品计划状态。

## 实际实现与结果（2026-10-19）

- 连接即时停用/恢复：控制版本 CAS + operation receipt；仅影响匹配任务冻结配置/引擎的依赖连接。其他连接/引擎仍可运行；历史不删除。
- 用途即时停用/恢复：运行许可独立保留，显式恢复以当前已应用意图核验，ordinary apply 不恢复；global 控制实际继承者，独立 game override 不串用。
- 补充范围：`application/configuration_controls.rs` 与 `application/mod.rs` 导出、`queries/credentials.rs`。凭据影响查询与事务内相同计算，撤销请求按 lifecycle revision + impact hash CAS，并用 operation_receipts 保证幂等；先持久化授权关闭/取消，再 abort/清片段，最后清系统秘密。失败保留 revoked、同 operation 重试继续清理、重启回收。旧历史池不关闭已换成其他池的当前连接；同凭据多池引用只生成一次恢复清理操作。
- DTO 仅列非秘密引用、已应用来源、运行安全许可与影响，不声称远程服务在线。没有新增 schema 字段或放宽 schema/视觉基线。
- 实际新增 12 项 Repository/runtime 用例：`.build/configuration-lifecycle-tests.log`，12 passed / 0 failed / 0 ignored。受控网关不作为真实模型或正式包结果。
- 统一产品门禁、前端/IPC接线、正式包与最终回归由根任务完成；MC-13 的配置实体删除/改绑子项不在本工作项，不能据此声明整项 MC-13 或首版发布通过。

## 独立审阅发现的注册竞态修复

- 原全局 snapshot 后直接 retain 注册表会误取消期间新注册的独立 B，留下 DB running；现先采旧候选 ID，再读权威快照，最终 tasks→partials 同锁域只清候选失效项，并按实际仍注册的任务保留片段。没有持锁 DB/await。
- 真实 Repository snapshot 屏障确定性强制 B 在捕获旧快照后注册/进入网关/发片段，旧实现副本失败；新实现 B 片段保留且完成，A 取消，DB 无挂起。另强制新 A 在 prepare 后被停用、清理后才注册，发送前权威校验阻止它进入网关。
- 最终 `.build/configuration-lifecycle-registration-race-after.log`：生命周期14 + 原runtime8 + retry4，共26 passed，0 failed/ignored；Clippy严格检查通过。负例 `.build/configuration-lifecycle-registration-race-before.log` exit101；`.build/configuration-lifecycle-race-results.json` 保留源码/日志身份。源码已冻结，独立复核通知已发送，结论由根任务核实。

补充：注册竞态修复后的 all-target Clippy 曾被并行 storage_disk_full 的 E0382 阻断；其 Owner 已修复并冻结，最终统一 Clippy 由 root 执行。本子任务不把修复前绿色记录当作最终统一通过。
