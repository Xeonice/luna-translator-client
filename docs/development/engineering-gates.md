# 可执行工程门禁

当前执行协议为 `effective-gates/v1`，依据[已批准的门禁设计](gate-validation-design.html)与[实施变更](../../openspec/changes/implement-effective-development-gates/proposal.md)。九类门禁均参与统一判定，手工仅是G09的裁决方式，不降低其效力。

## 统一入口与触发

```sh
npm run verify:quick -- --change <id>
npm run verify -- --change <id>
npm run verify:delivery -- --change <id>
npm run verify:release -- --change <id>
npm run test:guards -- --external-evidence <actual-inputs.json> --output <fresh-directory>
```

`verify`调用development_gates.py，默认工程阶段；quick只检查适用规范、范围、边界与技术，不授交付；delivery检查工作项完成，release要求全部九类及真实产品证据。`verify:legacy`保留原verify.py及原断言，历史失败不改写，不能用另选入口把当前失败改成通过。

SessionStart加载当前工作项，PreToolUse对明确越界目标拒绝，PostToolUse记录真实变化和必需检查，Stop在实际交付时核验。热probe有界且不作完整证据；opaque、大文件或未配对事件要求正式全量验证。正常问答放行，外部和人工blocked不无限重跑，同失败最多两次自动续跑。项目与全局hooks共存，信任和真实触发须有实际原件。Hook异常不是OS隔离，项目命令独立拒绝缺失或过期结果。

## 九类判定

| 门禁 | 强制内容 |
|---|---|
| G01需求与范围 | PRD/OpenSpec与新增真实入口双向映射；实际改动的独立审阅，正确ID不能替代语义 |
| G02模块 | 现有TS AST/Cargo依赖，UI不直接HTTP/SQL/FFI，Rust业务权威、窄Tauri和独立Core/worker |
| G03目录 | Git真实base或本地受授权冻结快照；新增/删除/重命名/未跟踪/路径逃逸，快照指纹不可重写隐藏变更 |
| G04技术栈与资源 | 现有technology锁、清单、ABI/架构和资源检查；不扩大legacy-debt |
| G05编译 | TS生产和测试、cargo workspace check和test --no-run，真实命令不能以lint代替 |
| G06功能 | 正式组件/端口、取消切换与迟到结果、重复提交、失败恢复；实际效果不能只看按钮 |
| G07存储 | 独立schema和实际迁移/Repository；类型、NULL/边界、外键、唯一性、事务、备份恢复 |
| G08正式包实机 | 实际包/签名/source/Core/资源及同包四游戏、真实翻译；消费原正式执行器和原完整用例，不接受合成状态 |
| G09视觉 | 一份设计/实现报告，同尺寸/数据/状态；明确授权人审、范围与当前实现身份；待审或过期不准出 |

G09未生成not_run，缺图/不可比/拒绝failed，待人工blocked，当前完整明确批准才passed。代码拥有的页面依赖决定受影响状态；报告必须绑定完整执行源码身份，不能自报一个依赖子集绕过时效检查。当前采用保守的完整输入绑定，输入变化后重新核对，不能自动沿用旧确认；旧设计源批准不等于本次客户端批准。无UI影响记录不适用理由，不伪造视觉通过。

## 门禁有效性

32组计划固定在[gate-cases.json](gate-cases.json)，原计划仍planned/not_run；每次真实运行另建独占目录。正/负/恢复、故障和变异检查预期规则、阶段、退出与副作用，任意非零/检查器崩溃不算正确拦截。L1协议、L2真实命令、L3真实Codex事件分别留证；模拟JSON不能冒充L3。GV31/32用隔离审核样本验证G09判定，正式产品拒绝fixture，门禁启用不依赖产品先通过。

真实回执绑定代码注册argv及检查类型、源码/规范/规则/测试输入、原始日志、时间和断言数；从日志独算计数，缺失、零断言、必选skip、篡改、串用、执行中源变化都不通过。tests不能改标command跳过断言核对。正式包及实机原件还绑定非源码文件与原始引用闭包，原路径换包或替换原件使回执失效。工作项不能通过声明kind/checks/UI=false缩减真实影响。

## 历史与发布义务

原153组合、RW逐层、存储专项、全量字段关系和四游戏义务不减少；原产品源、批准图、旧基线与失败原件保持身份。新授权替代本轮QA工作的旧许可链，不重建不存在的历史原件，不刷新旧违规hash。工程/自测通过不提升产品完成。

Git hooks安装器在无仓库时明确blocked，不自行init/commit；首次公开导入必须另有用户授权和精确bootstrap审核，之后为真实仓库配置core.hooksPath。远端采用[GitHub激活合同](github-activation.md)的可信base准入、只读执行与隔离Check Run发布，正常/拒绝PR和main保护必须实跑并读回，不能凭YAML或同名绿灯宣称生效。实际状态和限制统一见[结果页](../../reports/gates/index.html)及GitHub原件。
