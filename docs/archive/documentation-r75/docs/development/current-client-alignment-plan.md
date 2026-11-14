# 当前客户端对齐计划与准出基线 · 2026-10-31

本计划以 R74 最新[阅读页](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading)与[配置页](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=settings)作为实施目标。设计仍为 `pending_review`；本轮用户授权更新规划、PRD、测试及准出规则，不代表正式客户端已对齐、发布获批或新增字段/技术路线已获批准。

当前研发仍按[稳定性重构](stability-refactor-plan.md)与 current-phase 的优先级执行。下表是后续产品工作项的依赖计划，不能用这份文档直接获得全目录修改权限；每片实施另建 product change、精确路径和实际执行记录。原需求、Core/License、协议身份、历史回归和旧128定义/153组合保留。

## 1. 当前唯一目标与规则优先级

| 对象 | 最新依据 | 使用规则 |
|---|---|---|
| 日常选用 | 模型PRD第8章、SS与最新guided稿 | 当前会话/HUD共享浅层选择；仅已保存且符合资格的服务可选，保存/测试不切服务、不改启停 |
| 配置呈现 | 模型PRD第10章及逐字段呈现目录 | 用途→品牌→明确接口→必要字段；第9章实体平铺只是前版呈现，不能据此把技术字段全部展开 |
| 阅读 | 当前阅读PRD最新合同、guided当前句与后置details | 当前句独立，历史/原始/Hook默认折叠但全集和各区状态不丢；主窗与HUD须共享真实词法和固定学习对象 |
| 实施技术 | 研发基线、阅读技术稿、LLM主稿及本计划 | 目标接口与已有源码分别标注；HTML里的模拟API不是产品端口 |
| 测试与准出 | current-product-test-coverage.json/md及既有153/RW/存储门禁 | 旧集合与当前附加闭包同时强制，任一缺项都不准出；不能只依赖设计守卫 |

最新设计、规范和可执行登记不一致时登记冲突并修正，不以当前实现倒改设计。原已确认PNG/token不覆盖；本轮新的布局/接口仍需对应确认与产品实测。

## 2. 源码现状与目标差异

本表是本轮只读源码核对，不是新产品回归结果；详情和源定位见[阅读技术设计](../architecture/reading-window-session-technical-design.md)、[模型技术稿](../architecture/japanese-study-and-llm.md)、[客户端入口](desktop-client.md)。

| 领域 | 已有基础 | 本次尚需实施的差距 | 验收来源 |
|---|---|---|---|
| 当前阅读 | ReadingPanel的SessionOverview、SourceText、词卡与学习对象 | 概览还在正文Tab；普通p必须改复用SourceText。独立当前unit/revision卡、复制/重译/分词注音不能随折叠丢失 | GD01/02/12、GUI04、RW26～30 |
| 三数据集 | SessionRecordPanel、sessionRecords与分页/导出、Rust持久管线 | 互斥Tab条件卸载仍在；需三独立状态默认折叠、计数/警告与展开恢复，不能截断数据或重启预览 | GD02/03、RW17～23 |
| 服务注册 | Rust已存在OpenAI兼容、Anthropic、Azure分支 | 13协议/21HTTP预设/4离线的正式注册、字段投影和适配仍不完整；25项设计不是25个运行服务 | PA/OT、GD04～06/10/11 |
| 配置与选用 | 整体草稿save/apply、ModelConfigScreen六实体Tab | 缺已保存服务目录、资格原因、细粒度activate端口、共享浅层picker及真正必要字段表单 | SS、GD05/09/11 |
| 凭据与迁移 | Vault和单secret引用已有 | typed多字段/服务账号/OAuth、独立离线资源与无model目标仍待合同/codec/迁移评审；不把字段塞任意JSON | OT、GD07、GUI06 |
| 高级与学习 | 既有用途温度/模板/上下文/策略及学习流 | 按实际绑定和协议呈现；新服务编辑不改旧用途，观察元数据不洗掉已知失败，失效测试须结算 | GUI01/02/03/05/06/07/08 |
| 发布证据 | 已有组件/存储/SR6有限adapter和旧153校验 | 最新增量需按真实层逐项注册执行；SR6/旧包/设计截图不能代替完整新范围 | 当前准出闭包＋旧153/RW/存储 |

## 3. 任务、职责、复用与依赖

所有任务当前为 `planned / not_run`。Owner缩写和模块职责来自[模块权威表](../architecture/engineering-modules.csv)，不把M11偏好误称日志或历史。

| 任务 | 后续改动与职责 | Owner / 模块 / 复用 | 前置 | 完成条件 |
|---|---|---|---|---|
| CP-00 | 冻结产品目标、按页面/状态补独立参照与真实用例映射；清理过时入口说明 | QA/UI；M13/M15；保留原图与token | 本规划、当前设计评审 | 明确确认范围，逐项覆盖且无来源漂移 |
| CP-01 | 当前句独立卡、三数据集后置details、状态保存、轻量工具栏 | UI/AP；M03/M10/M13；复用SourceText、SessionRecordPanel和SessionPreviewCoordinator | 现有DTO可承载；CP-00设计目标 | 分词注音/词卡/旧句学习不丢，折叠不清页/筛选/跟随或停采集/重建预览 |
| CP-02 | Protocol/Profile/Preset注册、不可变版本、能力/字段/资格投影合同 | AP/AI；M04/M05/M06；复用已实现3协议边界 | 精确13/21/4与字段目录审阅 | Rust为权威，UI不手抄支持表；传统/LLM/离线目标明确，不虚造model/HTTP字段 |
| CP-03 | typed秘密、OAuth与离线资源/目标的DTO、codec、迁移及导入导出方案 | PL/AP；M16/M04/M10；复用Vault/存储迁移框架 | CP-02合同；字段/关系另审 | 旧profile/历史sealed revision可读，秘密只引用，回滚/坏文件/撤销有真实测试；新DDL不获本计划默许 |
| CP-04 | 迁移已有3协议并补其余适配；21预设精确身份、参数与真实请求 | AI；M06/M05；复用窄HTTP port和注册层 | CP-02/03 | 每协议contract与真实服务分别验证，TokenHub双接口独立，TMT仍排除，不隐式fallback |
| CP-05 | 四本机非LLM路线、系统/资源/许可/位数/语言健康与worker隔离 | PL/AI；M06/M12/M16；复用独立worker边界 | CP-02/03与实际平台资源/技术锁评审 | 未知阻断、断网可证、无自动云回退，探测不冒充翻译；不悄悄引入未批准桥接框架 |
| CP-06 | 已保存资源列表/资格原因、BindingCommand激活、范围继承/取消迟到 | AP；M04/M05；细化ApplicationPort与窄Tauri DTO | CP-02/03；所选服务CP-04/05 | 选用只影响后续句且保持启停，save/test不选用，重译为独立显式动作，旧结果不覆盖 |
| CP-07 | 引导配置与共享浅层picker、高级/用途/学习分层 | UI/AP；M13/M04；共享Select/字段控件/端口 | CP-02/03/06、CP-00 | 只配置必要项，固定地址预设维护，custom真正必需项不隐藏，模板/代理/用途覆盖均可达 |
| CP-08 | 逐项真实组件、领域/存储、各服务与资源的执行adapter | QA/AP/AI/PL；M15及各模块；复用现有guard/证据规则 | 各片实现 | 70来源与25服务/13协议/4引擎/GUI8逐层真实覆盖，无设计/fixture替代 |
| CP-09 | 当前macOS arm64+CrossOver同一最终包四游戏完整回归 | QA/PL/HK/JA；M02/M03/M07/M09/M10/M12/M13/M15 | CP-01及对应CP-04～08、账号/资源/桌面 | SR/MO/Never7/樱之诗各5不同状态；N7跨3页；实际原译文、学习、预览、退出/恢复及秘密边界同包核对 |
| CP-10 | 准出评估、缺口清零及正式发布审核 | QA；M15；复用原153/RW/存储与新sidecar | CP-00～09适用项 | 所有适用集合、准确源码/包/资源/nonce与审批齐全；任何缺项保持blocked |

CP-01可以在现有DTO边界独立先实施；CP-02～06先审核心合同，不让UI先造一套业务。CP-07不能绕过typed凭据/资格权威，CP-08随各片补，不等最后一次补测。每片另记录确切文件范围、Owner、source/package身份和失败重跑，不直接把本表勾成已实现。

## 4. 数据、模块与平台边界

- M03/AP持当前阅读与raw输入权威；M04/AP配置及绑定；M05/AI翻译调度；M06/AI协议/服务；M10/AP三数据集存储与分页；M11/AP偏好；M13/UI组件；M15/QA诊断规范/准出；M16/PL安全凭据。真实模块Owner以权威表为准，跨模块必须声明协作方。
- 阅读布局迁移优先复用已有当前unit、raw/Hook DTO与查询端口，不因折叠新增数据库。注册、typed凭据、OfflineEngineConfig/EngineTarget及旧revision升级需先核schema/codec和迁移：本轮没有批准任何新增字段类型、FK或DDL，也未改存储验收结论。
- 正式UI从Rust投影获得必要字段与资格，不把HTML中的全局模拟状态/JSON自动升级为生产协议插件。端点、认证/签名、参数支持和取消仍在适配层；UI不直接HTTP/SQL/FFI/Keychain。
- 当前发布对象固定为macOS arm64客户端＋CrossOver游戏。Windows原机资格单独保留为4项未执行计划，与Mac包hash分开，不因本轮规划声称已有Windows客户端分发。服务/引擎平台不可用时精确blocked；不能把某个游戏成功推广为四游戏或全平台。

## 5. PRD、测试和准出交付

本轮已把[模型PRD实施对照](../product/model-configuration-prd.md#client-configuration-implementation-plan)与[阅读PRD实施对照](../product/reading-window-session-prd.md#client-reading-implementation-alignment)连接到上述任务；技术细节由对应架构页维护，不让PRD暴露内部操作。最新[测试覆盖与准出](current-product-test-coverage.md)和[机器登记](current-product-test-coverage.json)给出逐项层/变体/Owner/前置/证据。

原128定义、153执行组合、97来源、原111回归CSV和已有存储/RW门禁全部保持；最新70来源的附加准出闭包独立校验，不能替代旧集合。25服务精确指21 HTTP预设＋4离线引擎，HTTP协议为13种。8项GUI问题从真实已发现缺陷逐一登记；设计919断言与原型图仍仅是设计结果。

准出依据不是“所有工程检查绿色”。开发态合同通过，只表示计划与规则一致；`--assess`在当前真实adapter/同包执行/设计审批缺项时必须返回`blocked`。release路径无条件检查最新sidecar，旧153只通过或旧包实测不能放行新目标。已有SR6 runner只覆盖其实际稳定性范围，不能据有一个已注册runner声称153和最新闭包均执行。

## 6. 维护与执行记录

- 本计划、PRD与技术文档描述目标/已知差异；source-audit只读证据不能替代产品运行。`implementation-status.json`旧审计状态不由本规划批量改绿。
- 测试计划由QA维护current-product-test-coverage.json/md；执行工具归scripts/quality/current_product_coverage.py及tests；UI/AI/AP/PL/HK/JA按真实层提供adapter、账号/资源/可见桌面与输入。
- 本轮实际检查和HTML报告另存`.build/current-alignment-20261031`及reports新目录；原计划保持not_run。每次产品实测单建run_id，失败留证、外部条件缺失写blocked，最终准出只认准确同源同包证据。
- 后续改PRD/设计/合同需更新追溯及程序规则并独立review；不重拍批准图、不提高阈值、不刷新旧债务hash来过门禁。本轮不提交或推送。
