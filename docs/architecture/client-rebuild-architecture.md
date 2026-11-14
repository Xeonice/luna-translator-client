# 客户端架构入口

当前完整设计组合（guided与R79历史页）已由用户明确审核通过，身份见[源版批准记录](../development/design-approval-complete-20261031.json)；其他页面按[全页与状态闭包](../development/all-page-design-coverage.md)逐项对齐。设计源批准、独立参照、真实组件和正式包验收分别记录；原29个阅读/配置增量状态继续保留，不能代表整套页面。

[首版研发架构与工程基线](first-product-engineering-baseline.md)统一定义首版功能、技术栈、模块、组件和依赖方向；[当前客户端对齐总计划](../development/current-client-alignment-plan.md)定义实施依赖与准出。`FP1-20261030-R75` / 2102源是历史实现规划起点；现行完整冻结身份以[基线记录](../development/iteration-baseline.json)为准，默认[guided设计](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading)，历史页由R79设计覆盖，完整61源组合已获用户审核确认。

现有客户端已经具备app-core/app-infra、独立Core/双broker、语言worker、真实SQLite、macOS系统Vault、四角色UI和独立预览监督。历史页已有双查询/独立cursor与详情读取，但R79布局/轻文字操作、旧范围失败提示、编辑全部导航及focus/caret保留与定位后完整分页仍需CP-11迁移/实测。目标另有明确差距：独立当前阅读卡、三记录区默认折叠及状态保留、必要字段引导、13协议/21预设/4离线候选同源目录、typed凭据、已保存服务选择与细粒度激活。设计目录或HTML原型存在不代表这些能力已实现；完整视觉、实际系统/服务、最终包与四游戏准出仍分别核对。

业务权威在Rust，Tauri只做窄适配，UI不调用HTTP/SQL/FFI。M03持正文身份，M04持配置/资格，M05持调度，M06持协议/执行目录，M10持正文/raw/Hook三数据集与持久化，M11持偏好，M12持原生窗口和预览，M13持呈现，M15持脱敏诊断与准出，M16持秘密和授权。Core、collector、language-worker及目标translation-worker各自隔离；当前卡/折叠优先复用现有DTO，新增认证、资源、codec或关系先过契约与迁移审阅。

| 阅读目的 | 现行入口 |
|---|---|
| 功能、框架、模块与职责 | [研发基线](first-product-engineering-baseline.md)、[16模块矩阵](engineering-modules.csv) |
| 目录、文件与组件复用 | [工程组织](engineering-layout.md)、[组件矩阵](engineering-components.csv) |
| Core ABI、所有权与进程边界 | [ADR-003](ADR-003-client-rebuild-on-core.md)、[ADR-004](ADR-004-first-product-engineering.md) |
| 阅读、三数据集、HUD与预览 | [阅读/会话技术设计](reading-window-session-technical-design.md) |
| 历史会话目录、两查询竞态与固定对象 | [阅读技术设计第15节](reading-window-session-technical-design.md#15-历史页会话目录记录分页与固定阅读对象)、CP-11 |
| 词法、模型、凭据与离线翻译 | [词法/模型技术设计](japanese-study-and-llm.md) |
| 客户端运行、构建和证据边界 | [客户端使用与验证](../development/desktop-client.md) |
| 逐层测试和准出 | [工程门禁](../development/engineering-gates.md)、[当前覆盖](../development/current-product-test-coverage.md) |

历史原文与架构修订：[只读归档](../archive/documentation-r75/docs/architecture/client-rebuild-architecture.md)。
