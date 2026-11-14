# GitHub 仓库与工程门禁

本项目公开仓库为 [Xeonice/luna-translator-client](https://github.com/Xeonice/luna-translator-client)，与原版 Python fork 分开维护。源码基线已公开；受锁构建依赖资产不是应用安装包或产品发布。

## 当前执行依据

| 验证项 | 已发生的结果 | 实际依据 |
|---|---|---|
| 仓库与源码 | 已公开，完成精确清单审计和首次导入 | 首次完整源码 `ddfa993f2330cccc4708a2da88cb046c78cc456b`，保留 GPL、许可、批准图和旧债务指纹 |
| Git 与正常 Codex hooks | 已安装并真实触发 | 四项新定义经官方信任；无 bypass 正常会话、配对事件、24项全局配置保持原样，见 `normal-project-hooks/result.json` |
| 冷机完整工程 | 正常候选通过 | [真实运行](https://github.com/Xeonice/luna-translator-client/actions/runs/43652040975)：144项控制测试、32组303断言、四项真实适配器92项，以及TS／Rust生产和测试编译；完整源码身份 `ee56fc58…`，产品仍 incomplete |
| 未审核候选 | 开放、非草稿 PR 被阻断 | [PR #1](https://github.com/Xeonice/luna-translator-client/pull/1)；准入失败和工程跳过分别记录，不据此声称G03已运行 |
| 具体 G03 规则 | 实际命令拒绝允许范围外文件 | [运行](https://github.com/Xeonice/luna-translator-client/actions/runs/43652096905)明确报出 `apps/desktop/src/ui/github-gate-scope-negative.ts`，`scope.actual_diff_rejected`；这是预期拒绝，不把整个负例变成正常通过 |
| 同名伪绿 | 假检查成功，PR仍 BLOCKED | [PR #3](https://github.com/Xeonice/luna-translator-client/pull/3)含重跑；[PR #5](https://github.com/Xeonice/luna-translator-client/pull/5)的假job在可信失败之后才新建，仍被阻断。读取全部检查和实际合并状态，不只看绿灯 |
| 同提交恢复 | 错配锚先拒绝，恢复后完整重跑成功 | [恢复运行 attempt 2](https://github.com/Xeonice/luna-translator-client/actions/runs/43652630674/attempts/2)；同一提交恢复至 CLEAN / MERGEABLE，失败原件保留 |
| main保护与正常合并 | 实际设置、读回并按规则合并 | 唯一必需检查 `Luna / engineering`，app `15368`，strict与管理员约束开启，禁止强推和删除；[正常 PR #2](https://github.com/Xeonice/luna-translator-client/pull/2)在保护不变时合并 |
| 本文档候选 | 覆盖更新现行说明，不预填自身CI通过 | 每个最终文档候选仍需独审、固定审核锚和完整CI；精确head/base/run见下述机器记录和[当前Actions](https://github.com/Xeonice/luna-translator-client/actions) |

完整原件索引为 `.build/quality/github-activation-20261115/activation-result.json`。上述已验证实现与本文档候选分别绑定：源码变化后不能继承旧审核或四项生产回执；最终文档的实际运行结论在机器记录中更新，不为给同一候选盖章再改源码制造递归身份。首次ANSI日志启动失败、预期拒绝、过期原件和恢复前失败均保留。

**不能仅凭 `gh pr checks --required` 的绿色判断可合并。** 延迟伪绿实测中该命令曾只显示假job成功，而GitHub实际仍BLOCKED。验收须核对真实head/base、`filter=all`的检查、测试合并SHA、保护字段和PR实际合并状态；正常放行另以受保护合并事实验证。本轮反例覆盖只读候选执行环境，不宣称对拥有修改仓库规则权限的管理员作对抗保证。

当前产品仍为 **R93 / incomplete**；本轮未修改产品源码或安装包，也未授予正式包、四款游戏、真实翻译或人工视觉通过。

## 首次导入与资源

保留 GPL、第三方许可、当前产品源、批准图和旧债务指纹。首次源码基线必须有精确候选清单、敏感内容扫描及非作者审核；之后用真实 Git base 比较，不以全零 SHA 或自报快照代替。

`reports/`、运行诊断、构建目录与 `apps/desktop/src-tauri/resources/` 生成副本不进入 Git。三个大型语言资源使用 `.github/ci-resource-lock.json` 对应的依赖资产；`python3 scripts/quality/github_ci.py resources` 下载时核对整体 ZIP、固定成员清单和原 `resources/language/manifest.json` 的逐文件大小/hash。原始许可和来源仍随源码提供。该资产是构建依赖，不是应用安装包或产品发布。

干净目录还必须运行 `python3 scripts/quality/github_ci.py compiler-resources`，从逐项校验的实际语言资源准备 Tauri 编译目录，不能依赖本机旧 staging 或创建空占位目录冒充已准备。该步骤不授予 Core/broker/worker 完整打包或游戏资格，正式包继续使用既有完整打包入口与G08。

## 远端触发与权威

工作流使用受保护 base 上的 `pull_request_target` 定义，并分为三个独立执行环境：

1. **准入**：不 checkout、不执行候选代码；核对 API 中真实 PR head/base、管理员固定的完整 `GATE_REVIEW_HEAD_SHA`、独审包 URL 与 SHA。
2. **工程**：准入后只读权限、无部署秘密、checkout 精确 head，关闭持久 Git 凭据。恢复固定资源/L3原件和独审，在冷机重新运行生产组件、时序、迁移、存储，再执行统一入口的 `--stage ci`。该代码拥有的profile即使仅文档/工作流差异也强制G01～G05、控制测试及完整32组，实际产品影响仍触发G06～G09，不能靠metadata或步骤标题假定全量执行。
3. **状态发布**：新环境不 checkout 候选、不信任其自报结果，只在正常 PR 的准入与工程都成功且 head 未变化时，向同一 head 写 `Luna / engineering`。失败、skip、取消、缺失和过期均非成功。main 将该实际状态设为必需检查；状态发出者、strict/admin/force/delete 等须实际读回。

管理员变量是树外审核锚，避免审核包包含 full source map 后又把自身 hash 写回源码造成递归身份。它只引用真实非作者审核原件；缺失、换包、错源码与旧迭代仍拒绝。PR 无权修改这些变量，修改 PR 中同名 workflow 也不会替代 base 工作流。首次建立 base 本身仍须独立审核。

`workflow_dispatch` 必须明确真实 base/head；管理员可执行 `negative_probe`，但这种手动实验绝不发布授予合并的成功状态。常规推送由本地 Git hooks 检查，远端以 PR 和 main 必需检查控制合并；不在公开 PR 上部署本机 self-hosted runner。

## 日常开发与合并

新clone先按锁文件恢复资源与真实原件，并运行 `python3 scripts/quality/install_git_hooks.py` 安装本机Git hooks。每次工作按当前OpenSpec限定路径实施；完成后由非作者审核完整源码，上传独立审核包，再由管理员固定 `GATE_REVIEW_HEAD_SHA`、`GATE_REVIEW_BUNDLE_URL`、`GATE_REVIEW_BUNDLE_SHA256`。

正常PR触发完整CI；新head必须重新核对，不能复用旧审核锚。同一head遇临时环境故障可修复外部条件后重跑，仍保留失败记录。即使只改文档，也执行 `--stage ci` 的完整工程档。只有当前必需检查满足、实际可合并且保护保持有效时才合入main；负例PR保持未合并，实验后关闭。

CI固定适配器的无颜色日志环境，避免Vite端口被ANSI装饰后无法解析；成功或失败均恢复外层环境。此设置不改组件断言，实际Vite／HTTP正反例验证该问题。

## 真实 Hook 与最小原件

项目 Hook 命令通过 Git 根定位，保留四类 handler、matcher 与超时。L3 producer 记录真实执行源根，消费者只按固定命令合同物化部署根及隔离逻辑根；七项当前源码、原始 Codex 事件、关闭 Hook 对照和文件副作用仍逐项核对。只公开隔离验证所需原件，不上传 Codex 账号、用户会话、凭据或生产记录。

`.github/ci-evidence.lock.json` 固定真实 L3 传输包。冷机四项生产回执必须绑定当前完整源码，不能继承上一轮的小依赖子集。G01 审核包从管理员变量恢复，其源码 map 必须与实际 checkout 及恢复的受锁资源相同。生成的 Tauri staging 不属于源码身份，根资源和许可继续参与身份核对。

## G06～G09 怎样验证

| 门禁 | 自动或实际执行 | 人工/外部条件 |
|---|---|---|
| G06 功能 | 真实组件/业务端口、重复提交、取消/切换/迟到结果，观察实际副作用 | 完整首版流程仍需最终包回归；已有局部测试不代表整页通过 |
| G07 存储 | 独立字段/关系合同、真实 SQLite Repository、实际迁移、失败回滚、备份恢复 | 正式包真实 writer 与完整冻结存储义务另验 |
| G08 正式包实机 | 验包/签名/源码/Core/资源身份及同包证据；缺失、混包、换原件均拒绝 | SR、MO、Never7、樱之诗逐款 Hook、字幕、词法和真实翻译；hosted runner不能代替 |
| G09 视觉 | Playwright 真实组件截图和设计同尺寸/数据/状态放入唯一报告；程序检查完整性和审核时效 | 用户明确确认真实设计/实现对照；待审、拒绝和过期均不能准出 |

当前工作项若没有修改产品模块，适用性表可记 G06～G09 不适用；这与这些门禁的正反例执行状态、局部生产测试以及产品验收状态分别记录。门禁有效性实验不把产品变成通过。

## 验证闭环

正常 PR 应通过完整 Actions 并在保护规则下可合并；故意越界 PR 应失败/阻断，管理员手动负例还须从原件确认具体 G03 规则和路径，不把任意崩溃当拦截成功。最终记录实际仓库可见性、提交、run、PR、固定资产、正常 Hook 信任、本地 Git hooks 与 main 保护；未执行项保持 not_run，外部缺失保持 blocked。
