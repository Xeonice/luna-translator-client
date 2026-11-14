# 稳定性重构执行计划

日期2026-10-26。当前优先级已按用户指令调整：**先重构与相关文档，消除本批已证交叉影响并完成稳定性回归，再补剩余产品内容。** 不缩减首版PRD，不将内部分拆当功能完成。当前SR0只定义计划与合同；SR1～6尚未实施/执行。

机器登记：[当前阶段](current-phase.json) · [阶段依赖](stability-refactor-plan.json) · [影响回归矩阵](refactor-impact-matrix.json)。技术合同见[状态权威与生命周期边界](../architecture/runtime-lifecycle-refactor.md)。上轮Review及目录报告保持原版，不继承为后续源码/包的运行通过。

## 1. 执行顺序

文档、表征测试、实现迁移、独立审阅和分层回归随每个切片一起推进，不能先抽完所有文件再补测试。母change `refactor-runtime-lifecycle-boundaries` 是tooling，仅授权SR0文档/入口；后续业务路径先建窄product子change及完整前后源码，禁止一次授权全目录。

| 阶段 | 内容 | 主责/独审 | 前置 | 当前定义 |
|---|---|---|---|---|
| SR-0 | 合同与迁移基线 | Root / AP + PL + QA | 无 | 文档定义 |
| SR-1 | 首切片：预览纯状态政策提取 | PL；AP独审；QA验证 | SR-0 | 尚未实施 |
| SR-2 | 消费端传输与展示分离 | UI；PL/AP独审；QA验证 | SR-1 | 尚未实施 |
| SR-3 | 主进程会话预览与封面职责拆分 | PL；AP/HK独审；QA验证 | SR-1, SR-2 | 尚未实施 |
| SR-4 | Hook facade 内部窗口关联与 journal 接口提取 | HK；AP/PL独审；QA验证 | SR-3 | 尚未实施 |
| SR-5 | 几何意图与词卡所有权结构化 | PL；UI/AP独审；QA验证 | SR-0 | 尚未实施 |
| SR-6 | 同源签名RC的稳定性关口 | QA主责；AP/HK/PL/UI/JA协作 | SR-2, SR-3, SR-4, SR-5 | 尚未实施 |

SR5只有在合同先冻结且精确文件Owner不冲突时可与SR2并行；SR3集成后仍须联合回归。SR1只提取WindowPreviewService私有纯政策，不同时动UI/Native/worker/Hook/数据库。几何若已符合单权威，可只冻结合同/补证据，不为拆文件而拆。

## 2. 每片工作卡必须固定

- 用户行为、公开DTO/IPC、Core ABI1/JSONL2、DB schema、资源锁、预算和材质保持；确需演进须单列影响后实施。
- 固定M/C、Owner、实际与拟新增路径、独审、子change开工前完整源码归档和精确diff。
- 每轴事实writer、只读消费者、失效原因、取消对象及真实释放ack明确；worker帧seq与service投影seq不混用。
- 先在生产共用路径执行表征测试再迁移。发现既存行为与合同矛盾时保留失败开bugfix，不改期望、容量、超时或fixture使其变绿。
- 聚焦命令仅开发反馈，不能冒充完整注册check；原100组件义务保持。源码、组件、Native、游戏/服务分层；计划CSV不回填实际运行。

## 3. 影响回归义务

9类义务覆盖7条已证事件链。现有检查与拟新增设施分别登记；此表是计划，不表示这些组合已执行。

| 义务ID | 应保持的行为 | 已有注册检查入口 |
|---|---|---|
| SRF-PV-IDENTITY | 同一lease/generation/consumer/source下，未知身份只能按有效首帧与projection一致证明单向解析一次；已知身份变化、解析后迟到null、旧代次和权限撤销均拒绝。 | product-native-ipc-tests, product-reading-component-tests, reading-component-guard-tests |
| SRF-PV-CONTEXT | A的绑定读取永不settle时，B必须独立获得绑定/预览；旧A结果、错误、finally和取消不能清B资源。 | product-reading-component-tests, reading-component-guard-tests |
| SRF-PV-FRAME-TRUTH | metadata无新图不能推进capturedAt或将stale升级Live；同blob也不等于新捕获。新有效frame才可恢复，权限拒绝后迟到高序号帧仍拒绝。 | product-native-ipc-tests, product-reading-component-tests |
| SRF-PV-ISOLATION | 挂卸载/切页、慢消费者、listener抛错和一个订阅取消不拥有producer；真正退出/目标失效才触发有界排空与资源释放。 | product-native-ipc-tests, product-reading-component-tests |
| SRF-DISCOVERY | 查询/DPI、Windows引用、Native选择和Hook接入分别表达；非法矩形/不完整身份继续拒绝，但合法负坐标可发现；process-only不得猜测WindowRef。 | product-target-probe-tests, product-hook-client-tests, product-infra-tests, product-window-title-proof-api-tests |
| SRF-WORDCARD-GEOMETRY | 新句/ruby/流式译文/内容适高不替换或关闭冻结词卡；真实用户geometry/Esc/owner scroll/外点按设计关闭精确当前代次，单纯blur不关闭。 | product-native-ipc-tests, product-reading-component-tests, reading-component-guard-tests |
| SRF-SESSION-DRAIN | 自然退出按expected SID+完整target完成barrier与FIFO尾事件；旧A不能stop/seal新B。真实Host/relay/lease释放确认之前不能宣称资源已释放。 | product-infra-tests, product-hook-client-tests |
| SRF-READING-VERSION | 重构不改变当前V1/V2/V3处理revision、聚合时序边界和UTF8引用上限；一个原文版本的token/译文/历史只能归属对应readingUnit revision，旧结果不得覆盖最新对象。 | product-core-tests, product-infra-tests, product-reading-component-tests |
| SRF-WRITER-BOUNDARY | 窄端口/内部coordinator拆分不破坏单writer事务、128队列政策、queue_full和失败恢复；尚无证据把队列延迟认定为近期卡死根因。 | product-infra-tests |

五类负例须有完整门禁实际回执；`five-scenario-execution`由现行verify自动运行，不是可随意填进requirements.checks的注册ID。真实Windows发现/DPI fixture、状态合同trace、M/C→事件→必跑场景守卫、正式包稳定性runner都仍待实施/注册，不能只写JSON或命令名就称已实现。

现有storage条件检查保持，以实际`storage_requirement_gaps`及注册adapter为准；触发生产存储/模块规则时必须实跑完整adapter。不能把架构M14（native-broker）重新解释成数据库模块来迎合工具旧映射。单writer排队是待观测风险，没有证据不擅拆库、扩容或宣称死锁。

## 4. 相关验收前置不能绕过

现有`apps/desktop`路径变更触发`product-ui-capture-v4`，SR1即便只改Rust也受该规则约束；SR4即便只改infra，关联/预览/封面受影响也保留视觉检查。原app.js来源指纹冲突必须单独审阅参照版本关系；不删capture、不刷新批准SHA/PNG、不放宽阈值。

缺此条件时可继续不依赖它的源码开发/聚焦验证，但不能关闭相关完整切片或稳定性阶段。新增ANSI/补全窗口旅程的整稿评审也不能被内部重构自动批准。

桌面、录屏权限、真实游戏及Steam、本机/内网模型和必要人工系统选窗/推进均是真实层前置。缺失写blocked，可继续纯规则工作，不以刷新/重选/重启冒充自然恢复。保留另一台Steam会话，不改变游戏几何。

## 5. SR6稳定性退出条件

1. 适用源码、编译、边界、五类负例、所有原维护断言及新增合同负例真实通过；相关视觉来源与实际PNG检查闭合。
2. 最小正式包稳定性runner真实实现并独立登记，绑定同一最终签名RC的源码、Core/语言、安装/运行对象、nonce和原生/游戏证据；允许可读人工step barrier，不允许手填pass替代观察。
3. 同RC串联冷发现→关联即Hook→选流→连续文字/真实变化帧→分词注音/冻结词卡→退出换游戏→真实睡眠唤醒。四游戏为SR、MO、Never7、サクラノ詩10th，不测Ever17。SR中文版不充当日语学习证据。
4. 主窗关闭/重开/切页、慢消费者、单订阅取消和封面失败不停止共享producer；旧SID/epoch/frame不污染新会话；内容适高不关闭词卡，真实用户geometry按原设计处理。
5. 关闭本批已证交叉影响缺陷，并有对应层重跑。未知原因、仅探针、仅编译或环境blocked均不等于完成；不承诺所有未来bug为零。
6. 此门槛通过仅允许恢复剩余产品开发，不等于首版发布。上线仍需原153 case@variant、RW91真实层、完整功能/模型与全部发布要求闭合。

## 6. 退出后再补的内容

精读设置purpose/game-scope闭环、参数/自定义键支持矩阵、显式fallback、仅进程补全窗口的产品旅程、整稿新增状态及其他内容继续保留PRD，退出后另开工作项。与重构无关的UI改版/通用内容清理不混入。

恢复既定行为所必需的安全、数据完整性或阻断缺陷可以优先修复，但须单列bugfix及影响回归，不借此扩产品能力。

## 7. 当前证据口径

SR0工程检查只证明文档和范围有效，不证明业务重构、正式包或新增adapter通过。计划中的not_run保留定义，实际结果在.build/reports独立回执；不覆盖原失败与旧源码/包事实。
