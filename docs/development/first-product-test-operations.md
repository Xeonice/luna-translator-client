# 测试前置、代码维护与证据归档

日期：2026-10-19 · 状态：设计与测试准备规划；不授权产品研发或实际付费调用。

之前的测试方案列出了环境和用例前置，但没有集中回答“谁准备、何时需要、缺什么影响哪些组合”和“以后在哪里维护”。本文件补齐这两个部分，与[逐层测试规划](first-product-test-plan.md)一起使用。权威登记为 [first-product-test-operations.json](first-product-test-operations.json)；18项前置、17类维护对象、原128个用例和153个组合的对应关系均可检查、导出和在HTML中筛选。没有新增产品用例，也没有把任何产品测试标为通过。

## 1. 现在需要你提前提供什么

不需要一次提供全部内容。先做规则、受控协议、数据库和组件测试时，不需要云端模型账号或四款游戏同时在线。进入对应实际服务/原生/游戏/分发组合前再核验该项；缺双屏只影响DISPLAYS的3个组合，不阻断其余基础原生测试。

| 需要你确认或协助的事项 | 具体内容 | 何时需要 |
|---|---|---|
| 真实模型与翻译资源 | OpenAI兼容、Anthropic协议、Azure传统翻译、本机兼容服务的可用端点与能力；LLM给实际模型ID，Azure给必要region/订阅配置；可共用服务商但按实际协议分别验 | B3真实服务前；原服务组合分别7/6/4/7，共24个 |
| 实际调用与文本授权 | 允许模型/用途、费用或token/次数上限、并发与时段、允许上传的文本范围；默认不擅自发起付费测试 | 任何真实请求前；多Key池仅MC-04专项另准备测试身份 |
| 游戏现场与操作窗口 | SR中文版、MO、Never7、サクラノ詩10th的可用安装/登录、对白存档或可复现推进路径；需要时协助手动进入和推进 | L7/B5实机前；每款4个组合，共16个；不测Ever17，不断开另一台Steam会话 |
| 原生与干净环境 | 可用Mac和权限操作时段；多屏组合另需真实1x/2x双屏；分发测试需符合策略的干净用户/设备及可恢复数据 | B2原生、B5安装前；双屏3组合与干净分发5组合分别核对 |
| 产品验收与人工日语审阅 | 完整设计最终评审；团队提出的性能/语言质量标准；指定或确认具备日语判断能力的审校者和争议复核人 | B0政策冻结及B4/B5内容质量前 |
| 发布及存储归属 | 分发渠道/主体、必要签名公证授权、是否迁移旧PoC数据；正式仓库/Runner/私有制品保存位置和访问留存政策 | B0决定范围，B5最终RC与持续执行前 |

普通非秘密信息可以登记到准备表。API Key、密码、证书私钥、公证密码在客户端设置、系统安全库或受控CI secret中配置；文档和报告只保留脱敏引用，不通过聊天、CSV、HTML、源码或命令行参数交付。先确定渠道和签名政策，再决定需要哪类授权；本轮不安装证书或操作发布账户。

本机游戏/CrossOver和Core的历史证据只能说明曾可用，当前登录、模型余额、硬件、签名授权均未在本轮复验。表中“历史证据待复验”“待确认”“待评审”“团队待交付”和“未配置”均不表示就绪。原产品组合继续not_run；真正尝试执行缺前置时才记录blocked。

## 2. 哪些由研发侧自行准备

| 研发侧交付 | 责任 | 关联任务/门禁 |
|---|---|---|
| 工具链、隔离配置档、可控时钟/barrier与合法fixtures | QA统筹，各模块Owner实现；不让用户写测试代码 | TE-01～06，B0～B3 |
| 新Core ABI1七回调、PE32/64 broker与整套资源/hash/许可 | HK与PL；旧GUI/独立C++探针不替代真实Rust消费 | TE-02/03，B1 |
| 真实SQLite/WAL/Keychain/HTTP/文件锁和worker故障工具 | AP/PL/AI/JA；使用临时DB、安全库前缀与可恢复副本 | TE-04/06/08，B2～B4 |
| 真正随包的分词器、UniDic、中文基础辞书及补充资源 | JA/PL准备字节、版本、覆盖和许可；用户无需自行找词典 | PRE-17、TE-08，B0资源冻结与B4离线验证 |
| 自制/授权语料、人工审阅rubric和预定性能测量方案 | JA/AI/QA先给候选，再交产品Owner评审；不在看到结果后调线 | TE-01/09/12 |
| 实际产品组件/原生/四游戏runner和capture/runtime adapter | UI/PL/HK/AP/AI按边界实现，QA汇总；目前仍planned | TE-05/10/11，B2～B5 |
| 最终RC、构建与安装清单、隔离迁移样本、归档与脱敏报告 | PL/AP/QA；缺包/缺词典是产品交付问题，不能甩给用户补环境 | TE-10/12，B5 |

完整[准备项CSV](first-product-test-prerequisites.csv)逐项列出交付人、Owner、当前状态、最晚门禁、解除方式和受影响执行身份。[组合依赖CSV](first-product-test-execution-dependencies.csv)列出153行 `case@variant → PRE → MA` 关系。PRE-16只限制双屏；PRE-12/17只在实际Core/语言资源组合或B0资源冻结要求，不反向阻断纯UNIT/合成协议。预算与多Key身份也分成PRE-06/PRE-18，普通服务不要求两把Key。

## 3. 测试以后维护在哪里

测试代码与产品源码在同一个客户端仓库随迭代维护，模块Owner对所属测试负责，QA负责覆盖、设施、证据和发布汇总。独立Core仓库继续自行维护C++内部测试，本仓库维护消费侧兼容及最终产品链路，不能相互代替。

下列为目标位置；除标明“已有”的工程设施外，产品测试目录与runner尚未实现，不预建空目录冒充完成。根集成旅程细分在既有 `tests/integration/` 下，不新增产品架构层。

```text
luna-translator/
├── docs/development/                       # 已有：权威计划/供给/Owner/用例及组合映射
│   └── first-product-test-{plan,operations}.json
├── openspec/                               # 已有：需求/场景 → 测试/实际检查入口
├── scripts/
│   ├── quality/                            # 已有：工程守卫及自身负例，不是产品测试
│   └── report/                             # 已有：HTML/CSV生成；报告QA长期源码也在此
├── crates/<crate>/
│   ├── src/... #[cfg(test)]                 # 规则单测贴近所属实现，按模块落地
│   └── tests/                              # crate公开API/协议/FFI合法集成边界
├── crates/app-infra/tests/                  # 计划：真实DB/Keychain/HTTP/worker适配测试
├── apps/desktop/tests/                     # 计划，当前未实现产品harness
│   ├── components/                         # 组件行为、键盘与可访问性
│   ├── visual/                             # 唯一产品截图基准与Playwright旅程
│   └── support/design-port.ts              # 仅测试构建，release不可引用
├── tests/
│   ├── contracts/                          # 计划：共享协议与跨进程兼容
│   ├── integration/
│   │   ├── application/                    # 计划：跨模块用例/投影/取消编排
│   │   ├── desktop/                        # 计划：最终包用户旅程
│   │   ├── native/                         # 计划：权限/多屏/安装迁移/签名
│   │   ├── games/                          # 计划：四游戏规程/校验器，非游戏资产
│   │   └── quality/                        # 计划：稳定性/语言质量与人工审阅工具
│   └── fixtures/{contracts,providers,storage,language}/
│                                           # 计划：版本化合成/授权样本与许可/manifest
├── .build/quality/                         # 已有：工程守卫命令回执
├── .build/client-regression/<run_id>/       # 产品运行临时输出，须另做受控归档
├── diagnostics/                            # 私有现场，不默认放进可分享包
└── reports/                                # 生成的脱敏HTML/ZIP，不是测试源码

独立 luna-hooks-core/                        # HK维护C++内部测试、ABI导出、构建和许可
受控制品存储/<run_id>/                       # 待配置：原始证据独立备份与权限/保留政策
```

Rust测试不豁免依赖规则：`app-core`不能为了测试通过dev-dependency反向链接`app-infra`；真实I/O测试放`app-infra/tests`，跨模块黑盒放`tests/integration`。确需新增测试crate必须先登记架构角色/允许依赖。前端测试端口不能进入正式bootstrap或release包；正式包也不能包含fixture词典或样例模型fallback。

设计规范唯一来源仍为 `docs/design/first-product-v1/`；产品Playwright截图基准位于 `apps/desktop/tests/visual/`，不是另起一套设计稿。原型/报告截图QA与实际产品截图明确分开。[维护位置CSV](first-product-test-maintenance.csv)逐项记录17类对象的状态、主责、触发时机、职责及命令现状。当前每个用例已有一个或多个维护区映射，尚未落地的测试文件只记录目标区域，不编造可运行文件名。

## 4. 谁负责、何时跑、失败怎么维护

Owner含义：AP应用领域/数据，HK Hook/broker，PL系统平台/分发/秘密库，AI模型服务/调度，JA词法/词典/语言质量，UI组件/视觉，QA计划/设施/证据/发布。不是由QA独自修复所有测试。

| 触发点 | 要维护和执行的内容 | 主责 |
|---|---|---|
| 每次需求或代码变更 | OpenSpec与用例/依赖/Owner同步；计划/供给守卫、工程边界与编译、受影响实际测试 | 模块Owner，QA检查覆盖；现有CI文件不等于远端已运行 |
| ABI/DTO/schema/资源变化 | 共享契约、双架构消费、拒绝旧版本、迁移与跨窗口一致性 | HK/JA/AP/PL及消费侧Owner |
| UI/token/字体/状态变化 | 组件行为与批准稿截图；原生行为另测；基准变化须独立候选/diff/评审 | UI、设计Owner、PL |
| 每个B阶段交付 | 该垂直切片L1～L6；失败修复后重跑及共享边界影响组合 | 模块Owner交付、QA组织 |
| 最终RC或重签/重包 | 同一最终RC的全部必需正式包/四游戏/服务/离线/分发组合；保留与同源低层结果的关联 | QA统筹，各Owner签结论 |
| 定时稳定性/服务兼容检查 | 先在B0确定周期与预算；与发布回归分开保存 | QA/AI/PL；目前未配置定时任务 |

失败归属到对应模块Owner，记录defect和首次失败；修复/重跑创建新attempt，不覆盖失败或重跑到绿掩盖不稳定。前置未满足与产品缺陷分开，不能删必需用例换放行。规范/设计变化确有授权时，先修改依据并输出候选/diff，经相关Owner明确评审后才更新基准；禁止为迁就实现自动重拍截图、放宽阈值或改期望。

## 5. 证据与版本管理

计划JSON、测试源码、授权fixtures及批准的产品截图基准进入同一版本库；当前目录尚无Git/remote，不在本轮初始化、提交或推送。生成的CSV/HTML是评审副本，不替代唯一登记和测试源码。报告QA源码在 `scripts/report/verify_test_case_report.cjs`；`.build`只放其输出和缓存。

每次产品run生成独立run_id/attempt、包/源码/资源/环境身份及逐组合结果，原始日志/画面写临时输出后复制到受控存储并校验hash。**长期证据不能只存在可清理的 `.build`。** 访问人员、保留期限、备份和清理策略在PRE-15冻结，本规划不擅自指定付费存储、上传私人文本或数值保留期。`reports/`只发布审核后的脱敏副本；游戏资产、完整台词、个人路径和凭据不默认提交。

供给登记通过只说明记录/映射完整，不说明账号、设备或资源已可用；实际准备复验回执与run绑定。当前18项就绪记录仍待确认/准备/评审，真实产品capture/runtime adapter未实现，原153组合仍全部not_run。

## 2026-10-19：补齐已评审设计的执行覆盖

新增[RW逐项逐层测试合同](reading-window-test-coverage.md)与[发布执行证据格式](release-execution-evidence.md)。34项RW从权威CSV闭包检查，按设计/真实产品组件/真实数据管线/真实语言/原生/游戏六层划分；缺层、漏状态、旧源码/包、零断言、无nonce、截图未关联、跨层冒充均应失败。设计adapter由`interaction-design-execution`实际启动，`interaction-coverage-contract`与`interaction-coverage-tests`校验登记及正反例；它们不属于产品运行adapter。原计划CSV不重写，153个case@variant执行证据单独验证。

设计层所需已有本机Chrome与固定Playwright；真实层所需正式包、已核对Core/词典资源、可用桌面/权限、多屏与快捷键冲突条件、真实回调和可推进对白的游戏，继续由原测试供给矩阵维护。新层adapter尚未实现，不能用设计样例补齐。测试源码归`scripts/quality/interaction_coverage.py`与`test_interaction_coverage.py`、设计动作归`scripts/report/verify_interaction_gap_design.cjs`、映射归`reading-window-test-coverage.json`，由QA主责、UI/AP/PL/HK/JA按层协作。每次执行写独立`.build/quality/`回执，失败保留；历史成功不追认新范围。
