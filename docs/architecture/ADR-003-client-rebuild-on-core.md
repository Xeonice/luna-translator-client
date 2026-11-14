# ADR-003：基于精简 Core 重建客户端

日期：2026-10-18。状态：规划采用，尚未实施。用户已明确本轮只做重建与回归规划。

本ADR继续约束Core进程/ABI边界；首版完整应用层、存储、组件和目录选型由[ADR-004](ADR-004-first-product-engineering.md)及[首版研发基线](first-product-engineering-baseline.md)补齐。下面的Reader0.1.1是形成本ADR时的历史基线，当前切片已到0.1.3，仍未集成新Core。

## 问题与依据

现有 Luna Reader 0.1.1 已能正式打包并显示原文，但它仍使用原版十回调 LunaHost、旧注入命令和原版资源锁。独立维护的 [luna-hooks-core](https://github.com/Xeonice/luna-hooks-core) 已完成源码裁剪，并提交于 `e9242ebac32a09e911f306b7c90714891c217302`。它使用 Core ABI 1、七回调和独立注入器，与当前 Rust FFI 不兼容。

Core 的四款游戏验证由原生回调探针完成；它们证明提取基础可用，不能证明新客户端已接入。原 PRD 和设计稿仍是产品范围依据；本轮不再寻找旧 Rust 工程，也不另起一套产品设计。

## 决策

1. 复用当前 Tauri 2、React/TypeScript、Rust 工作区与锁定依赖；保留协议校验、目标身份复核、会话隔离、日志和打包的有效实现。重建 FFI、资源准备、宿主管理和产品应用层，不机械重写所有文件。
2. Core 继续独立仓库维护，客户端通过固定提交、ABI、架构和文件哈希使用它的产物。客户端中不复制一份 Core 源码，不重新带入原版 Python/Qt、翻译和回写逻辑。
3. 首版选择与目标同位数的 Rust broker32／broker64，分别加载 HostCore32／64，启动同位数 Injector 和 Hook。Mac 原生进程通过同一个 CrossOver bottle 运行 Windows broker。此选择基于已验证路径，不声称 Core 在技术上绝对禁止 Host 跨位数通信。
4. 为 Core 建立单独的 FFI 与运行时 profile。计划将私有 JSONL 升至 v2，移除嵌入能力字段，增加 Core 身份、broker 架构、事件序号与明确的完整性状态；ABI 1 与 JSONL v2 是两个独立版本。旧产物保留作对照，不自动回退到旧组件掩盖错误。
5. Rust 应用核心统一管理输入、选流、显示单元、翻译、配置和历史。Tauri、未来 CLI 与产品 Web API 使用同一用例层；网络调用和界面操作不进入 DLL 回调。
6. 先交付接入新 Core 的可打包原文版本，再交付真实翻译与原设计 HUD 闭环，最后按原优先级补齐全部产品能力。每个里程碑均有独立回归门禁。

本方案明确建议重排原 PRD 第 417 行所述“D1/GE 未过不得进入 D2～D6 生产开发”的顺序：M1/M2 使用新的、范围受限的 CrossOver 交付门禁，允许先建立客户端闭环；原生 macOS 真实提取另列 M3-P0。此项是本次重建方案对开发顺序的替代建议，不是原 D1/GE 已通过或 nativeMac 需求已取消的认定。完整 P0 和原 D1/GE 的产品声明仍须满足原功能与平台要求。本轮只有规划，没有据此启动后续生产开发。

## 影响与边界

- `Luna_GetAbiVersion()==1` 后才能注册七回调；x86 必须使用 C 调用约定。ThreadParam 按值布局须在两个实际 Rust target 上验证。
- 借用数据在回调返回前复制；回调不阻塞等待消费者，不做翻译、存储或同步重入 Host。
- Core 没有 Stop/Join 与安全卸载确认。生命周期继续以 broker 进程隔离，不对存活 Host 执行 FreeLibrary；断开回调不能被解释为游戏内 DLL 已卸载。
- 首版本地交付环境是 macOS arm64＋CrossOver。Windows 原生发布、原生 macOS 游戏提取分别验收，不能沿用该结论。
- Core 已删除内嵌回写。原 REQ-006 的 E 标记及 REQ-008/009 的相关业务保留在需求账本，先显示能力不可用；后续若实现，另立能力适配器与 ADR，不把回写塞回只取文 Core。
- 原设计快照逐字节保留。旧架构中十回调、旧注入器成功码和旧运行时方案由本 ADR 替代；历史记录保留当时语境。

实施细节见[重建架构](client-rebuild-architecture.md)，执行顺序见[开发计划](../development/plan.md)，通过标准见[产品回归计划](../development/client-regression.md)。本 ADR 不构成实施、实机通过或提交推送记录。
