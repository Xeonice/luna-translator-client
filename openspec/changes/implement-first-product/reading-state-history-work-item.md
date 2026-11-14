# 阅读会话状态与历史筛选补齐

- 授权与基线：用户要求继续既定首版实施；基于 FP1-20261019-R9，不改变 PRD、材质、批准图或技术栈。
- 依据：FP-04/05/09/12、REQ-001/045；当前整稿 `#hud-window`、`#main-window`，历史页与会话结束/恢复状态。会话筛选为 FP-09 已有要求，现行历史稿只有游戏筛选，新增控件状态作为独立候选记录，不提升整稿批准状态。
- 模块/组件：M03/M10/M12/M13；C16/C17/C18/C34。Owner root，独立复核由其他协作者完成。
- 问题：停止/失败会话保留已选文本流时仍被显示为接收中；历史分页只按游戏过滤，手动筛选在 UI 截取本页会漏掉更早内容；产品详情暴露内部会话、unit 和内容 hash。
- 实现：复用 Rust 权威 session.state 的统一 UI 文案映射；停止状态保留原文、词典和学习，采集控制不可误示可用。历史筛选通过 typed application-port 和 Rust 参数化查询处理，游标绑定筛选范围，拒绝错用与坏游标；来源、会话和时间显示为用户可读信息。
- 允许路径：features/reading/**、features/history/**、windows/main/MainWindow.tsx、windows/hud/HudWindow.tsx、bridge/**；app-core contracts/ports/application；app-infra storage/database.rs、storage/queries/mod.rs 及独立 history 模块；Tauri IPC 查询/注册；新增真实存储历史测试。视觉脚本和受控端口由 product_ui_impl 独占，root提供 UI36..38 用例需求后整合。
- 验证：真实 Repository 的跨游戏/多会话/手动来源/相同时间分页、筛选范围变化与错游标；Playwright 验证终态不显示接收、不启用采集控制、历史筛选与加载更早记录及隐藏内部身份。正式包复核另记来源和实际结果，不预填 passed。
- 依赖与限制：不修改 DDL、原始计划 CSV、431 字段规范、PRE14 或发布 adapter。模型/游戏四款最终同包回归仍需独立完成。

## 恢复回归补充（root）

新增 sourceSummary 后，重开/备份恢复的原全值断言会把生产恢复正常更新的 session 状态误当作正文变化。独立架构审阅确认后，`storage_acceptance_routes.rs` 明确先检查 active/manual/原始时间，再保留完整 ReadingUnit 全值比对，仅将恢复时的摘要 state/endedAtMs 设为实际 crashed/203 或 400；没有忽略摘要、删断言或改生产恢复。真实两条存储路由测试通过，日志 `.build/wave2-storage-recovery-regression.log`。

受控 UI 新增 UI-41 模型默认模板编辑/草稿保存/作用域隔离/恢复/复制 CAS，UI-42 独立短词卡冻结/明确 AI 操作/迟到关闭代际。根负责 `verify-ui.mjs`；测试端口不实现模型复制业务，Rust Repository 测试独立验证。门禁库存从 40 扩至 42，独立审阅后才更新权威指纹，不改变视觉阈值或批准图。

## 报告证据归属（root）

`scripts/report/build_product_verification_report.py` 改为独立新输出目录，不覆盖旧报告；只自动选择与当前 build-manifest 同整树身份且含 steps/profile 的完整门禁，拒绝把字段探测 JSON 当完整门禁。可显式传入 UI/native/组件日志；历史不同源码记录标 historical_different_source，不计入当前包。受控 UI 逐文件比对标 same_current_ui_sources_only，保留失败视觉图及限制，不冒充正式包/真实游戏通过。组件日志数量有交集，不汇总为产品覆盖。生成后用报告机器索引和原回执核对。

## 满盘重开回归补充

首次统一门禁实际拒绝 storage_disk_full 的旧恢复预期（保留失败回执 `.build/storage-disk-full/12578-1792360194371530000/receipt.json`）。独立架构核对确认与前述动态 sourceSummary 相同：满盘期间原全值比较仍必须完全不变；只在重开 now=200 后允许 session state=crashed/end=200。根保留满盘原断言，新增创建时精确摘要检查，重开后两对象均做完整全值比较；恢复后对象从只比正文加强为全对象。`.build/wave2-all-infra-after-recovery-fix.log` 实际 16 suite / 141 passed、0 failed/ignored；原生SQLITE_FULL=13、事务回滚、幂等恢复及卸载仍由实际测试执行。此后重新打包再统一验收，旧6ec2包不当作改后同源包。

## 规范守卫负例前置状态补充

R10 已经明确审阅并冻结有限 PRE-14 工程矩阵后，原守卫单测仍隐式假定当前 registry 为 pending。独立架构审阅在内存真实调用守卫证明：给该“伪造 pending 版本”负例明确设置 pending_review 可同时触发原 execution claim 与 cannot invent supported versions 两个断言。根只增加此 fixture 状态；生产守卫、原断言和预期文字均不变。新 R12 仅独立审阅后的初始主窗配置单项技术锁 SHA 更新，旧锁与 R11 基线完整保留；不提升任何验收状态。
