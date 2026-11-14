# 项目协作约束

## 用户规则

- 始终用中文交流；尊重现有架构和逻辑，不为满足测试而硬编码行为或结果。
- 发现运行错误时先修复问题，再继续原任务；不能删减断言、修改预期或伪造结果掩盖错误。
- 只有用户明确要求提交时才提交；只有用户明确要求推送时才推送。提交前检查 debugger、秘密和意外文件。
- 高还原度要求使用 Playwright 截图与用户提供或确认的设计对照。

## 可执行约束入口

- 本项目已采用 OpenSpec；每次实现必须有 `openspec/changes/<id>/` 的 proposal、delta specs、design、tasks 和 verification.json。先读[可执行门禁](docs/development/engineering-gates.md)。
- 开发验收运行 `npm run verify -- --change <id>`；CI额外传Git base。规范、源码范围、技术锁、边界、逐项测试与实际编译任一失败都不能宣称本轮通过。
- 必须验证PRD越界、设计不一致、模块越界、目录越界和未批准框架五类负例；检查命令和实际回执绑定，不只填checks名称或任务勾选。
- 禁止扩大legacy-debt、刷新旧违规文件hash来绕过门禁、自动重拍批准图或把门禁自测当产品通过。发布不接受历史债务；产品UI缺真实候选截图检查、发布缺真实执行adapter时拒绝。

## 当前阶段与依据

- 用户于2026-10-19明确授权按既有PRD、设计和架构开始首版开发与对应测试；当前实施变更为 `implement-first-product`，范围见该OpenSpec与迭代卡。此授权不自动提升整稿、字段方案或正式发布评审状态。在已授权范围内自主完成，无需反复请求常规实现许可。
- 每次工作先读取 [PRD 索引](docs/product/README.md)、[首版范围](docs/product/first-product-scope.md)、[当前整稿](docs/design/first-product-v1/README.md)、[研发基线](docs/architecture/first-product-engineering-baseline.md)与[迭代执行规则](docs/development/iteration-governance.md)。
- 核对 [基线指纹与确认范围](docs/development/iteration-baseline.json)。指纹变化必须说明原因；不得自动更新指纹来掩盖未经评审的变更。范围、材质、整稿、架构和产品实测的确认状态分别理解。
- 当前完整设计仅 `docs/design/first-product-v1/`；旧专题、历史报告、未完成产品草稿不作为现行实现规范。原始 PRD 和 License 保留。

## 每次迭代

- 使用 [迭代卡模板](docs/development/iteration-template.md)，在编码前写明需求/验收 ID、设计页面和状态、模块/组件 ID、Owner、允许修改路径、依赖、验证方式与完成条件。
- 需要的映射、设计状态或前置门禁不具备时，先补齐；可继续独立工作，不能把缺口当作默认通过。
- 严守 Rust 业务权威、窄 Tauri 适配、独立 Core 与语言 worker 边界；UI 不直接调用 HTTP/SQL/FFI。跨层导入、协议和资源变更按架构规则评审。
- 普通实现细节由任务 Owner 决定。改变已确认产品行为、交互、公共契约或架构决策时，先记录影响并按执行规则处理，不能改规范迁就代码。
- 修改只限已授权的工作范围；并行协作者不得覆盖或回退他人变更。复用已有组件和端口，不复制业务实现。
- 完成报告必须给出实际改动、需求/设计对照、执行的检查、结果、证据与限制；编译成功、浏览器设计图或旧探针均不能代替正式包实测。
- 原计划 CSV 保持计划定义；每次实测另建运行记录。没有执行的写 `not_run`，外部条件缺失写 `blocked`；不得提前填写通过。
- 视觉候选输出到独立目录。未经明确评审不得覆盖已确认截图、修改 token 基线或放宽差异阈值。

现有可执行覆盖与限制以engineering-gates及本次机器回执为准；CI配置文件不等于已在远端运行或设为必需检查，守卫自测不等于首版产品验收。
