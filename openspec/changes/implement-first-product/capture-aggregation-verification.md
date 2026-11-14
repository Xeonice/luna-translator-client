# 聚合修复执行记录

本记录属于 IMP-CAPTURE-AGGREGATION-20261019；不提升新正式包、四游戏或完整发布状态。

## 实现与焦点检查

- 8 个获授权 Rust 产品路径已实现；公开契约、DDL、资源、依赖与 R12 未修改。
- `.build/capture-aggregation-rust-tests-20261019-r5.log`：`cargo test --locked --offline -p app-core -p app-infra --test application_runtime --test storage_product --test history_queries --test configuration_lifecycle --lib`，实际 115 passed、0 failed/ignored。
- 分项：app-core 9、app-infra lib 16、runtime 18、configuration lifecycle 14、history 9、storage product 49。
- `.build/capture-aggregation-clippy-20261019-final-r2.log`：`cargo clippy --locked --offline -p app-core -p app-infra --all-targets -- -D warnings`，exit 0。其间修复两个可折叠 if 和一个测试布尔表达式的 lint，未改变预期。

新增回归覆盖逐字/分行、直接 watch 原文先到、稳定后确切版本、合法重复保留、原始跨度重建、旧 revision、三段超长 UTF8/三个事件 ID 稳定顺序、期限和容量、生命周期事务/CAS、暂停有序 marker、在 Tokio future 执行前已断开/重选的确定性交错、open 原始负载与对象清理保护、异常/重启、正常退出 stopped、退出失败后继续取文与重试成功。

既有历史多修订分页断言保持不变；latest 投影仅适用于明确声明 unitProjection=latest 的聚合内部修订。原清理测试补真实 finalized 前置，保留原删除数量/回滚断言。原翻译关闭测试补逻辑时钟稳定期，仍由真实 runtime timer 封口；未直接 flush 伪装自动调度。

## 发现并修正的失败

首轮新增与既有关联锁重复使测试挂起，已中止并删除重复锁。后续实际失败分别定位到受控时钟未进入稳定期和旧清理 fixture 仍为 open，均先修正确前置，未修改断言或预期。失败和挂起日志保留，不算通过。

独立 reviewer 两轮发现的锁内暂停/退出边界、容量排序、晚调度权威、open payload 保护、退出失败恢复、正常 stopped 及最终 max revision 均已修复并加入实际回归。最后一轮复核尚待主任务可用 reviewer 执行；不把 agent 容量不足写为已审阅。

## 仍需独立执行

- 主任务同源统一 verify、五类规范负例及新正式包构建/签封/manifest。
- 新正式包四游戏重新采集；特别是 SR 慢速跨行原文/译文及其他游戏不误合并。
- 原有视觉与完整存储发布验收状态分别保持原有证据，不因本次 115 项通过自动提升。
- 无页边界 GDI 的聚合仍是有界启发式；默认策略不代表全部 Hook 参数设置 UI 已交付。不提交推送。
