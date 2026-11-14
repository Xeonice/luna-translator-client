# 本机非大模型翻译：逐层测试与依赖

修订：2026-10-30 · 工作项：[design-local-offline-translation](../../openspec/changes/design-local-offline-translation/proposal.md)。本文件是新离线范围的计划，原MC/153组合与历史CSV不改。本轮设计模拟只验证页面交互；所有实际系统API、worker、DLL、资源、迁移和最终包用例仍`not_run`，缺外部条件时单独运行记录标`blocked`。

对应[PRD第7章](../product/model-configuration-prd.md)、[技术主稿5.13起](../architecture/japanese-study-and-llm.md)、[完整设置候选](../design/first-product-v1/candidates/local-offline-20261030/index.html?scenario=settings)。候选pending用户评审，不能继承旧截图批准。机器场景定义为model-service-registry-design.json的offline_acceptance。

## 用例计划

| ID | 功能 | 要证明的行为 | 逐层执行 | 产品状态 |
|---|---|---|---|---|
| OT-01 | 目录与后端分流 | 13HTTP协议/21preset身份不变；4离线候选引用3backend，DLL不伪装HTTP | 静态+设计+适配单测 | not_run |
| OT-02 | 本机条件表单 | 系统/专用MT/DLL只出现适用安装、资源、语言字段，不要求URL/Key/LLM模型 | 设计+真实组件 | not_run |
| OT-03 | 用途隔离 | 离线仅translation；grammar/context_word拒绝，修改绑定不隐式开启学习 | 设计+领域+存储 | not_run |
| OT-04 | 五动作分离 | 草稿、探测、指定样句、应用、启用分别有版本与结果；探测不等于译文成功 | 设计+领域+正式包 | not_run |
| OT-05 | 系统版本与包授权 | Apple传统策略/OS/SDK按能力验证；语言下载授权独立；取消能力不伪造 | 平台API+正式包 | not_run |
| OT-06 | 安装与资源边界 | 缺失/损坏/版本不兼容/目录变化/架构错误有恢复；内容hash而非仅路径识别 | 设计+资源+worker | not_run |
| OT-07 | 直达与显式中转 | 无日中直达包时不得静默英语中转，明确语言链路/质量提示/同意和缓存身份 | 设计+适配+真实语言 | not_run |
| OT-08 | DLL隔离与容器 | J北京/金山各自同位数worker；Windows与CrossOver独立，不进入游戏/Hook/UI进程 | 原生+进程+正式包 | not_run |
| OT-09 | worker故障与预算 | 超时/崩溃/EOF/协议畸形/大输出/进程退出/重启退避，其他翻译与词法继续 | 进程+故障注入+负载 | not_run |
| OT-10 | 取消与迟到 | 操作ID/配置代次/资源版本/样句身份匹配；旧进程和旧测试迟到不覆盖当前 | 设计+领域+真实时序 | not_run |
| OT-11 | 断网与不回退云端 | 断网首启/已安装语言包真实日中翻译；无HTTP请求，不自动改付费或LLM后端 | 断网+正式包+出站观测 | not_run |
| OT-12 | 资源与缓存 | 同文不同引擎/资源hash/语言链路/配置/人工修订不误命中，来源可追溯 | 领域+数据库 | not_run |
| OT-13 | 类型与历史迁移 | 新tagged target与HTTP model引用XOR；FK/空值/边界/旧codec seal只读兼容 | 字段+真实SQLite+迁移 | not_run |
| OT-14 | 导入与导出 | 导出不带本机绝对目录、凭据、安装授权、测试回执；导入重绑定草稿且不启用 | 设计+组件+存储 | not_run |
| OT-15 | 多引擎与游戏覆盖 | 全局/游戏覆盖、并行翻译、停用/删除影响明确；历史保留原引擎与版本 | 设计+领域+正式包 | not_run |
| OT-16 | 最终语言与许可回归 | 同一签名包SR/MO/Never7/樱之诗；真实语句质量、许可与资源分发、冷/热启动 | 正式包+四游戏+平台 | not_run |

## 测试前必须具备的输入

| 输入与负责人 | 用于哪些验证 | 不具备时 |
|---|---|---|
| 当前PRD/目录/候选设计、固定Chrome与共享材质；UI/QA | 设计旅程、条件表单、Select键盘、200%/窄窗、过期/取消 | 文档/设计检查可独立执行，不能替代产品 |
| 支持系统Translation的Mac、对应SDK、语言包下载同意与离线包；PL | OT05/11，15/26/26.4符号能力按实际版本核对；传统策略可证明 | 仅该平台项blocked，不假定所有macOS可用 |
| Argos/CTranslate2完整pipeline PoC、tokenizer/分句/包schema与明确生产运行栈；AI/PL | OT06/07/09/11/12；日中直达与中转分别验证 | 不引入未批准Python，不把加载权重当翻译完成；实现not_run |
| 用户合法安装J北京/金山、原始目录与DLL/字典版本、Windows与CrossOver；PL/QA | OT06/08/09，CPU架构与系统分别建记录 | 不下载/分发商业组件；软件或授权缺失写blocked |
| 经独审的新DDL/codec、冻结旧profile副本、崩溃/磁盘满注入环境；DB/QA | OT12/13/14真实迁移与事务回滚 | 新字段仅候选，不以旧存储adapter通过代替 |
| 可发送/可离线使用的固定日文与独立中文预期、长句/标点/Unicode/空输入/人名；JA/QA | OT07/11/16输出质量/术语与低资源适用性 | 人工样例不得伪称真实引擎输出；先记录来源和授权 |
| 同源签名正式包、Core/语言资源清单、四款游戏可操作与可见桌面；PL/QA | OT15/16同包端到端与正常退出 | 不拼接旧包/探针；不测Ever17 |

## 源码、执行与结果维护位置

- 规划：本文件与原机器目录offline_acceptance；原始PRD/MC/153计划保留身份。
- 本轮设计静态守卫：scripts/quality/offline_translation_design.py、test_offline_translation_design.py；浏览器旅程：scripts/report/verify_local_offline_design.cjs。从统一verify真实执行，输出独立run目录与命令/source hash/断言/截图。
- 后续生产测试建议路径（尚未创建/实施）：crates/app-core/tests/offline_engine_configuration.rs（用途和版本）、crates/app-infra/tests/offline_translation.rs（适配和进程）、crates/app-infra/tests/storage_offline_translation.rs（字段/迁移）；平台API/worker协议及许可需先完成技术与目录评审。
- UI真实组件维护在apps/desktop/tests的对应model-config套件；复用正式应用端口，不重写一套假业务层。
- 正式包/四游戏记录按client-regression.md逐run维护于.build/client-regression；设计记录留.build/offline-translation-design-20261030；计划永不填写运行pass。
- 缺断网/原生/字段/资源/真实输出执行adapter时保持not_run或blocked；设计guard和截图通过只属于design层。

## 自行选择的设计验证场景

从主窗设置真实入口选择本机离线翻译；逐引擎切换并检查无LLM字段、语言/资源条件状态；依次保存、探测、样句、应用后仍关闭、显式启用；改配置令测试过期；语言包缺失/版本不支持/超时可恢复；取消或换样句后旧回执不能覆盖；无直达包须显式中转同意；学习用途不得选择离线引擎；导出脱敏并导入仅草稿；返回旧HTTP/大模型配置与阅读主窗可用。Playwright另在1440与较窄视口截图，参照旧同材质整稿，不刷新批准图或放宽阈值。
