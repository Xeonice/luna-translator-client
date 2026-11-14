# 当前 PRD 与产品依据

## 当前实施与准出入口（2026-10-31）

最新[guided阅读/配置稿](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading)作为本期实施目标，仍待对应设计评审；原批准图与历史回执保留。产品行为看模型PRD第8/10/11章及阅读PRD最新合同；前版实体Tab或平铺稿仅保留来源，不作为并行现行表单布局。

- [当前客户端对齐计划](../development/current-client-alignment-plan.md)：现状差距、CP任务、模块Owner、依赖、资源/存储边界与产品完成条件。
- [最新测试用例与准出](../development/current-product-test-coverage.md)及[机器登记](../development/current-product-test-coverage.json)：保留旧153，追加70来源、逐服务/协议/引擎/GUI/四游戏的适用观察；缺真实层继续blocked。
- [客户端技术设计](../architecture/reading-window-session-technical-design.md)与[模型技术稿](../architecture/japanese-study-and-llm.md)：正式源码与规划合同分开；HTML模拟接口不是当前产品能力。

以下按日期记录来源；其“最新”仅指当时版本，不覆盖本节统一入口。整个首版12项范围和原始产品快照不变。

2026-10-30最新阅读与配置候选：[当前阅读](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading)首屏显示独立原文/译文卡，历史/全部提取/Hook诊断后置默认折叠；[服务配置](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=settings)从用途、品牌与明确接口进入必要字段，学习与高级设置按需展开。见[配置第10章](model-configuration-prd.md#guided-service-configuration-prd)、[会话PRD](reading-window-session-prd.md#guided-current-reading-prd)、[25项呈现目录](../architecture/service-configuration-presentation.json)及[GD测试](../development/guided-service-reading-tests.md)。本轮仅文档/候选设计，`pending_review`，不改21/13/4目录、旧计划或产品`not_run`。

2026-10-30前版平铺来源：[完整候选](../design/first-product-v1/candidates/flat-service-session-20261030/index.html?scenario=reading)与[服务配置](../design/first-product-v1/candidates/flat-service-session-20261030/index.html?scenario=settings)同页展示统一服务管理与条件编辑、独立精读/查词区，以及会话正文历史/原始提取/Hook诊断区域。移除重复导航、对象不适用字段和产品表单内的演示控制，保留底层实体、版本、高级配置与全量审计。见[PRD第9章](model-configuration-prd.md#flat-service-management-prd)、[会话约束](reading-window-session-prd.md#flat-current-session-prd)、[技术呈现合同](../architecture/japanese-study-and-llm.md#flat-service-session-contract)、[FL测试与功能审计](../development/flat-service-session-tests.md)和[本轮OpenSpec](../../openspec/changes/design-flat-service-and-session/proposal.md)。新候选`pending_review`；本轮仅文档/设计，原13/21/4、PA/OT/SS、MC/RW/153计划和产品`not_run`保持。

2026-10-30前次日常翻译服务选择来源：用户已确定统一浅层选服务、另处配置、没有配好不能选。最新[完整候选](../design/first-product-v1/candidates/translation-service-selection-20261030/index.html?scenario=reading)由当前会话与HUD复用同一选择器；仅已添加/已保存且用途支持的合法服务可选，HTTP未测试与未配置、本机资源/运行route门禁分别表达。配置管理新增/编辑/保存/检查/样句不改current；明确选择激活后续句绑定且保持启停。见[PRD第8章](model-configuration-prd.md#translation-service-selection-prd)、[技术合同](../architecture/japanese-study-and-llm.md#translation-service-selection-contract)、[机器目录](../architecture/model-service-registry-design.json)、[SS逐层测试](../development/translation-service-selection-tests.md)及[本轮OpenSpec](../../openspec/changes/design-translation-service-selection/proposal.md)。新交互pending_review；R71本机候选、旧验收与原矩阵/not_run保留，本轮不改产品或DDL。

2026-10-30本机非大模型翻译修订：补齐系统API、独立离线worker及传统DLL候选；不是把Ollama当本机翻译引擎。见[模型配置PRD第7章](model-configuration-prd.md)、[完整整稿候选](../design/first-product-v1/candidates/local-offline-20261030/index.html?scenario=settings)、[技术主稿](../architecture/japanese-study-and-llm.md)、[逐层测试与依赖](../development/local-offline-translation-tests.md)及[本轮变更](../../openspec/changes/design-local-offline-translation/proposal.md)。新交互待评审；原13HTTP协议/21预设、原批准源/图与产品not_run保持，字段和运行器仍候选。

2026-10-30模型服务文档更新：用户确定本批全量13协议/21路由，明确移除腾讯TMT，保留TokenHub Chat/专用翻译。[模型配置PRD](model-configuration-prd.md)与[既有LLM技术主稿第5章](../architecture/japanese-study-and-llm.md#5-翻译引擎与模型服务设计)现为同一设计入口；[机器目录](../architecture/model-service-registry-design.json)绑定范围与计划状态，[变更记录](../../openspec/changes/specify-full-provider-registry-design/proposal.md)记录差异。新增协议/注册/凭据/迁移仍未实施，不覆盖当前运行包、旧设计确认或历史回归结果。


2026-10-21当前修复：用户明确要求本机封面优先，无图时只为正式关联的有效目标保存首张有效帧；首版仅连接已运行游戏，删除详情三重复入口及游戏启动入口；同面板核对后一次关联立即Hook、候选显式选正文；已有权限自动预览且会话跨页签常驻；Hook日志保留有界脱敏原生排障事实并默认降低raw噪声。见[本轮工作卡](../../openspec/changes/repair-library-binding-preview-diagnostics/iteration-card.md)。状态 `implementation_in_progress`，本轮新候选不覆盖原批准PNG/token，不继承09旧工程或局部Hook为产品全通过。

首版实施已于2026-10-19获用户授权；2026-10-21本轮五项修复按明确用户要求进入实施与对应测试。此前六项设计修订及门禁缺口分析作为来源保留，不能据旧文案排除本轮已授权工作。 当前规范按“首版范围 → 场景与配置详规 → 整稿”阅读，历史专题不再作为并行规范。

工程实现方式以[首版研发基线](../architecture/first-product-engineering-baseline.md)为统一入口，覆盖技术栈、模块Owner、组件、设计规范、复用及[目标目录](../architecture/engineering-layout.md)。本轮设计确认不改变技术栈与既有产品、已批准参考图、既有测试结果或正式发布状态。

| 文档 | 当前职责 |
| --- | --- |
| [首版范围](first-product-scope.md) | 首个可用产品版的 12 项功能、阶段边界与验收方向 |
| [游戏入库与窗口关联](game-library-window-prd.md) | 添加程序、选窗口、核对与一次确认关联并接入 |
| [运行窗口、字幕与会话](reading-window-session-prd.md) | 真实窗口预览、HUD 拖动与快捷键、自绘 Select、会话全量提取与 Hook 日志、分词注音和材质 |
| [AI 阅读与学习](ai-reading-prd.md) | 翻译、精读、语境查词的入口、输入和结果归属 |
| [模型配置](model-configuration-prd.md) | 配置管理与日常浅层选用；连接、模型、用途、版本、凭据及失败恢复 |
| [高保真整稿](../design/first-product-v1/index.html) | 上述首版场景的统一可交互设计 |

用户已确认首个可用产品版的[12项功能范围](first-product-scope.md)：游戏取文、翻译、内置词典分词、按需精读与模型配置。当前完整设计入口为[首版高保真稿](../design/first-product-v1/index.html)、[画廊](../design/first-product-v1/gallery.html)及[使用旅程](../design/first-product-v1/entry-flow.html)，以独立窗口和真实游戏场景串起首版流程；仅设计演示，原始产品快照保持不变。

[macOS 磨砂玻璃案例](../design/macos-material-case-v1/index.html)的风格已获用户确认，是整稿材质基准；本轮用户另已确认最新整稿修订；具体16源与37图绑定在设计确认记录。设计确认不替代产品运行回归。

新增产品草案：[AI 阅读与学习](ai-reading-prd.md)，补充情境翻译、句子精读、语境查词及后续对照/学习/历史场景。它是待评审的独立补充，不修改下列原始快照，也不表示所有新增场景已有实施任务或通过回归。

模型配置详规：[模型配置 PRD](model-configuration-prd.md) · [待执行验收矩阵](model-configuration-acceptance.csv)。补齐连接、模型、用途三层及保存/测试/应用/启用、参数、上下文、凭据和恢复行为。对应当前整稿的[设置窗口](../design/first-product-v1/index.html?scenario=settings)及[阅读配置往返](../design/first-product-v1/entry-flow.html)；[模型配置 v1](../design/archive/README.md)仅作历史专题保留。16条AI-CFG与22条MC仍为规划，不改变原56项需求或产品回归状态。

首次使用补充：[游戏入库与窗口关联PRD](game-library-window-prd.md) · [20条待执行验收](game-library-window-acceptance.csv)。参照原版游戏管理、选择进程/窗口，采用核对后“关联并接入”的单次确认流程，覆盖空库、程序添加、映射核对、正文选择、目标切换和解除关联；对应当前整稿的默认入口。网页交互检查不替代真实文件解析或CrossOver窗口映射。

2026-10-19 六项缺口修订：[运行窗口、字幕与会话 PRD](reading-window-session-prd.md) · [34条新增待执行验收](reading-window-session-acceptance.csv) · [候选设计入口](../design/first-product-v1/index.html?scenario=window-preview#window-preview)。它细化 FP-03/04/05/07/09/11/12，并补齐窗口帧与选流前提取事件的可观察范围。设计状态为 `approved_by_user_20261019`（见[确认记录](../development/design-approval-20261019.json)）；所有新增产品执行项均为 `not_run`。原97项来源矩阵、153个发布执行组合及历史实测不被重写，后续实施前需逐项补映射，不把已有相似标题视为覆盖。

原文件从用户指定的远端项目导入，内容保持逐字节一致：

- [PRD v1.2](original/docs/01-prd.md)：REQ-001～056。
- [设计规范](original/docs/02-design-spec.md)：SCR-01～22、DES-01～22。
- [原设计画廊](original/design/index.html)、[画面计划](original/design/screen-plan.json)、[资源清单](original/design/manifest.json)。
- [完整来源与 SHA-256](source-manifest.json)。

来源为 `100.97.190.45` 上的项目
`/Users/douglasdong/orca/workspaces/luna-translater-rust/项目迁移至-rust-并跨平台维护的可能性`。
这批原稿在远端尚未纳入 Git；清单中的仓库 HEAD 只提供环境上下文，不能当作文件的提交版本。
此次导入的逐文件哈希、时间和远端路径才是这份快照的准确身份。

共保留 100 个文件：2 份主文档、设计说明/metadata/HTML、89 张预览图和2份素材。
没有复制旧 Rust/Swift 源码、旧开发任务、旧 OpenSpec 或构建产物。

## 仍然约束新项目的内容

- 统一输入、可取消的进程/环境连接、候选文本流及显式保存。
- HOOK、独立实时/一次 OCR、剪贴板、手动和文件输入均保留在产品路线中。
- 翻译服务、LLM 上下文、术语、预处理、缓存及历史互相分层。
- 阅读 HUD、22 个页面/窗口、10 类单层设置、隐藏不暂停、不抢游戏焦点等视觉和交互语义。
- 国际化、凭据、诊断、固定依赖来源与可检查的跨平台能力状态。

原稿中的“已确认视觉稿”“待评审稿”和运行测试 `NOT_RUN` 分别保留。导入成功不提升任何页面的运行验收状态。
新后端的无视图边界及阶段安排见 [ADR-001](../architecture/ADR-001-headless-lunahook.md) 与
[新开发计划](../development/plan.md)。本阶段只完成产品基础，不删改原 PRD 的 P0/P1/P2。

