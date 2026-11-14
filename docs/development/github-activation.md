# GitHub 仓库与工程门禁

本项目公开仓库为 [Xeonice/luna-translator-client](https://github.com/Xeonice/luna-translator-client)，与原版 Python 项目的既有 fork 分开维护。此处规定源码、资源和远端验收方式；实时状态以 [Actions](https://github.com/Xeonice/luna-translator-client/actions)、实际 main 保护配置和 `.build/quality/github-activation-20261115/activation-result.json` 为准，不用配置文件存在表示执行通过。

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
