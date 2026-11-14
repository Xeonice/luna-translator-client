# SR1迭代卡

日期2026-10-27，父基线R47。用户授权：开始处理，直至整个重构计划完成。

追溯FP-03/12、RW预览身份/状态；整稿interaction-review.html#01-window-preview，M12/C35。PL实现、AP独审、QA实跑。产品Owner仅window_preview.rs和私有state.rs（内嵌测试）；Root拥有本change、当前阶段/进展、技术说明、源码清单及R48精确评审包。并行协作者不得回退他人。

依赖：SR0已完成；迁移前完整Native测试；原视觉来源冲突仍阻断相关完整验收，不阻断独立纯规则工作。复用现有Lease、projection及frame类型。所有外部契约/预算/安全核验/平台effect不改，无数据库或新依赖。

验收：原Native完整维护检查和新增表征不减断言、TS/Rust编译/边界/合同漂移/五类负例与真实视觉检查均实际执行。失败或外部缺项保留，不宣称完整切片通过。真实包/四游戏另由SR6执行。
