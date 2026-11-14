# 日语学习与大模型能力补充方案

## 2026-10-31 服务配置源码差距与实施依赖

当前源码核对范围为冻结 `FP1-20261030-R74` / 2087源；目标 [guided-service-reading-20261030](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=settings)仍 `pending_review`。任务、需求与新增准出闭包见[当前客户端对齐总计划](../development/current-client-alignment-plan.md)。下文5.4、5.13～5.23所称注册、typed凭据、离线配置和BindingCommand均为目标合同，不能据接口示例、设计JSON或HTML推定已实现。

| 目标合同 | R74实际实现 / 精确入口 | 差距与Owner |
|---|---|---|
| 25项必要字段与目录驱动 | [ConnectionEditor.tsx](../../apps/desktop/src/features/model-config/ConnectionEditor.tsx) `ConnectionEditor:13-20`手写OpenAI兼容/Anthropic/Azure及通用endpoint/auth；[ModelConfigScreen.tsx](../../apps/desktop/src/features/model-config/ModelConfigScreen.tsx) `Tab:19`、`ModelConfigScreen:22`与`:67`仍为六实体Tab | **未实现新引导**。M04/AP定义typed必要字段/资格，M06/AI产同源目录，M13/UI按用途→品牌→明确接口显示；25为21 HTTP预设+4离线候选，不是25个已安装服务 |
| 三层注册与13协议/21预设 | [providers/mod.rs](../../crates/app-infra/src/providers/mod.rs) `execute:124`、路由`:177`；[领域校验](../../crates/app-core/src/domain/model_config/mod.rs) `validate_protocol_purpose:8`、协议白名单`:194-202` | **部分**：已有3集中协议与网关，尚无三层注册。M06/AI实现精确tuple及硬能力，M04/AP复用目录校验；不得让厂商名/用户声明突破硬限制 |
| typed秘密与授权 | [CredentialInput](../../crates/app-core/src/contracts/desktop.rs) `CredentialInput:233`仅label/secret；[gateway.rs](../../crates/app-infra/src/providers/gateway.rs) `HttpModelGateway::key:16`和 `execute:33-38`把Vault字节作为单UTF8 key；[Vault](../../crates/app-infra/src/credentials/mod.rs) `SystemVault:7`、`Secret:55` | **部分**：不可变系统引用/单Key/清理已存在，SystemVault目前只有macOS实现，非macOS返回不可用（`:42`）；多字段envelope、schema版本、OAuth grant/epoch/刷新尚未实现。M16/PL主责秘密，M04/AP持非秘密关系，M06只取本次短期材料 |
| 资源与用途隔离 | [DTO](../../crates/app-core/src/contracts/mod.rs) `Connection:338`、`Model:353`、`Engine:366`、`PurposeBinding:375`；[ModelEditor](../../apps/desktop/src/features/model-config/ModelEditor.tsx) `ModelEditor:8`、传统翻译不需模型ID`:15`；领域已有Azure只限translation校验 | **部分**：连接/模型/用途独立、无虚构Azure模型ID已有；独立OfflineEngineConfig、EngineTarget联合、ResourceSnapshot/route未实现。M04/AP、M06/AI、M10/AP、M12/PL分别处理配置/执行/关系/资源观察 |
| 已保存服务与细粒度选用 | [Command](../../crates/app-core/src/contracts/mod.rs) `SaveConfiguration:96`、`ApplyConfiguration:100`；[配置存储](../../crates/app-infra/src/storage/queries/configuration.rs) `save:2`、`apply:494`；[ApplicationPort](../../apps/desktop/src/bridge/application-port.ts) `ApplicationPort:4` | **未实现新命令**。已有整体草稿/应用不可冒称BindingCommand；M04/AP增加只读可选性/原因与引用保存版本的原子激活，M05/AI接取消与后续句，M13共享HUD/当前会话控件 |

所有状态仅是源码核对；本轮没有执行新组件、Rust、持久化、真实认证/服务/离线系统、原生或同包四游戏产品验收，均 `not_run`。外部前置在真实开跑时不足记 `blocked`；原有局部成功、具体失败和旧包身份不互相覆盖。

### 先合同、后迁移、再接真实执行

- **可独立复用的呈现任务**：M13当前卡、三记录区默认折叠、已有高级区/用途区的可达性可以先沿现有DTO及共享Select实现，不默认新增DDL。当前卡与词卡/学习须固定M03的unit/revision；记录全量归M10；M11是偏好，M15才是脱敏诊断/验收。配置新引导在缺Rust目录/资格之前只能保持明确的设计目标，不能用静态JSON成为生产权威。
- **HTTP注册与typed凭据链**：PRD字段/用途→M06注册及catalog纯合同→M04 DTO/保存资格→M16 typed秘密/授权→M10 codec/迁移审阅→13协议/21预设fixture与真实服务→M13表单/选择器。沿原ModelGateway、RequestPlan与统一调度，不另建JS网络或测试假成功路径。
- **独立离线链**：4候选的运行路线、最小平台API/worker边界、精确依赖/许可与真实资源先审，再确定ExecutionBinding/EngineTarget、资源快照、旧codec/seal、FK/XOR/GC/回滚。未知backend/版本允许按合同读历史但拒绝执行；不为离线虚造Connection、endpoint、Key或actualModelId，不重算旧revision seal。
- **选用与保存隔离**：已有保存/测试/应用语义继续保留；新BindingCommand引用保存版本、校验scope/profile generation/expected binding revision并保持启停，只影响后续句。当前句重译独立。保存共享资源不重解释旧绑定；精读/查词保留独立用途与固定对象，不自动借用翻译绑定。
- **准出**：既有provider fixture、真实SQLite adapter、组件与有限原生adapter继续复用，各自只证明登记范围。新用途/认证、离线资源/OS、服务真实响应、新UI与原生/最终包须有适用真实adapter及同工件执行；缺adapter、缺Key/资源和已执行失败分别记录，静态目录/守卫/Playwright原型不替代这些层。

2026-10-30必要配置与阅读优先：最新[完整候选](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading)和[配置候选](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=settings)状态`pending_review`；5.23补充面向用户的条件字段投影与当前阅读优先，复用已有配置/资格/会话权威。新增[呈现目录](service-configuration-presentation.json)为设计数据，不是生产协议或schema实现，不改产品/DDL或历史执行。

2026-10-30同页呈现增量：最新[配置候选](../design/first-product-v1/candidates/flat-service-session-20261030/index.html?scenario=settings)与[会话候选](../design/first-product-v1/candidates/flat-service-session-20261030/index.html?scenario=reading)状态`pending_review`；新增5.22仅约束配置与会话投影的展示/独立状态，不改下文服务目录、业务合同、进程边界或持久化。FL-01～10新增产品计划全为`not_run`，此前候选和历史证据保持。

版本：契约详解 1.6 · 更新：2026-10-30 · 保留13个HTTP协议／21接口预设、腾讯TMT／TC3排除与5.13～5.18本机候选；5.19～5.21补充配置管理/日常选用的独立决策模型，5.22补充同页呈现。前次[服务选择候选](../design/first-product-v1/candidates/translation-service-selection-20261030/index.html?scenario=reading)pending_review；仅文档、候选稿和测试规划，不安装引擎、不引入生产Python/新框架、不改产品/DDL/旧seal，新增产品执行not_run。总体选型与职责仍以[首版研发基线](first-product-engineering-baseline.md)为准。

当前设计入口为[首版高保真整稿](../design/first-product-v1/index.html)、[字幕与精读旅程](../design/first-product-v1/entry-flow.html)和[统一视觉规范](../design/first-product-v1/style-guide.md)。旧字幕专题已进入[历史归档](../design/archive/README.md)，不再作为当前材质或交互规范。原 100 文件、56 项需求及 22 屏保持原样；设计样例不代表真实分词、词典或模型已接入。原2026-10-18章节属于设计要求；当前客户端已有实现及局部回执，不能沿旧日期把它们写成尚未研发，也不能把旧回执外推为本轮新增能力通过。


### 修订与状态追踪

| 日期 / 修订 | 变更 | 状态与追踪 |
|---|---|---|
| 2026-10-31 / 1.7 | R74源码/目标差距、正确模块Owner、复用及typed凭据/离线迁移依赖 | [当前总计划](../development/current-client-alignment-plan.md)；仅规划，新产品验收not_run |
| 2026-10-18 / 1.1 | 词法与LLM设计初稿 | 原设计来源保留；原句、词法和学习的既定约束继续有效 |
| 2026-10-30 / 1.2 | 全量模型服务注册、13协议/21预设、认证与候选持久化设计；排除TMT | [原变更记录](../../openspec/changes/specify-full-provider-registry-design/proposal.md)；[机器目录](model-service-registry-design.json)；新增验收全为not_run |
| 2026-10-30 / 1.3 | 独立执行backend、四个本机非大模型翻译候选、资源/worker/迁移与OT规划 | [本轮变更](../../openspec/changes/design-local-offline-translation/proposal.md)；HTTP目录不变；新增技术/字段/UI待评审，产品not_run |
| 2026-10-30 / 1.4 | 配置管理与日常选择分离、已保存服务投影、显式绑定激活/范围/取消与SS规划 | [本轮变更](../../openspec/changes/design-translation-service-selection/proposal.md)；旧13/21/4目录与PA/OT保持；新交互pending_review，产品not_run |
| 2026-10-30 / 1.5 | 统一服务管理同页编辑、条件字段和高级区；会话三独立区域与功能审计，FL规划 | [本轮变更](../../openspec/changes/design-flat-service-and-session/proposal.md)；不改底层业务合同/DDL/旧目录与PA/OT/SS；新交互pending_review，产品not_run |

**现状与目标分开：** R68源码已有 `openai_compatible`、`anthropic`、`azure_translator` 三种集中式协议实现，Rust `ModelGateway`、配置版本、任务和系统凭据入口已存在；实际启用哪些服务以配置档和实测记录为准。统一三层注册、其余协议、typed凭据与新字段均是本轮目标，不是当前包能力。词法/学习现状见[实现状态](implementation-status.md)，旧的“未下载/未集成/待评审”等原计划文字不代表当前全部实现状态。

本轮不改变已批准的材质、PNG、词典资源、Core或运行包；新增服务选择、导入和多字段认证状态还须在当前整稿中补映射并进行产品回归。HTML由本文件等现有文档导出，保留源文件hash，不覆盖旧报告。

## 1. 这次补什么

上一版计划保留了 REQ-029 分词与 REQ-021 LLM 接口，但日语阅读的展示、分词数据与模型任务分工仍不够具体。本次把语言学习提前为 **M2-L**：在 M2 翻译产品闭环后交付一个可用的学习切片，先评审设计，再做本地分词与按需文法。原 P1/P2 优先级不被改写，完整 P0 的验收口径不变。

| 能力 | 数据来源与运行方式 | 用户看到什么 | 出错时保留什么 |
|---|---|---|---|
| 分词、词性、词元与假名 | 随客户端内置的 MeCab＋固定 UniDic；离线分析 | 汉字词上方注音，可选词性色彩，点击词语卡 | 原文和翻译照常；未知词保留词面 |
| 翻译 | 传统HTTP服务、通用LLM、本地/远程模型适配器；第5.13节另列本机非大模型候选 | 原文先显示；译文可流式，显示服务与完成状态 | 原文、本地词法、其他已成功结果 |
| 本句文法与词义解释 | 独立 ExplainProvider，默认按用户点击调用 | 原文证据片段、文法点、简短解释、可能的其他理解 | 本地词卡；失败可重试，旧分析仍属于旧句 |

词性高亮只表达词法标签，不宣称已完成句法分析；模型给出的文法解释也不属于已证明的语言学事实。

## 2. 参考实现与新增决策

Luna 官方教程采用 MeCab／UniDic，提供注音与高亮开关，并展示原文上方注音。[官方分词教程](https://docs.lunatranslator.org/zh/qa1.html)（2026-10-18 核验）是本次功能参考；我们保留原项目的阅读与单层设置关系，材质、图标和配色采用当前已确认的暗蓝灰整稿。用户已明确基础词典随客户端内置：正式包安装后即可离线分词，首次启动不要求下载、寻找路径或另装词典。原稿中的自定义扩展资源能力仍保留在后续完整需求中。

本地固定 Fork 源码的 `myutils/mecab.py` 分别处理词面、读音、原型和词性，并对不同词典字段布局分支解析；显示层另控制注音格式与高亮。本次不复制其 Python/Qt 实现，保留“词法数据与展示分开”的边界。源码参考：[固定 Fork 的 mecab.py](https://github.com/Xeonice/luna-translator/blob/5a11c55e2cc8c0095a77f9f2145eb01bd6cfc410/src/LunaTranslator/myutils/mecab.py)。

MeCab 的节点 API 提供 surface、length、feature 等字段；这些字段与所用词典一起解释，不能把 IPA 的示例列序硬套到 UniDic。[MeCab API](https://taku910.github.io/mecab/libmecab.html)。UniDic 官方说明区分词面、词元、假名形和发音形，字段可随词典配置变化；长音发音形不能直接当规范假名读音。[UniDic 字段说明](https://clrd.ninjal.ac.jp/unidic/faq.html)。

工程决策：先验证一个固定版本的现代日语书面语 UniDic，并与分析器一起装入各平台发行包；清单锁定版本、编码、字段 schema、大小、hash 和许可。发布前校验基础资源完整、断网首启可用。支持扩展词典须新增 profile 与兼容测试，未知 schema 拒绝导入、不猜列号。绑定方式确定为MeCab C ABI，由本机独立Rust语言worker持有；精确资源版本、字段profile、二进制及中文释义覆盖仍须B0资源门禁锁定，当前设计预览尚未实施产品打包。

## 3. 开启分词后的阅读效果

HUD尺寸与操作行以当前整稿及[运行窗口、字幕与会话PRD](../product/reading-window-session-prd.md)为准；2026-10-18初稿的“仅暂停与更多常驻”已被后续常用操作直接上浮窗的评审要求替代。增加注音后高度向下扩展，不把注音硬塞入原 132pt 高度，也不压缩译文字号。长文限制正文区并允许滚动；按钮区保持可达。

- **关闭学习标注**：显示普通日文原文与译文，布局回到原阅读模式。
- **仅注音**：汉字词上方显示平假名；纯假名、标点不重复注音。可选片假名；罗马音作为后续兼容选项，另验证长音、促音和混合字符。
- **注音＋词性**：保留原字符顺序，以轻量底色区分名词、动词、助词等。图例和词卡有文字标签；不能只靠颜色传达信息。
- **选词**：点击词面显示读音、词元、词性及词典释义。释义必须标来源；词法分析器本身不提供中文释义。轻量词卡与完整查词窗分开，悬停默认不触发云模型。
- **本句文法**：从更多菜单或词卡打开独立学习面板，冻结当前句。HUD 继续跟随游戏，学习面板说明自己正在分析哪一句；不会把旧句解释放到新句上。

注音按词语对齐，不能把整词读音平均分给每个汉字。混合汉字假名先做可验证的送假名对齐；对齐不确定时整词注音或不标注，并保留未知状态，不生成看似精确的错位读音。

### 原页面归属

| 原 screen | 本次扩展 | 设计限制 |
|---|---|---|
| SCR-01 阅读 HUD | ruby、词性、选词态、文法入口 | 不新增一排常驻工具按钮 |
| SCR-09 查词 | 词法/词典卡；按需 AI 解释详情 | 本地信息与模型解释明确区分 |
| SCR-12 通用设置 | 阅读与语言学习展开页、内置词典信息、开关 | 保留原十类设置；基础词典开箱即用 |
| SCR-15 翻译服务 | 服务配置、翻译/文法任务分配、上下文范围 | 服务可共享凭据，但任务和模型独立配置 |
| SCR-18 显示设置 | 注音比例、间距、词性色彩、长文可读性 | 浅深色及 200% 字号分别验收 |

SCR-17 仍为语音设置。本次不新增第 23 个必需页面；文法详情是现有查词/学习能力的呈现扩展。

## 4. 词法链路与文本位置

```text
原始 Hook / OCR / 手动输入
  → 阅读单元 source_text + revision + 原始事件引用
  ├→ 翻译（纯文本，不包含 ruby 或词性色彩）
  └→ 有界本地词法任务 → TokenSnapshot → HUD / 查词
                                  └→ 用户请求 → 文法解释任务
```

词法运行在本机应用层的受控 worker 中，绝不进入 LunaHook 的游戏线程或 Host 回调。首个切片采用一个有界串行 worker 管理分析器实例，后台建立新词典实例后原子切换版本；采用最小C FFI与独立原生子进程的边界已固定；跨平台实际产物仍须PoC验证。不能直接给原生共享分析器套多线程调用并假设安全。

`TokenSnapshot` 至少包含：

| 字段 | 用途 |
|---|---|
| session、stream、unit_id、text_revision、source_text_hash | 精确归属正在显示/历史中的文本 |
| tokenizer_profile、dictionary_version、schema、analysis_revision | 解释结果来源与失效条件 |
| token_id、surface、lemma、reading、pos_path、conjugation | 查词、注音与词性标签 |
| start_byte、end_byte（半开区间） | 在完整 UTF-8 source_text 中定位；保留未知片段与空白 |
| reading_kind、unknown、alignment_status | 区分假名/发音、未知词、读音对齐是否可用 |

跨度先验证落在字符边界、按原文切片等于 surface。token 与显式空白/换行/gap 按顺序拼接必须还原同一 source_text。Rust 使用 UTF-8 byte span；WebView 通过显式映射转换到 UTF-16/DOM，不能直接当 JavaScript 索引。组合字符、emoji、重复词、换行和罕见汉字必须验证。

阅读单元可能是经过规则处理的文本，因此 token 引用阅读单元的确切版本；它与原始回调的映射需保留处理来源，不能宣称修改后的偏移仍是原游戏内存偏移。显示归一化也必须记录变换，不能为了定位直接改掉原文。

未知词保留原词面，无可靠读音时不猜；人名/游戏术语可通过版本化用户词典覆盖。用户修正是本地覆盖记录，注明作用游戏及版本，不默默改写分词器输出。点击原文、查词、复制、翻译和历史都从模型取纯文本，不读 ruby DOM 的 textContent 拼出重复注音。

## 5. 翻译引擎与模型服务设计

### 5.1 设计依据、范围及完成边界

本章是模型服务的技术主稿，产品行为以[模型配置PRD](../product/model-configuration-prd.md)为准。对应FP-06/08/10/11/12及已有AI-CFG-01～16；M04负责配置，M05负责调度，M06负责协议，M09负责学习，M10负责持久化，M16负责凭据。新增接口不扩大Hook/Core与语言worker职责。

用户先要求协议一次接全，随后明确排除腾讯TMT；因此本批固定为 **13个协议、21个接口预设、0个TMT兼容实现**。不再把Responses、Gemini原生、Google Advanced或签名/OAuth列为下一批。内部可以按依赖顺序实施，但不能以延期、只接兼容端点或缺测试账号为由删减目标。

唯一机器目录为[model-service-registry-design.json](model-service-registry-design.json)。下列两张表由同一目录生成，文档检查拒绝缺行、悬空引用、TMT回流、伪造通过或源表漂移。该检查只证明设计文件一致，不证明协议已实现。

### 5.2 协议全集

<!-- model-service-protocols:start -->
| 协议 ID | 协议 | 契约版本 | 类型 | 认证 | 模型身份 | 响应 | 用途 | 源码状态 | 适配／实服验收 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| openai_compatible | OpenAI Chat Completions | 1 | llm | Bearer / 本机服务按配置无认证 | required | JSON + SSE | translation, grammar, context_word | existing_source_requires_registry_migration | not_run / not_run |
| openai_responses | OpenAI Responses | 1 | llm | Bearer | required | JSON + 类型化 SSE | translation, grammar, context_word | not_implemented | not_run / not_run |
| anthropic | Anthropic Messages | 1 | llm | x-api-key + 协议版本头 | required | JSON + SSE | translation, grammar, context_word | existing_source_requires_registry_migration | not_run / not_run |
| gemini_generate_content | Gemini 原生 generateContent | 1 | llm | Gemini API Key | required | JSON + SSE | translation, grammar, context_word | not_implemented | not_run / not_run |
| ollama_native | Ollama 原生 /api/chat | 1 | llm | 按本机/云端实际部署声明 | required | JSON + NDJSON | translation, grammar, context_word | not_implemented | not_run / not_run |
| lmstudio_native_v1 | LM Studio 原生 /api/v1/chat | 1 | llm | 按服务认证设置声明 | required | JSON + SSE | translation, grammar, context_word | not_implemented | not_run / not_run |
| azure_translator | Microsoft Azure Translator | 1 | translation | 订阅 Key + 可选 region | not_applicable | JSON | translation | existing_source_requires_registry_migration | not_run / not_run |
| deepl_v2 | DeepL Text API | 1 | translation | DeepL-Auth-Key | not_applicable | JSON | translation | not_implemented | not_run / not_run |
| google_translate_v2 | Google Cloud Translation Basic v2 | 1 | translation | Basic 认可的 API Key 等认证 | not_applicable | JSON | translation | not_implemented | not_run / not_run |
| google_translate_v3 | Google Cloud Translation Advanced v3 | 1 | translation | OAuth / IAM + project/location | optional | JSON | translation | not_implemented | not_run / not_run |
| baidu_vip | 百度通用翻译 | 1 | translation | APPID + secret + salt / MD5 | not_applicable | 表单 → JSON | translation | not_implemented | not_run / not_run |
| youdao_v3 | 有道文本翻译 | 1 | translation | appKey + secret + salt + curtime / SHA256 | not_applicable | 表单 → JSON | translation | not_implemented | not_run / not_run |
| tencent_tokenhub_translation | 腾讯 TokenHub 专用翻译 /api/translations | 1 | translation | Bearer | required | JSON + 流式 delta | translation | not_implemented | not_run / not_run |
<!-- model-service-protocols:end -->

传统翻译仅分配translation用途；LLM协议允许哪些学习用途还要与厂商profile、实际模型能力和用途测试取交集。模型列表可访问、协议能发送或JSON合法均不能提升用途可用性。Google Basic与Advanced、Chat与Responses、Gemini兼容与原生、TokenHub Chat与专用翻译分别有独立契约，不静默互换。

Ollama/LM Studio原生只覆盖本产品的文本推理与学习请求，不开放模型下载/管理、MCP、工具执行、音视频或图像业务。TokenHub hy-mt2的Chat路由与专用翻译不是同一请求格式；术语库权限、语言码和地域/站点域名由明确profile约束，并逐项实测。腾讯旧TMT、TC3专属签名/凭据和专项验收均已排除，TokenHub两路保留。

### 5.3 厂商与接口预设

<!-- model-service-presets:start -->
| 预设 ID | 名称 | 路由 | Provider ID | 协议／契约版本 | 兼容配置／版本 | 预设版本 | 用途 | 实现／验收 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| builtin.deepseek | DeepSeek | deepseek | builtin.deepseek | openai_compatible@1 | deepseek-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.qwen | 通义千问 / 阿里云百炼 | qwen | builtin.qwen | openai_compatible@1 | qwen-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.ark | 豆包 / 火山方舟 | ark | builtin.ark | openai_compatible@1 | ark-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.gemini-compatible | Gemini 兼容模式 | gemini-compatible | builtin.gemini | openai_compatible@1 | gemini-compatible-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.openrouter | OpenRouter | openrouter | builtin.openrouter | openai_compatible@1 | openrouter-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.ollama | Ollama | ollama | builtin.ollama | openai_compatible@1 | ollama-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.lmstudio | LM Studio | lmstudio | builtin.lmstudio | openai_compatible@1 | lmstudio-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.openai-chat | OpenAI Chat Completions | openai-chat | builtin.openai | openai_compatible@1 | openai-chat-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.anthropic | Anthropic Messages | anthropic | builtin.anthropic | anthropic@1 | anthropic-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.azure-translator | Microsoft Azure Translator | azure-translator | builtin.microsoft-translator | azure_translator@1 | azure-translator-text@1 | 1 | translation | planned / not_run |
| builtin.deepl | DeepL API | deepl | builtin.deepl | deepl_v2@1 | deepl-text@1 | 1 | translation | planned / not_run |
| builtin.google-v2 | Google Cloud Translation Basic v2 | google-v2 | builtin.google-cloud-translation | google_translate_v2@1 | google-v2-text@1 | 1 | translation | planned / not_run |
| builtin.baidu | 百度通用翻译 | baidu | builtin.baidu | baidu_vip@1 | baidu-text@1 | 1 | translation | planned / not_run |
| builtin.openai-responses | OpenAI Responses | openai-responses | builtin.openai | openai_responses@1 | openai-responses-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.gemini-native | Gemini 原生接口 | gemini-native | builtin.gemini | gemini_generate_content@1 | gemini-native-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.youdao | 有道智云文本翻译 | youdao | builtin.youdao | youdao_v3@1 | youdao-text@1 | 1 | translation | planned / not_run |
| builtin.google-v3 | Google Cloud Translation Advanced v3 | google-v3 | builtin.google-cloud-translation | google_translate_v3@1 | google-v3-text@1 | 1 | translation | planned / not_run |
| builtin.ollama-native | Ollama 原生 | ollama-native | builtin.ollama | ollama_native@1 | ollama-native-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.lmstudio-native | LM Studio 原生 | lmstudio-native | builtin.lmstudio | lmstudio_native_v1@1 | lmstudio-native-text@1 | 1 | translation, grammar, context_word | planned / not_run |
| builtin.tencent-tokenhub-chat | 腾讯 TokenHub hy-mt2 Chat | tencent-tokenhub-chat | builtin.tencent-tokenhub | openai_compatible@1 | tencent-tokenhub-chat-text@1 | 1 | translation | planned / not_run |
| builtin.tencent-tokenhub-translation | 腾讯 TokenHub 专用翻译 | tencent-tokenhub-translation | builtin.tencent-tokenhub | tencent_tokenhub_translation@1 | tencent-tokenhub-translation-text@1 | 1 | translation | planned / not_run |
<!-- model-service-presets:end -->

21是服务/路由预设数，不是21家公司。每个预设引用一个精确协议与profile；同一厂商提供多个协议时由用户明确选择，失败不能自动换协议或供应商。默认地址按站点/地域和服务控制台填写；预设更新不反写用户已应用的endpoint、模型或Key。

### 5.4 三层注册及唯一装配入口

| 层 | key / 内容 | Owner及边界 |
|---|---|---|
| ProtocolAdapterRegistry | `(protocolId, adapterContractRevision)` → `Arc<dyn ProtocolAdapter>` | M06；真实wire编解码、认证接线、流事件、语言映射、错误转换和发现能力 |
| CompatibilityProfileRegistry | `(compatibilityProfileId, profileRevision)`，引用完整协议key | M06；参数schema、已实现的受控字段变换、认证要求与能力上限，不含脚本 |
| ProviderPresetRegistry | `(presetId, presetRevision)`，引用完整协议/profile key | M04/M06；显示名、默认地址、非秘密默认值和来源，不持有密钥或业务状态 |

同一tuple重复注册拒绝；不同版本可并存。profile不能跨协议版本误绑定，preset与Connection必须同时匹配两个key。协议硬能力、profile限制、模型声明与实际测试是不同来源，运行使用其交集；`supported/unsupported/unknown`均需保留。用户声明和厂商名称不能扩大硬能力。

中心注册仅放在Tauri组合根调用的Rust构造函数，不在runtime/UI中堆厂商分支。以下是目标接口示意，不是当前已存在的API：

```rust
let mut builder = ProviderRegistryBuilder::new();
register_builtin_protocols(&mut builder)?; // 本批全部13协议
register_builtin_profiles(&mut builder)?;
register_builtin_presets(&mut builder)?;   // 21个接口预设
register_extensions(&mut builder)?;       // 受审的编译期开发扩展
let registry = Arc::new(builder.freeze_and_validate()?);
let catalog = registry.catalog();         // 由同次注册产生的纯DTO目录
```

`catalog`与协议实现同源，注入app-core的纯校验与应用端口；app-core不能导入app-infra。UI从Rust目录读取协议、服务、字段和禁用原因，不各自维护支持列表。协议/profile/内置预设在启动后不可变；用户导入的不可变预设revision形成新的目录视图快照，不修改可执行协议表。

```text
React 设置/阅读/学习
  → 窄Tauri命令/订阅
  → app-core：用途、请求材料、版本、调度与结果归属
  → ModelGateway：精确lookup → 授权/能力检查 → 凭据lease
  → AdapterRegistry(protocol tuple) + CompatibilityProfile(profile tuple)
  → 受控HTTP transport → 模型/翻译服务
          ↑
  系统Vault / 版本化凭据 / OAuth token material

同源ProviderCatalog → core配置校验 / UI目录与字段
UserPresetRepository → 目录视图快照 → 连接草稿（不自动调用）
```

### 5.5 适配器与兼容规则契约

沿用现有`ModelGateway`与`PortFuture`，不引入新的AI编排框架或JS业务服务。目标`ProtocolAdapter: Send + Sync`公开`descriptor`、`execute(context, RequestPlan, on_delta)`、`discover_models(context)`；测试请求通过同一gateway/adapter路径执行，不另写“测试专用假成功”实现。context仅含本次冻结连接、受控transport和短期凭据材料，不能取得Repository、UI或完整profile状态。

| descriptor/profile字段 | 类型/约束 | 使用方式 |
|---|---|---|
| protocolId / contractRevision | 受限稳定ID / 正整数 | 编解码行为版本，精确查找，不用服务显示名推断 |
| purpose / modelIdentity | 用途集合；required/optional/not_applicable | 传统翻译不填虚构模型ID；精读独立校验 |
| authKinds / credentialSchema | 已实现认证种类和typed字段描述 | UI生成输入项，秘密值只短暂进入安全保存命令 |
| parameterSchema / protectedFields | 类型、范围、互斥、允许键与保留键 | 保留继承/设置/省略三态；不丢弃显式不兼容配置，不覆盖model/路由/凭据/tools |
| request/response dialect | 适配器支持的具名格式变体 | 允许受审字段重命名/缺省策略；不执行用户表达式或任意JSONPath脚本 |
| output/discovery/language | JSON/SSE/NDJSON、发现策略和语言映射 | 无models端点允许手填；不能普遍用GET /models作健康检查 |
| capability evidence | 文档、模型声明、测试对象及版本 | 协议能编码流不等于该服务/模型已验证流式或结构化 |

所有execute/test/discover入口均先精确lookup；未知ID、缺失版本、能力或认证冲突在取密钥与发网络前失败。不得保留“未知协议默认Bearer/Chat或默认/models”的兜底。网络客户端仅按实际代理/超时等非秘密配置复用；Key不能写进共享client默认header造成跨连接串用。

### 5.6 用户新增厂商与开发者新增协议

用户可从“设置 → 模型配置 → 连接 → 导入服务预设”或同一编辑器的新建入口创建草稿。导入文件只能配置已安装的协议/profile/schema；目标示例：

```json
{
  "schemaVersion": 1,
  "presetId": "user.my-service",
  "presetRevision": 1,
  "displayName": "我的兼容服务",
  "adapterBinding": {
    "schemaVersion": 1,
    "protocolId": "openai_compatible",
    "adapterContractRevision": 1,
    "compatibilityProfileId": "openai-chat-text",
    "profileRevision": 1
  },
  "endpoint": "https://api.example.com/v1",
  "deployment": "cloud",
  "authMode": "api_key",
  "options": {"temperature": 0.2}
}
```

处理顺序为：结构校验 → 协议/profile引用与字段白名单校验 → endpoint/权限语义校验 → 保存新用户预设revision → 建Connection/Model草稿 → 用户另行输入凭据 → 显式测试 → 应用/启用。导入/保存不外发、不复用已有Key、不更改活动配置。推荐托管于`<appData>/profiles/<profileId>/provider-presets/`；数据库保留不可变内容与hash，文件不是运行时权威。

`builtin.*`与`user.*`隔离；用户不能覆盖内置项。同一用户ID修改产生新revision，已应用配置仍保留冻结内容。无效导入只拒绝该导入并定位错误，不让客户端启动失败。文件中拒绝秘密、vault/pool引用、任意认证header或可执行代码；options须再过profile schema。跨档/设备导入不带凭据授权，不继承旧测试通过。

新增真正wire协议时，开发者在`providers/adapters/<id>.rs`实现trait，提供`register(&mut ProviderRegistryBuilder)`同时登记descriptor/profile/预设，中心`register_extensions`增加一处调用，再运行契约/边界测试并重新打包签名。第一版采用编译注册；免重编只适用于已有协议/字段/认证/profile支持的数据预设，不承诺运行时dylib、JS或WASM插件。

### 5.7 目标目录与复用

以下为拟定目录，不表示文件已存在；实施前按工程目录规则冻结允许路径。

```text
crates/app-core/src/
├── domain/model_config/catalog.rs     # 纯描述、能力、参数/引用校验
├── contracts/                         # 目录DTO、AdapterBinding、typed凭据命令
└── ports/runtime.rs                   # 保留ModelGateway/PortFuture
crates/app-infra/src/providers/
├── gateway.rs                         # 授权/凭据/adapter分派
├── registry.rs                        # 精确tuple索引及冻结
├── profiles.rs                        # 受控兼容规则
├── presets.rs                         # 内置服务/接口预设
├── register.rs                        # 中心装配与开发扩展登记
├── transport.rs                       # 有界reqwest、代理/超时与脱敏
├── auth/                              # Key、签名、Google OAuth
└── adapters/                          # 本批13协议，各自实现/fixture
crates/app-infra/src/storage/
├── migrations/<new-number>_*.sql       # 新编号，不能改旧迁移
└── queries/                           # 用户预设/配置/授权关系
apps/desktop/src-tauri/src/bootstrap.rs # 窄组合根
apps/desktop/src/features/model-config/# 复用现有编辑器/Select，目录驱动
crates/app-infra/tests/provider_*.rs    # 协议与生命周期实测
```

M05的排队、并发、重试、上下文、术语、缓存和任务/attempt归属继续唯一实现；adapter不能复制调度、访问SQL、改变用途或在Hook线程调用模型。UI只走应用端口，Hook/语言worker保持独立。

### 5.8 全量认证及秘密生命周期

| 认证种类 | 对象及非秘密配置 | 秘密与请求处理 |
|---|---|---|
| none | 适用本机/内网，显式选择 | 不生成假Key；依现有endpoint与部署规则校验 |
| api_key及专用头 | Bearer、Anthropic、Google Key、DeepL、Azure/region | 单Key存Vault；header/必要query由协议实现，配置endpoint不得嵌入凭据 |
| baidu_app_secret | APPID、签名版本、语言映射 | typed envelope存secret，按原文与salt计算官方MD5，签名后再编码表单 |
| youdao_app_secret | appKey、签名版本、校时 | secret、salt、curtime及官方长文本input规则计算SHA256；Unicode长度/截断须官方fixture核对 |
| google_service_account | project/location、明确主体、允许scope | 私钥进入Vault，使用经审阅的认证/密码学库换取短期access token，不自行实现签名算法 |
| google_oauth_user | 公共client ID、issuer、project/location、scope、grant | 系统浏览器Authorization Code + PKCE和有界loopback回调；refresh token只入Vault，access token仅短期lease |

本批不含TC3或旧TMT专属凭据。现有`credential_refs.auth_kind=api_key`、`CredentialInput(label,secret)`和`Connection.auth_mode=none/api_key`不足以表达全部认证，必须作为同批DTO/安全存储/新迁移改造，不能将多字段secret塞入networkMetadata。现有Key池、不可变版本、撤销和实际凭据来源仍保留。

Google Advanced固定使用翻译接口接受的`https://www.googleapis.com/auth/cloud-translation` scope；更广scope必须由单独配置/授权显式审阅，不能用默认扩大权限。服务账号和用户OAuth是两种清楚标识的授权路径，不把桌面应用内置client secret当作可保密材料。服务账号私钥/用户refresh token均不可导出到普通配置。

OAuth将**不可变授权身份**与**可更新token材料**分开：grant固定issuer/client、可验证主体或明确unknown状态、scope、credential来源及撤销epoch；新登录/账号变更创建新grant并显式应用。拿不到可信主体信息时不得用显示名认作同一账号，也不为识别账号额外申请无关scope。

同一grant的access token刷新不修改已应用配置语义。refresh-token轮转产生新的Vault材料版本，以grant/epoch/当前材料revision做CAS推进cursor，不原地覆写旧秘密或伪称credential版本不变。任务冻结grant及授权epoch；每attempt记录实际材料revision和无秘密lease来源。刷新返回扩大scope、主体不一致或撤销时拒绝，取消受影响任务并要求重新授权，不静默绑定另一账号。服务账号换私钥按新凭据版本处理。

认证刷新遵守任务取消/超时预算，不脱离任务后台运行；AuthProvider可提前更新即将到期的lease，重新发送模型请求仍归业务层的attempt/重试策略控制。access token有效期、刷新失败、无refresh token、时钟偏差、重复刷新与轮转崩溃恢复分别测试。刷新缓存不写入共享HTTP默认头，token、签名、私钥、原文与供应商错误body不进日志。

依据：[Google Translation v3与授权scope](https://docs.cloud.google.com/translate/docs/reference/rest/v3/projects.locations/translateText)、[桌面OAuth](https://developers.google.com/identity/protocols/oauth2/native-app)、[服务账号](https://developers.google.com/identity/protocols/oauth2/service-account)。具体Rust认证库、依赖版本与许可须实施前过技术锁审阅；本轮不修改Cargo或引入Node/Python服务。

### 5.9 持久化、版本与请求身份

DTO仍使用`Connection.adapterBinding`表达协议/contract/profile/schema版本。与早期调研中单个`adapter_binding_json`候选相比，本轮按[数据库设计](database-design.md)“参与授权/关系的关键字段独立建列”规则，建议以独立候选列持久化binding；字段和关系详见该稿新增章节。旧JSON候选不再与本稿并列作为实施目标；这不是已批准DDL。

协议、profile、endpoint、实际模型、有效参数、模板/上下文/术语与配置版本共同进入请求、测试和缓存身份。preset标签/来源revision与请求行为版本分开；仅修改显示名不应伪造新能力。缺失已注册版本的新请求明确失败，历史仍可读，不自动改用latest。

旧配置继续按原编码验证seal；只对已识别旧格式中缺失的新字段使用固定legacy映射，损坏字段/未知版本/缺适配器不得落入legacy回退。新增配置使用新codec与显式字段，不回写旧revision、hash或冻结测试。迁移需真实旧profile副本、严格类型/CHECK/FK/trigger对照和崩溃回滚验收，本轮不改初始DDL、schema-contract、字段CSV或用户数据库。

### 5.10 流式、错误、取消与网络边界

统一业务输出仍是有界累积文本与明确最终状态。JSON、SSE和NDJSON的字节分帧、UTF-8、多事件和合法结束分别由协议解析；thinking、tool调用、拒绝/敏感审核、截断、流内error、超限或缺结束不得作为完整译文提交。usage缺失表示unknown，不填0或推算费用。语言码按协议转换；TokenHub与旧教程不同的繁中码/地域资料先记录证据并实测，不主观纠正后判通过。

失败映射至少区分配置/版本不支持、凭据缺失/撤销、认证失败、限流/额度、网络/超时、响应格式、拒绝/截断、能力和主体/授权冲突。诊断保留安全code/stage/status及脱敏来源，不输出服务body/完整带参URL。HTTP跳转禁止沿用认证；配置地址禁止userinfo/secret query。

取消由现有Rust runtime中止future；adapter不得脱离生命周期spawn。新流、迟到分片、重试、OAuth刷新和结果提交均再次核对task/attempt、授权epoch和配置代次。取消后不能写成功/缓存；停止本地接收不保证供应商停算或不计费。传统和本机某服务不支持远端取消时明确显示能力，不制造确认回执。

### 5.11 验收、依赖与实施步骤

本批按“注册核心/目录 → typed凭据/DTO/迁移 → 全部13协议/21预设 → 协议fixture/负例 → 逐服务真实调用 → 同一新包回归”的依赖顺序实施，不把任何协议移出本批。测试源码复用`provider_protocol.rs`、配置生命周期与存储测试；UI目录/导入/认证字段用现有组件与真实端口验证。

[机器目录](model-service-registry-design.json)的PA-01～12是本轮设计场景，均为not_run；尚未注册为产品执行器。原MC-18只含TRADITIONAL/REMOTE-COMPAT/ANTHROPIC/LOCAL-COMPAT四变体，不能据其登记声称覆盖13协议；实施前须新增逐协议/逐路由变体和真实adapter绑定，保留原CSV/153组合历史身份。

| 验证层 | 方法和断言 | 前置输入 / 维护归属 |
|---|---|---|
| 设计一致性 | 13协议/21引用、TMT排除、状态与MD矩阵、source hash/HTML同源 | 本JSON/现有MD；文档工具，仅证明设计一致 |
| 注册/业务负例 | 重复tuple、缺/跨协议profile、未知发现、能力越权、保留参数、用户导入命名空间 | Rust单测与本地fixture；M04/M06 |
| 协议HTTP | 本地真实listener逐项断言route/认证/语言/参数/流/终止/错误，不只看HTTP200 | 授权录制或合成样本、mock凭据；M06 |
| 凭据/OAuth | 系统Vault、签名Unicode、撤权、PKCE/回调、scope/主体、并发刷新/轮转CAS与崩溃恢复 | 平台Keychain/Credential Manager、测试OAuth client/账号；M16/QA |
| 存储与迁移 | 新旧codec/seal、字段边界、复合关系、真实旧profile副本、事务回滚 | 新DDL独审、PRE-14支持矩阵与冻结旧库；M10/QA |
| 真实服务用途 | 指定实际模型的ja→zh、多句、学习对象引用、流/错误恢复与取消，按服务独立回执 | 账号、API开启、Key/额度、IAM、project/location、可用网络与同意发送的样句；AI/QA |
| UI/正式包/游戏 | 目录驱动、导入只建草稿、各用途隔离，SR/MO/Never7/樱之诗同包连续变化 | 批准设计状态、新签名包、可见桌面、游戏与对白操作；UI/PL/QA |

以下展开目录中的逐项规划，状态均为`not_run`，只定义后续应证明的行为：

| 用例 | 要验证什么 | 计划状态 |
|---|---|---|
| PA-01 · 注册完整性 | 重复 ID、预设引用缺失适配器、目录/实现不一致都拒绝启动。 | `not_run` |
| PA-02 · 所有入口精确查找 | 未知协议在应用、发送、测试、发现四处拒绝；零取密钥、零网络；禁止默认 Bearer / GET models。 | `not_run` |
| PA-03 · 用途与硬能力 | 传统翻译用于 grammar/context_word 必须失败；用户声明不能突破 adapter 硬限制。 | `not_run` |
| PA-04 · 请求契约 | 本地 HTTP fixture 实际检查路由、认证、签名、语言码、模型与参数白名单。 | `not_run` |
| PA-05 · 流与终止 | 拆分 UTF-8 / SSE、缺终止、流内错误、拒绝、截断、reasoning-only / tool 输出、超大响应。 | `not_run` |
| PA-06 · 生命周期隔离 | 取消、撤权、停用、换配置后迟到流不能写成功/缓存；future 不得脱离任务。 | `not_run` |
| PA-07 · 秘密和网络隔离 | Header/query/body/错误/Debug 脱敏；共用 client 不串 Key，Key 不进入默认头或普通 JSON。 | `not_run` |
| PA-08 · 旧配置迁移 | 真实旧 profile 备份副本迁移、事务回滚、FK/seal/历史保持；旧客户端明确拒绝新 schema。 | `not_run` |
| PA-09 · 身份与缓存 | 从已保存adapterBinding冻结protocol/contract/profile版本进入请求/测试/缓存身份；升级失效，preset更新不能重解释旧配置。 | `not_run` |
| PA-10 · 架构与 UI | UI/核心不得绕层 HTTP/SQL/FFI；目录驱动表单保持草稿、测试、应用和启用分离。 | `not_run` |
| PA-11 · 真实服务验收 | 每服务实际 ja→zh、多句连续、取消/失败恢复；学习用途单独核解释结果，模型列表成功不代替。 | `not_run` |
| PA-12 · 同包游戏回归 | 新旧候选分别固定包身份；所有新服务改动在同一新包内完成 SR/MO/Never7/樱之诗回归，不继承旧包通过。 | `not_run` |

缺Key、账号权限、额度或网络写blocked，未执行写not_run；不删协议、不用fixture替代live、不继承旧包通过。真实输入与密钥不进入测试仓库；运行记录独立于计划，含协议/profile/服务/模型/config/凭据版本、包身份、命令、错误及来源。文档审阅、字段批准、代码/编译和产品实测分别记录。

### 5.12 官方来源与局限

每协议官方入口保留在机器目录并随HTML导出。重点差异另参考[Responses迁移](https://developers.openai.com/api/docs/guides/migrate-to-responses)、[Gemini原生](https://ai.google.dev/api/generate-content)、[Ollama流](https://docs.ollama.com/api/streaming)、[LM Studio原生](https://lmstudio.ai/docs/developer/rest/chat)、[TokenHub模型协议矩阵](https://cloud.tencent.com/document/product/1823/130079)及[专用翻译](https://cloud.tencent.com/document/product/1823/132252)。资料核对日期为2026-10-30；API/地域/模型/权限可变，实施前再核。百度正文部分来自官方搜索索引，不能代替当前控制台契约和真实请求。

本地分词/基础词典仍随客户端内置；模型是否支持翻译、学习或结构化输出按实际用途检验。未来扩展服务也继续走相同注册与授权，不把该设计转成原版Python客户端或外部Web API依赖。

<a id="offline-translation-backends"></a>

### 5.13 本机非大模型翻译：独立执行backend与目录

产品语义见[模型配置PRD第7章](../product/model-configuration-prd.md#offline-translation-prd)，交互为[独立完整候选稿](../design/first-product-v1/candidates/local-offline-20261030/index.html?scenario=settings)，均属[本轮设计工作项](../../openspec/changes/design-local-offline-translation/proposal.md)，不是已批准实现合同。既有protocols/presets/counts原对象保持不变；offline_engines是并列目录，不把四个本机引擎计作第14～17个HTTP协议或厂商预设。

| execution_backend | 执行边界 | 配置身份 |
|---|---|---|
| http | 既有ProtocolAdapterRegistry与受控HTTP transport | 原Connection/Model及AdapterBinding，保留13协议/21预设 |
| platform_native | Rust调用窄系统翻译适配；本轮候选为Apple Translation | OfflineEngineConfig＋精确引擎契约＋系统/API/策略/资源观察身份 |
| local_worker | Rust监督独立翻译子进程；本轮候选为Argos路线/J北京/金山 | OfflineEngineConfig＋精确worker合同＋安装/引擎/数据包与语言路由快照 |

ExecutionBinding是拟议的带标签联合：HTTP引用原adapterBinding；离线引用engineId/engineContractRevision/engineConfigId/resourceSnapshotId与backendKind。翻译用途引擎项的EngineTarget分别引用已有model_config或独立offline_engine_config，只允许一个目标；不为离线引擎生成假Connection.protocol、endpoint、API Key或models.model_id。具体DTO、端口和SQL字段须实施前另审，不能用普通JSON扩展绕过类型与关系。

同源目录仍从Rust注册生成，只读DTO投影给UI；新增OfflineEngineRegistry负责精确engine key、backend、平台/资源/用途硬能力及其来源。runtime/UI不据名称猜路径，不动态加载用户脚本为协议。新增目录允许展示候选，但没有安装实现/资源/受审运行路线时可保存草稿、不可执行。所有离线候选硬限制为translation，不能借厂商或模型声明提升grammar/context_word。

    React统一设置/翻译用途
      → 窄Tauri端口 → Rust配置/权限/任务/epoch/输入与结果归属
      → ModelGateway复用既有调度入口，按EngineTarget精确分派
        ├ http → ProtocolAdapterRegistry → 受控HTTP
        ├ platform_native → 注入的窄系统翻译端口 → Apple Translation
        └ local_worker → 独立翻译worker监督器 → 同位数引擎/语言包
      → 既有History/Cache/Repository事务
    Hook host / capture collector / language-worker均不承载翻译引擎

M04/AP持配置与有效绑定；M05/AI继续唯一持调度、取消、回退、缓存和结果归属；M06/AI持执行适配与目录；M12/PL提供平台窄能力，M10/AP持存储。M16仅保留既有凭据职责，离线无Key不生成假凭据。Core不依赖Apple/Win32/推理库，Tauri层不持业务策略。translation-worker与language-worker分别有自己的协议、资源锁和生命周期，不将DLL加载进UI、主应用业务进程、Hook Host或游戏。子进程隔离是故障边界，不自动构成沙箱或安全执行任意DLL的授权。

### 5.14 四个候选、平台与技术锁边界

| 引擎ID | 设计路线与前提 | 许可/兼容/实现状态 |
|---|---|---|
| apple_translation | platform_native；系统Translation框架，选择传统翻译能力；逐版本检查API和所选语言对 | 无用户URL/model/Key；系统包由系统管理；本产品bridge/断网/取消实测not_run |
| argos_translate | local_worker候选；Argos全流水线和语言包参考，先独立PoC比较原实现行为 | 上游Python；生产Python/其运行框架/新推理依赖未批准；不能称已采用或可分发；not_run |
| jbeijing | 用户合法安装Windows引擎，同位数独立worker加载受支持DLL/数据 | 不随包分发DLL/商业数据；版本/位数/授权/Windows与CrossOver均需单独核对；not_run |
| kingsoft_fastait | 用户合法安装Windows引擎，同位数独立worker加载受支持DLL/词库 | 同上；不把上游实现存在写成当前产品兼容；not_run |

**Argos专用NMT也有模型包。** 上游Python库的语言包包含推理模型和分词/分句等配套资源，并可经中间语言组合。它不要求用户部署通用大模型，但不能称“无需任何模型/语言包”。仅调用CTranslate2不等价于Argos完整分句、分词、推理、合并及中转流水线；若考虑Rust监督原Python或原生重实现，两者都只是待评审路线，须用同包样本、Unicode/多句/特殊符号/语言方向与质量PoC核对，再审精确运行栈/依赖锁/许可。不得未经批准把Python、LibreTranslate服务、PyQt或其他框架加入生产。依据：[Argos官方包与流水线](https://argos-translate.readthedocs.io/en/stable/source/argostranslate.html)、[官方分句设置](https://argos-translate.readthedocs.io/en/latest/source/settings.html)。

J北京和金山只把上游加载边界作为参考：[LunaTranslator jb7.py](https://github.com/HIllya51/LunaTranslator/blob/main/src/LunaTranslator/translator/jb7.py)、[kingsoft.py](https://github.com/HIllya51/LunaTranslator/blob/main/src/LunaTranslator/translator/kingsoft.py)。这些是上游项目的一手实现，**不是供应商兼容/免费/再分发授权证明**。本轮不推断DLL符号、编码、语言方向、位数或具体商业版本均兼容；后续对用户合法安装逐项冻结ABI/位数/返回内存所有权、初始化/销毁顺序、编码与调用线程，再以真实worker验证。

Windows worker按引擎DLL位数选择，和游戏位数无关。CrossOver是额外候选运行环境：冻结Bottle身份、Windows worker实际架构、Wine/CrossOver版本与商业安装；Windows原机通过不能代替CrossOver，CrossOver通过不能代表原生macOS翻译DLL。DLL和词库不进入发行包或导出配置，不通过游戏Hook加载它们。

Apple桥接语言/生命周期同样需要最小PoC与技术锁审阅。若macOS 15路线需要Swift/SwiftUI受控host，仅是系统适配候选，不等于批准新增生产UI框架；React界面、Rust权威状态和现有Tauri边界保留。不得因为官方示例在SwiftUI View内就让React页面挂卸载控制业务请求。

### 5.15 Apple版本、资源准备与非大模型语义

以下是2026-10-30直接核对Apple官方符号metadata所得的API下限，不代表当前Tauri包已完成桥接或该机该语言对可用：

| 能力 | 官方macOS availability | 实施要求 |
|---|---|---|
| TranslationSession、prepareTranslation | 15.0+ | 基础会话与系统语言下载许可；需核对会话从有效系统host取得及其生命周期 |
| init(installedSource:target:)、cancel() | 26.0+ | 已安装语言的无UI会话与session取消；15.x不能直接调用这些新符号 |
| Strategy.lowLatency及带策略初始化 | 26.4+ | lowLatency为traditional models；显式选择并按同策略检查语言可用性 |

来源：[TranslationSession](https://developer.apple.com/documentation/translation/translationsession)、[prepareTranslation](https://developer.apple.com/documentation/translation/translationsession/preparetranslation())、[installedSource初始化](https://developer.apple.com/documentation/translation/translationsession/init(installedsource:target:))、[cancel](https://developer.apple.com/documentation/translation/translationsession/cancel())、[lowLatency](https://developer.apple.com/documentation/translation/translationsession/strategy/lowlatency)。版本下限来自各页官方Markdown元数据；实施时仍须以受锁SDK编译及实际OS availability检查，不能仅检查framework存在。

非大模型候选在可用版本明确选择lowLatency；highFidelity属于Apple Intelligence路线，不在本轮范围。旧OS/旧SDK无策略符号时，须PoC证明确实使用该版本传统翻译路径及限制；无法确定时不可宣称符合非大模型要求。不能静默选highFidelity或把新API视为macOS15已提供。官方说明不同策略语言集合可能不同，检查与执行须使用相同策略。

LanguageAvailability按明确源/目标语言对返回installed/supported/unsupported；supported只代表可准备，不是已安装或实际翻译成功。系统下载许可必须独立由用户触发；不可在保存草稿/导入/自动翻译时后台下载或反复弹窗。prepareTranslation不产生译文，取消许可和断网缺包保持未就绪。新headless会话仅用于已安装资源，不能绕过系统下载授权。[LanguageAvailability](https://developer.apple.com/documentation/translation/languageavailability)。

系统session.cancel尝试停止该session所有工作，不保证同步完成；已取消session不可继续复用。旧版无专用cancel时使用该系统任务实际可用的取消方式并停止本地提交，必要的资源回收以PoC证据为准。一个session取消影响所有在途，因此未证明独立取消前每语言对受控串行/单在途；不得把多个独立业务任务共享一个不可分割取消域。

Apple声明TranslationSession内容在设备处理，可能收集独立使用量/性能指标；本产品不能据此声称机器零网络包或复制/分发系统语言数据。语言包下载与内容推理的网络边界分别验证。系统未公开语言包精确版本/hash时，按下节可观察身份与缓存作用域处理。

### 5.16 资源、worker合同与故障隔离

资源准备独立于配置应用与执行。用户选定安装/包目录后Rust仅检查授权范围内的已知文件；标准化实际路径、拒绝越界/不受支持内容、读取受控元数据，再生成不可变ResourceSnapshot。包的引擎版本、平台/架构、数据/分词/分句版本、每文件hash或明确不可观察状态、来源/许可材料和所需磁盘条件分别保存。资源解包须防路径穿越/解压膨胀；实际阈值在PoC后冻结，不为文档测试硬编码假容量。

默认direct_only；没有ja→zh直达包即缺资源。用户另行选择pivot须显示并冻结完整ja→en→zh路径、每跳语言/包/处理规则与质量限制，经明确应用后执行。不得调用上游自动pivot作为隐式回退；失败不改语言/路由/供应商。

独立翻译worker私有协议拟为translation-worker-v1，**不复用Hook JSONL v2或语言worker协议身份**。Hello/Ready返回worker build、engine contract、实际安装/资源指纹、位数和探测能力；Execute带request/task/attempt/profile_generation/authorization_epoch、固定source unit/revision/hash、语言对、精确资源/route和deadline；Cancel带同一请求身份；Final只有有界译文、实际资源路径身份、状态/安全错误码，不含可执行脚本/任意加载参数。协议具体字段/容量与DTO须独立审阅后冻结。

父进程监督启动、握手、单引擎有界队列和退出，stdout只传协议、stderr有界脱敏。外部原生API未证明线程安全前串行调用。超时先请求取消/关闭，再在有限grace后回收自己拥有且身份匹配的worker；不能杀游戏、Hook、词法worker、其他用户实例或任意同名进程。初始化/卡死/崩溃/超长响应映射独立错误，恢复必须有新worker_instance_id并隔离旧回包；是否自动重启不等于允许自动重试原翻译。

UI的“停止/取消”在Rust先推进终态/epoch并阻止提交；物理停止是否支持单列能力，不能把停止等future误报DLL计算已停止。所有结果提交复核任务、attempt、epoch、profile generation、worker instance和资源身份。原句推进但未取消且身份有效的结果只写原历史，取消/停用/换资源后的迟到不写成功/缓存。

目标目录仅为待评审分工，不表示文件已存在或放宽允许路径：

    crates/app-core/src/domain/model_config/    # EngineTarget/OfflineEngineConfig纯业务
    crates/app-core/src/ports/                  # 注入系统执行/worker端口，沿用ModelGateway
    crates/app-infra/src/providers/offline/     # engine registry、gateway分派、资源校验
    crates/app-infra/src/providers/offline/worker/ # 父进程监督与私有协议客户端
    crates/translation-worker/                 # 候选独立Rust worker宿主；不等于批准其推理运行栈
    apps/desktop/src-tauri/src/platform/        # 窄Apple系统桥接/组合根
    crates/app-infra/src/storage/               # 后续新迁移/Repository，当前不改
    apps/desktop/src/features/model-config/    # 复用编辑器与Select，Rust目录驱动

### 5.17 资源版本、缓存、持久化与迁移

有效请求与测试身份在既有task/attempt/config/unit/语言/术语/处理规则基础上增加backendKind、engineId/engineContractRevision、engineConfig revision、worker build/私有协议版本（适用时）、实际ResourceSnapshot与完整route。显示名/安装目录字符串本身不代替资源内容身份；移动相同包与更换同名文件分别处理。worker启动握手和每次派发核对资源观察；资源变化不能静默重解释已应用配置。

Apple系统资源不能取得稳定版本时保存OS build、策略/API版本、语言对、可观察availability和本次observation epoch；不伪造package hash。不能可靠核对跨启动/系统更新资源相同性时不跨观察代次复用测试或缓存；历史保留“系统版本未提供”及当时观察身份，不将未知当恒定版本。

应用新资源产生新snapshot/config revision；旧历史仍引用当时不可变身份。独立测试保存探测阶段与真实翻译阶段，不含秘密，不写正文。配置导出只含可移植非秘密选项、engine key和资源需求，不带DLL/语言包/商业数据、许可材料、绝对安装路径、授权或测试通过；导入重新定位/检查/应用，不自动执行。

候选关系详见[数据库设计第15章](database-design.md#offline-translation-db-candidate)：独立offline_engine_configs、资源快照与EngineTarget关系；HTTP原表继续保留。新增codec先区分旧HTTP、前一轮AdapterBinding候选和本轮offline union，不给旧revision补字段重算seal。未知backend/engine/contract可读历史、拒绝执行；真迁移/严格字段与FK/XOR/GC/回滚另审，本轮不改DDL、schema-contract、迁移hash或用户库。

### 5.18 验证、依赖与未批准项

[机器目录offline_acceptance](model-service-registry-design.json)逐项登记OT-01～16，与PRD同ID，同为not_run；旧PA、MC CSV、153组合、历史包和词法验收身份不变。设计页的安装/就绪/译文为明确演示，不替代真实系统/API/DLL/语言包。文档检查只证明本批目录和规范一致。

| 验证层 | 应证明内容 | 前提与归属 |
|---|---|---|
| 文档/设计 | 13/21不漂移、4候选及3backend、条件字段、状态/路由/用途隔离；独立Playwright截图/交互 | 本轮候选；UI/QA；不能升产品通过 |
| Rust/私有协议 | tagged union、精确lookup、无HTTP/SQL/FFI越层、消息容量/错ID/旧epoch/迟到/崩溃/卡死 | 后续真实产品代码/fixture；AP/AI/PL |
| 资源/语言 | 合法包/安装、版本/位数/许可、完整性、缺包/移动/损坏；直达/逐跳中转；多句/长句/简繁/Unicode人工质量 | 每engine独立真实资源；AI/QA；Argos先PoC与技术锁 |
| Apple原机 | 15/26/26.4符号矩阵、同策略语言availability、许可拒绝/取消/缺包、断网与session生命周期 | 受锁SDK、实际macOS/签名host；PL/QA |
| Windows/CrossOver | 同位数worker进程确证、DLL ABI/编码、退出与卡死回收、断网实际译文 | 合法商业安装和各自环境；AI/PL/QA；结果不互相继承 |
| 存储/迁移 | 新旧codec/seal、EngineTarget XOR、资源/历史引用、GC、导入重新定位、事务与真实旧库回滚 | 新DDL独审/PRE-14矩阵/实际adapter；M10/QA |
| 正式包 | 实际入口→保存/测试/应用/启用→手动与四游戏变化→历史/停用/恢复，绑定同包/资源/worker与平台证据 | 最终签名包/合法资源/SR/MO/Never7/樱之诗；各Owner/QA |

实施前未批准项包括：Argos生产运行路线及新增库/精确依赖、Apple最小bridge与不同OS host路线、商业DLL版本/ABI/许可与CrossOver可行性、worker协议/容量/取消预算、独立DTO与数据库字段/迁移/codec，以及候选UI状态。须按影响完成技术锁、契约/字段和设计审阅后实施依赖部分，不能把整体架构确认视为新增细节均批准。

所有真实引擎/平台执行为not_run；实际开跑后条件不足写blocked并保留原因。文档工具、浏览器演示、上游源码、成功探测、系统语言已安装均不能代替实际译文、质量、断网或正式包验收。



<a id="translation-service-selection-contract"></a>

### 5.19 配置目录与日常服务列表的两个投影

最新任务模型见[PRD第8章](../product/model-configuration-prd.md#translation-service-selection-prd)、[完整候选](../design/first-product-v1/candidates/translation-service-selection-20261030/index.html?scenario=reading)与[SS逐层测试](../development/translation-service-selection-tests.md)。候选pending_review，本轮仅文档及设计模拟，不将新交互、DTO、字段或运行能力写成已批准实施。

配置目录继续回答“能添加什么”，包含安装版本真正支持的protocol/profile/preset、字段schema和离线引擎候选；只在添加/编辑服务时展示模板。日常服务列表回答“我已经配置好什么”，由Rust配置服务从当前profile已添加、已保存的HTTP模型配置与独立OfflineEngineConfig生成只读投影；不把目录模板直接映射成可选项，不读取窗口未保存的表单，不另造第三套资源注册。

日常列表项的候选投影包含稳定资源ID、显示名、family、保存revision/configVersion、用途、available/status/reason及状态来源。LLM要求显式templatePurpose为translation并满足translation能力；精读-only配置过滤掉，离线仍仅translation。显示名和服务位置不能决定用途。HTTP字段合法且用途支持时，未知连通性/未测试或旧测试过期不等于不可选，可标未验证并允许明确选用；当前版本已知失败、撤权、停用及字段非法阻断。离线沿用5.13～5.18的本机条件、资源、运行路线、语言对和必要能力门禁，未知必需条件不能放行。

可选性规则放app-core纯校验，由组合根注入协议目录、已保存资源视图和平台观察端口。UI仅呈现Rust原因码及可理解的恢复动作；Tauri做窄命令与DTO适配，UI不查SQL、不取密钥、不直接HTTP/FFI或探测本机。原型中的list/selection等JavaScript方法只演示此投影，不能成为新的业务权威。

### 5.20 显式选用命令、不可变版本与作用范围

选中服务经显式BindingCommand的activate意图进入应用层：引用已有保存版本，校验profile generation、预期binding revision、scope与资源当前可选性后，原子创建并激活新PurposeBinding版本。它复用配置版本、用途解析、调度和取消端口；不是保存整个编辑器草稿或自动应用所有用途。BindingCommand为本轮候选合同命名，当前源码的SaveConfiguration/ApplyConfiguration等现有命令不因此变成已具备该细粒度能力。

概念输入例如：scope=game、gameId=当前游戏、purpose=translation、main=某model_config@已保存revision、comparisons=有序去重引用、expectedBindingRevision=当前值。资源目标继续复用5.13的EngineTarget联合，HTTP不复制为离线资源，离线不虚造Connection/模型ID。app-core冻结connection/model或offline_config、凭据/资源、profile/schema/route等实际依赖版本；保存共享连接的新版本不重解释旧绑定。

命令保留已有purpose enabled；启停走独立明确的用途/会话控制。全局/游戏覆盖继续复用现有scope解析，恢复继承是明确命令；激活后发布包含source和binding revision的同一多窗projection，HUD与当前会话复用选择器和application-port。版本冲突或资格变更时拒绝本次激活、重取投影并保留原绑定，不“最后写入者胜出”。

主项一个、对照有序且不能重复同一资源；主项改变时从对照中移除相同目标。去重以稳定资源身份及其引用关系判断，不能按显示名去重或通过给同一配置换名绕过；不同真实配置仍可各自作为引擎。原REQ-024多引擎独立来源、进度、失败和并发预算保持。

### 5.21 后续句生效、取消隔离与验证边界

激活新绑定后仅后续句自动使用它；当前句不隐式重发。当前句重译是独立命令，冻结unit/text_revision与新binding revision并保留旧结果。保存资源、测试样句、打开或取消选择器、配置往返均不创建activate意图。

切换按原任务合同取消旧绑定排队/在途任务，递增相应配置/授权代次；流分片、最终提交、缓存和历史写入均核对profile/session/unit/text/binding/task/attempt及engine/resource身份。旧绑定被取消后迟到结果不能写成功或冒充新绑定；切换前已存在历史保留原来源。不会因为新翻译失败自动回原服务或切云端；回退仍需既有显式策略。

去配置上下文只携带origin、scope/game、purpose和资源ID等非秘密路由信息；设置窗返回后重取已保存列表，不自动选新保存项。多窗projection订阅继续使用6.3既有revision/generation缺口重同步机制，不由HUD或主窗各自保存当前业务绑定。

本轮不新增DDL，也不修改数据库字段候选、旧codec/seal或迁移。若后续细粒度激活需要持久化扩展，先证明既有版本/关系不能承载，再单独字段及迁移审阅。SS-01～12独立产品计划全部not_run；既有HTTP13/21、离线4、PA12、OT16、MC22及153组合不改。设计Playwright验证只覆盖模拟状态；真实组件、Rust竞态、存储、系统/服务及最终同包四游戏分别留run记录。

<a id="flat-service-session-contract"></a>

### 5.22 平铺呈现、条件字段与独立区域

最新任务规则见[配置PRD第9章](../product/model-configuration-prd.md#flat-service-management-prd)、[会话PRD](../product/reading-window-session-prd.md#flat-current-session-prd)、[机器目录flat_presentation_design/flat_acceptance](model-service-registry-design.json)与[FL测试计划及功能审计](../development/flat-service-session-tests.md)。[新完整候选](../design/first-product-v1/candidates/flat-service-session-20261030/index.html?scenario=reading)待评审，本轮只修订文档、设计与测试规划。

配置管理从同一已添加服务列表选择资源，同页呈现适用连接/引擎条件、模型/语言与样句测试，类型仅作标签/分组，不再作为family导航或五个实体Tab。该管理列表可包含不完整草稿，与5.19的日常已保存可选投影分别处理；同一资源ID/版本继续复用M04权威。HTTP Connection/Model/PurposeBinding与独立OfflineEngineConfig保持；传统服务不虚造模型ID，离线不虚造HTTP字段。字段schema、能力和原因码仍由Rust解释，UI不能通过隐藏字段宣布配置有效。

精读和语境查词保留独立用途区；翻译选用/启停复用5.20～5.21的浅层命令和投影。模板/参数、上下文、运行策略、密钥池放适用高级区，保留摘要和按需展开入口。保存、测试与选用仍分开；任何同页编辑不得自动激活全部用途、取用其他草稿或泄露凭据。故障返回模拟归页外评审工具，不是产品配置。

会话读取M03/M10既有正文单元、提取事件和脱敏Hook事件投影（M15负责诊断脱敏与验证；2026-10-31模块归属澄清），在同页三独立区域显示；各区的查询/筛选、页游标、滚动/跟随及新到计数分别持有，不能用一个共享activeTab状态互斥挂载或重置其他区。展示页大小不改变数据全集，搜索/导出覆盖实际声明范围，序号、身份、增量/合并关系、缺口及原生事实继续按RW-17～23校验。M12持有的会话预览不随区域展开或页面滚动重启；同页排布不授权新增采集、持久化台词诊断或放宽脱敏。

M13只做呈现与局部视图状态，M04/M05/M10分别持配置/调度/三数据集业务权威，M11持偏好，M15负责诊断脱敏与验收（2026-10-31模块归属澄清）；窄Tauri与独立Core/语言/离线worker边界不变，不新增DTO/DDL或第三套资源。FL-01～10只增量登记，设计Playwright与文档守卫不能替代后续真实组件、Rust时序/存储、实际服务/平台及最终同包四游戏运行。

<a id="guided-configuration-reading-contract"></a>

### 5.23 必要配置目录与当前阅读优先投影

依据为[配置PRD第10章](../product/model-configuration-prd.md#guided-service-configuration-prd)、[会话PRD](../product/reading-window-session-prd.md#guided-current-reading-prd)、[逐服务呈现目录](service-configuration-presentation.json)与[GD逐层测试](../development/guided-service-reading-tests.md)。本轮仅文档、完整候选和设计检查，字段/DTO/新服务执行与正式发布不因此通过。

呈现目录精确引用21 preset、13 protocol和4 offline engine，按品牌分组而不改精确tuple。每字段分类、类型、用户标签、条件和来源可核；多个接口必须明确选择，只有能力相容的用途进入引导。已核缺省有来源；内置固定地址/站点/应用OAuth资料未核时标独立资料阻断，不让普通用户补填。仅真正自定义服务器或资源要求才显现相应输入；模型样例不作默认。条件判断只控制展示，不能成为新的授权/执行权威，不注册脚本、wire映射或任意header。当前JSON与生成JavaScript只用于独立候选；未来仍由Rust同源目录生成typed projection，窄Tauri/UI不得直接联网/读SQL/取密钥/探测本机。

普通用户配置API Key/应用ID和密钥、服务要求的模型/项目/地区或本机资源，不输入credential_ref、schema/contract revision、salt/curtime/签名或token材料。秘密值只短暂进入系统安全保存端口，常规草稿/已保存资源持不透明引用；离线设计只模拟，不接收真实Key或伪造OAuth。自定义兼容地址与所需认证/模型在条件成立时必须可见且完整校验，仍使用明确协议/profile；未知字段或不支持认证在取凭据/发请求前拒绝。

精读与查词各持独立用途、版本、词句和策略，默认折叠不等于禁用或借用翻译绑定。高级模板/参数/网络/密钥池和运行策略按适用性呈现；保存/测试不切服务，激活仍引用已保存不可变资源并保持启停，后续句生效及取消/迟到/历史归属沿用5.19～5.21。本机沿用5.13～5.18健康门禁：必要API、路线、资源、语言对未知仍阻断，UI省略无关字段不放宽资格。

会话当前阅读卡直接读取当前unit/revision及独立译文/来源状态，首屏不由历史列表取代。M10原三数据集（M03接收/正文、M15诊断脱敏与验证；2026-10-31模块归属澄清）在后方各自默认折叠；每区查询/筛选/游标/滚动/跟随/计数与数据全集均独立保留，折叠不取消订阅、不丢提取事件或脱敏原生事实，不触发重新注入/选流/预览重建。警告/缺口摘要持续可见。当前卡与历史结果仍引用各自unit/config/binding身份，不为重排复制调度或历史权威。

本轮不增DDL、不改正式DTO或技术锁。GD-01～12独立产品计划`not_run`，旧PA/OT/SS/FL与153组合保留；设计截图/文档守卫只证明所选候选状态。200%整桌面壳裁切与入口可达分别登记，不能将后者冒充整窗适配。正式组件、Rust、持久化、真实认证/服务/本机、原生及最终包四游戏分层另验。

## 6. 文法分析与学习解释

默认点击后调用，不逐句自动分析或悬停即计费。第一次启用云解释时显示目标服务和发送范围；本地分词不会因此上传文本。解释面板冻结所选 unit/revision，即使游戏继续推进也保留“正在看上一句”的关系，用户可手动切换到当前句。

学习用途分层交付：首个切片提供本句文法、活用与词在本句中的含义；语气、敬语、省略和指代解释同时保留多种可能。后续可复用 ExplainProvider 做译文对照说明、按难度重述和自造例句，结果均须标为 AI 生成并与游戏原文分开。Anki 词释/例句草稿沿原 REQ-032 的制卡预览，用户编辑确认后才发送；模型不会自动写入词库或制卡。剧情/路线总结若后续引入，须另定上下文、剧透范围和质量门禁，不纳入首个文法切片。

建议结构化输出：

```text
AnalysisResult
  source_text_hash / unit_id / text_revision / token_snapshot_id
  summary（短句意说明）
  grammar_points[]
    label / token_ids 或 source_spans / explanation / alternatives
  omitted_or_inferred[]（省略主语、指代等推测，另列）
  uncertainty_notes[] / provider_profile / model / prompt_version
```

复合文法允许引用多个 token；例如「かもしれない」不要求与词法词边界相同。模型不能覆盖本地词性或造一个原文中不存在的主语跨度。UI 使用独立下划线/选中态表示当前文法证据，避免与词性底色混淆。可展示结构名称和通俗说明，不请求或展示隐藏思维链，不生成未经校准的百分比“准确率”。

应用层先校验结果大小、schema、拒绝/截断状态、token 是否存在、span 是否越界、是否对应绑定的原文，再发布文法卡。流式 JSON 可先显示“分析中”，不能把半个对象变成已验证结论。结构合法只说明引用可检查；日语分析质量仍须人工审校样本集，包括多义、口语、省略与模型错误。

若服务只返回普通文本，显式标“文字解释，未绑定逐词证据”，禁用精确原文高亮；不能伪造结构化成功。格式错误可有一次有界修复/重试，仍失败就保留错误，不无限循环计费。拒绝、限流、超时、流中断、格式不合法和语义不确定分开显示。

官方资料说明不同服务具有不同的结构化能力和限制：[Claude Structured outputs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs)、[Gemini Structured outputs](https://ai.google.dev/gemini-api/docs/structured-output)、[Ollama Structured outputs](https://docs.ollama.com/capabilities/structured-outputs)。本方案据此要求 capability 检查与应用校验，而非仅凭 endpoint 名称认定支持。

## 7. 并发、缓存、数据与费用

原文、词法、翻译、文法有各自状态。任务身份绑定 session、stream、unit、text_revision、task_config_revision；词法/文法另绑定词典和 token snapshot。切流、增量修订、换词典/模型或取消后，迟到结果不能更新当前句。仅因当前句推进而迟到、请求未取消且旧身份仍有效的结果，可以写回对应历史并注明来源；主动取消或禁用服务的请求不再写入正文/解释历史，只保留取消状态和可获得的用量元数据。

翻译按阅读事件调度；文法只分析用户选择的稳定快照。两者队列、并发和超时独立，学习请求不能饿死原文或翻译。输入无限变化时丢弃过时待处理分析，保留当前快照完整性与丢弃指标。网络流取消后停止显示和新重试，但不能保证上游已停止计算或不收费。

缓存身份至少包括文本、上下文摘要、语言方向、游戏术语/prompt/处理规则、精确协议contract/profile版本与provider/model/config版本；词法还包括 tokenizer/dictionary/schema，文法还包括解释深度/语言和 token snapshot。旧版本不能错命中。模型用量未知时显示“未提供”，不编造估价；费用预算/次数限制在本机可配置。

字幕、术语、词典释义都是不可信待处理资料，不是系统指令。模型没有 shell、文件、任意 URL 或游戏控制执行权限；解释页面不渲染未消毒的 HTML。凭据留系统安全存储。诊断默认不记录完整正文/请求体，导出需要明确内容边界。

## 8. 内置资源、许可与平台

基础分词器与词典随客户端发行，资源随应用整体升级。设置只需显示“内置日语词典”及只读版本信息，常规流程没有缺词典状态、下载按钮或路径配置。构建和发布门禁负责完整性、平台 ABI、离线首启和升级验证；未携带基础词典的包不能作为正式产品交付。

MeCab 与 UniDic 分别登记来源和许可。UniDic 官方区分不同许可的资源，计划只选择明确适用的现代日语版本并附 AUTHORS/对应许可。[UniDic 许可说明](https://clrd.ninjal.ac.jp/unidic/faq.html)。基础中文释义路线确定为中文维基词典日语条目的固定快照与离线索引，保留来源和CC BY-SA归属；JMdict仅作明确标英语的补充。精确版本、覆盖范围和许可材料须过B0门禁并通过离线样本审校；形态词典不直接提供中文释义。这是原资源选型阶段的边界说明；当前已锁定资源与真实离线状态按implementation-status和实际包回执核对，本轮服务设计不修改资源。

模型运行器的接口兼容性、模型的任务能力、权重的许可分别登记。翻译专用模型只作为用户自配服务的适配点，匹配其模型/prompt版本；不随产品默认下载或捆绑未经确认可分发的权重。文法任务必须另验证模型能力，不能仅凭它能翻译就启用结构化文法。

macOS 原生学习 worker 与 Windows 学习 worker 分别打包和实测；CrossOver 只用于 Windows 游戏的取字 broker。本地分词不需要放到 bottle，也不扩大 Luna Hooks Core 职责。现有 Core 仓库与许可证不因本次设计修改。

## 9. 设计与回归门禁

设计预览覆盖关闭/开启、仅注音/词性、选词、文法、内置词典信息、设置、浅深色、长文、200%字号以及模型请求失败状态。截图和交互检查只证明设计稿可查看，不代表真实分词、词典质量、模型正确性或原生焦点已通过。原游戏前台不夺焦点、置顶、穿透与键盘导航须在最终 Tauri 包另验。

新增 CR-12 验词法与显示，CR-13 验服务、上下文、文法、异步和隐私。除了合成/授权样本，还要用最终包在 SR、MO、Never7、サクラノ詩 10th 复测；SR 中文版应走非日语降级而非强制假名，日语三款至少核对五个变化状态和所选词片段。Ever17 仍排除。

词法质量不以“返回了 JSON”验收：原文还原、位置、读音/词元、未知词、标点/emoji、跨行 ruby、重复词和资源更换必须有样本。LLM 则分别检查真实调用、流式/错误恢复、引用合法性，以及人工审校的解释质量；不编造质量分数。正常闭环须无静默错配；故障用例可因正确隔离/报告而通过，不能外推为完整正文或正确解释。

新增验收细化见[补充需求](../development/client-learning-extensions.csv)，实施依赖见[任务表](../development/client-rebuild-tasks.csv)，回归见[用例矩阵](../development/client-regression.csv)。本轮新增协议/注册/凭据/迁移的产品回归仍为not_run；既有设计确认与旧运行回执保留原身份，不继承为本次新增范围通过。
