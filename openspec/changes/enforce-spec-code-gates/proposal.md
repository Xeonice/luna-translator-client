## Why

现有 AGENTS 和执行规则仍是文本。用户要求用 OpenSpec、源码边界检查和真实编译把规范连接到代码，违规时实际阻断，而非继续追加提醒。

## What Changes

- 初始化并固定 OpenSpec 1.2.0；首版 FP 与 MC/GL 验收作为目标规格导入，保留未实现状态。
- 用机器可读 sidecar 将 change 的 requirement/scenario 绑定检查，核对修改范围与权威文档指纹。
- 增加 TypeScript AST / Cargo 依赖方向守卫，以及精确历史债务的只减不增规则。
- 同一根命令执行规范检查、门禁正反例、TypeScript 和 Rust workspace 编译检查。
- 增加与包 hash 绑定的发布证据校验及 CI 工作流；缺少实测不能通过产品发布门禁。

## Capabilities

### New Capabilities

- `engineering-enforcement`: change 追溯、源码边界、真实编译、历史债务、源范围与发布证据的可执行约束。

### Modified Capabilities

无产品行为变更。`openspec/specs/first-product`、`model-configuration`、`game-onboarding`导入现有PRD/验收，不提升其实现或确认状态。

## Impact

仅工程工具、OpenSpec、根依赖锁、CI 和开发文档。不得修改 apps/crates/native/resources 产品源码和资源；不初始化 Git、不提交推送、不启动游戏。完整设计视觉差异和原生产品验收仍是产品任务，当前发布应被阻断。
