# 客户端重建架构入口

当前研发方案统一为[首版研发架构与工程基线](first-product-engineering-baseline.md)，本轮实施规划入口为[当前客户端对齐总计划](../development/current-client-alignment-plan.md)。2026-10-31核对冻结 `FP1-20261030-R74` / 2087源：`app-core`、`app-infra`、独立Core/broker、语言worker、真实SQLite/Vault、四角色UI及平台预览均已有源码；原先“未继续客户端研发”只反映早期方案阶段，不能用于描述当前工程。

最新目标为 [guided-service-reading-20261030](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading)，仍 `pending_review`。本轮更新文档/PRD/测试与准出规划，未修改客户端代码或运行包。已有源码不等于全部产品验收；13协议/21预设/4离线候选、typed凭据、共享浅层选择、必要字段引导及三记录区折叠均按专项差距分别处理，不因HTML原型存在而宣称已落地。

当前主要迁移复用原模块：M03/AP提供正文身份，M04/AP配置与资格，M05/AI调度，M06/AI协议，M10/AP三数据集与存储，M11/AP偏好，M12/PL原生平台，M13/UI呈现，M15/QA脱敏诊断和准出，M16/PL凭据。当前卡/折叠容器优先沿既有DTO实现；注册/typed授权/离线资源若需新DTO、codec或持久关系，先独立契约与迁移评审，不改变Rust业务权威、窄Tauri、Core/worker隔离或技术锁。

- [首版功能、框架技术栈、模块与职责](first-product-engineering-baseline.md)
- [目标目录、文件职责和依赖规则](engineering-layout.md)
- [16个模块与Owner](engineering-modules.csv)
- [34个组件族与复用位置](engineering-components.csv)
- [Core接入决策 ADR-003](ADR-003-client-rebuild-on-core.md)
- [首版工程决策 ADR-004](ADR-004-first-product-engineering.md)
- [词法/模型详细契约及R74服务差距](japanese-study-and-llm.md)
- [阅读/三数据集/独立预览迁移合同](reading-window-session-technical-design.md)
- [当前客户端与历史0.1.3切片](../development/desktop-client.md)

此前规划1.0引用Reader0.1.1、早期HUD尺寸和旧视觉依据，存储/组件选型也未确定；其字节快照保存在[历史归档](archive/pre-engineering-baseline-20261018.zip)，不再作为现行总架构。旧文中的Core ABI、回调所有权与进程隔离约束已保留于新基线及ADR-003。原始PRD和既有实机证据不受本次文档调整影响。
