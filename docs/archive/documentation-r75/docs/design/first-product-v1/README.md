# 首个可用产品版：高保真交互设计 v1

## 2026-10-31：设计目标到客户端与准出的规划

本次最新目标仍为[guided当前阅读](candidates/guided-service-reading-20261030/index.html?scenario=reading)与[必要配置](candidates/guided-service-reading-20261030/index.html?scenario=settings)，源/manifest及设计状态保持不变，没有重拍或覆盖批准图。现行客户端尚有Tab、3协议/单secret与整体apply等差距，详见[客户端对齐计划](../../development/current-client-alignment-plan.md)及[新增测试/准出合同](../../development/current-product-test-coverage.md)。

设计层已有的交互断言和截图仅保留当次准确来源；后续正式React、Rust/存储、服务/本机、原生/游戏均独立执行。当前记录为pending_review，不因本规划更新设计批准。旧Tab/平铺稿留作演进来源，不再作为现行主表单或阅读布局目标。

## 2026-10-30 阅读优先与必要配置候选 · 待评审

[当前阅读](candidates/guided-service-reading-20261030/index.html?scenario=reading) · [服务配置](candidates/guided-service-reading-20261030/index.html?scenario=settings)

当前原文/译文形成独立首屏阅读卡，连接状态与预览辅助；正文历史、全部提取、Hook诊断放在阅读后方，默认折叠且数量与警告/缺口摘要可见。展开保留各区搜索/筛选/分页/滚动/跟随与新到计数，折叠不裁数据、不停订阅或重建预览。

配置先选择用途、品牌与明确接口/引擎，只显示该选择当前必要的密钥、模型、项目/区域/授权或本机资源。字段来自[21HTTP/13协议/4离线呈现目录](../../architecture/service-configuration-presentation.json)，固定协议、连接/模型内部关系、schema/revision和凭据引用不要求用户填写。内置固定或站点/套餐地址有出处并自动带入，资料缺口明确待完善/阻断，不转嫁给普通用户；自定义地址/认证/模型只在真正需要时显现，不能伪填。精读/查词独立折叠，高级模板/参数/网络/密钥池/策略按需保留。

状态`pending_review`，仅独立完整设计候选；此前R73平铺、服务选择及本机候选、canonical源、批准图/token和历史结果保持。GD-01～12及[新逐层计划](../../development/guided-service-reading-tests.md)全部产品`not_run`；浏览器样例不证明HTTP、OAuth、系统API、资源/DLL、持久化、原生或四游戏可用。200%整桌面Zoom裁切须如实记录，入口/键盘可达不等于整窗无裁剪。


## 2026-10-30 服务配置与当前会话平铺候选 · 待评审

[打开最新完整候选](candidates/flat-service-session-20261030/index.html?scenario=reading) · [服务配置](candidates/flat-service-session-20261030/index.html?scenario=settings)

配置页以统一已添加服务列表和同页条件编辑替代“引擎类型 → 用途/连接/模型/模板与参数/测试与运行”两层Tab；云端、本机非大模型与LLM使用类型标识，选中资源后展示适用连接/本机条件、模型/语言及样句测试。精读与语境查词保留独立用途区；高级模板/参数、上下文、运行策略、密钥池按需展开且有摘要。底层连接/模型/用途与离线资源、保存/测试/应用/选用/启停语义不变，日常翻译仍从浅层同一选择器明确选择。

当前会话将正文历史、原始提取（全部提取）和Hook诊断（Hook日志）平铺为同页独立区域，各区摘要、数量与入口同时可见，详情可按需展开；宽窗可并排、窄窗纵排。各区筛选/搜索/分页/跟随及新到计数互不覆盖，完整数据与导出范围不缩水，会话预览继续常驻。清理重复导航、对象不适用字段与产品区的演示控制；故障注入仅放页外评审工具，不删除审计/高级能力。

新候选状态`pending_review`；与此前translation-service-selection、local-offline候选、canonical完整稿、原批准PNG/token及历史回执分别保留。FL-01～10与[逐层测试和功能审计](../../development/flat-service-session-tests.md)独立登记，产品全部`not_run`。新候选交互与截图只证明设计样例，不代表真实HTTP、系统API、worker/DLL、持久化、原生窗口或正式包四游戏通过；实际执行结果由本change的独立回执记录。


2026-10-27紧凑预览确认：用户已明确“采用此方案，更新完整设计稿”，范围由[本次采用记录](../../development/design-approval-20261027-compact-preview.json)单独维护。本轮将[已确认的282×346紧凑方案](../../../.build/stability-refactor-20261027/compact-preview-design-v1/index.html)纳入[当前会话](index.html?scenario=window-preview#window-preview)与统一选窗面板，两处继续复用同一预览组件。主卡保留游戏、状态、16:9完整画面、原采集时间、身份摘要和常用动作；完整身份、路径、hash、诊断原文与隐私说明从真实详情入口查看和复制。仅详情正文滚动，主卡不整体滚动、不缩字、不缩放；宽卡依据组件实际宽度布局，不能按桌面视口误判窄侧栏。此确认只采用设计方案，不修改原批准PNG/token、旧候选或产品实机 `NOT_RUN`。

2026-10-21当前修复：用户明确要求本机封面优先，无图时只为正式关联的有效目标保存首张有效帧；首版仅连接已运行游戏，删除详情三重复入口及游戏启动入口；同面板核对后一次关联立即Hook、候选显式选正文；已有权限自动预览且会话跨页签常驻；Hook日志保留有界脱敏原生排障事实并默认降低raw噪声。见[本轮工作卡](../../../openspec/changes/repair-library-binding-preview-diagnostics/iteration-card.md)。状态 `implementation_in_progress`，本轮新候选不覆盖原批准PNG/token，不继承09旧工程或局部Hook为产品全通过。

日期：2026-10-19 · 六项交互修订及Select间距稿已于本轮获用户评审确认 · 原材质风格沿用 · 正式产品回归 `NOT_RUN`。

**当前唯一完整设计入口为本目录。** 窗口流程统一为：选择窗口或进程 → 核对 → “关联并接入” → 自动检查与接入 → 候选正文。同目标复用、不同目标确认切换，失败保留有效关联并提供重试；解除关联保留取文。当前五项用户覆盖已进入实施；离线设计只证明表达与交互，产品源码、正式新包和四游戏各自回归，不由本页提升通过。

[打开完整交互稿](index.html) · [截图画廊](gallery.html) · [使用旅程](entry-flow.html) · [视觉对照](comparison.html) · [首版功能范围](../../product/first-product-scope.md) · [功能覆盖表](coverage.csv) · [视觉规范](style-guide.md)

本稿对应用户确认的首个可用产品版：游戏取文、翻译、内置分词与词典、按需AI精读、模型配置。它包含现工程计划M1、M2、M2-L的产品能力集合，不将仅取原文的M1技术版当成完整首版，也不表示这些实现或发布门禁已经完成。


## 2026-10-30 翻译服务统一选择完整稿候选 · 待评审

[打开新完整交互稿](candidates/translation-service-selection-20261030/index.html?scenario=window-preview) · [从浮窗查看](candidates/translation-service-selection-20261030/index.html?scenario=reading) · [配置管理](candidates/translation-service-selection-20261030/index.html?scenario=settings)

当前会话的“翻译服务”与浮窗紧凑入口打开同一个选择器。日常列表统一展示已添加并保存的云端翻译、本机离线翻译和大模型翻译配置；内置待添加模板、未保存草稿及精读 / 查词专用模型不混入列表。字段不完整、当前版本已知失败或本机能力 / 资源 / 路线未就绪的配置显示禁用原因，并可准确进入相应设置。HTTP字段合法但尚未检测的配置标为“未验证”，允许明确选用，不能冒充真实可用性通过。

配置页专注新增、编辑、保存、检测与样句。保存不更换服务；返回时保留选择器来源与作用范围。选择主服务一次即用于后续台词，自动翻译开关保持；当前句独立重译并保留原历史。全局默认、本游戏覆盖与恢复继承清楚区分，附加对照去重；句子精读和语境查词继续独立配置模型。首用关闭状态须由现有“自动翻译”开关明确启用，无需再到配置页重复应用 / 启用。自动翻译关闭时，用户仍可明确重译所选单句；该请求独立捕获服务版本并保存新记录，不改变自动开关，服务切换、撤权或迟到结果不会覆盖其他当前句。

新候选状态 `pending_review`，不替换前版本机离线候选、canonical整稿、已确认截图或token。仍使用共享Apple磨砂材质、SVG图标、Select与游戏回放。各附加对照按服务独立保存设计状态、结果与错误；页外“单项对照返回模拟”可对指定服务注入完成、等待、失败或超时，主服务与其他对照结果保留。所有检测、译文与附加对照均为明确标注的页面设计样本；没有真实网络请求、系统API、资源下载或DLL调用，不提升任何正式客户端、离线、存储、四游戏或发布验收。文件与共享依赖身份见[manifest](candidates/translation-service-selection-20261030/manifest.json)，新旅程结果由本轮独立回执登记，不能继承旧17项旅程。

## 2026-10-30 本机离线翻译完整稿候选 · 待评审

[打开新完整交互稿](candidates/local-offline-20261030/index.html) · [直接查看翻译引擎设置](candidates/local-offline-20261030/index.html?scenario=settings)

本候选从真实主窗“设置 → 翻译引擎”进入，明确区分云端翻译、本机非大模型翻译与大模型服务。候选包含 Apple 系统翻译、Argos 专用机器翻译候选运行器、用户已安装的 J北京及金山快译软件 / DLL；不将 Ollama / LM Studio 混入本机机器翻译。引擎字段、资源 / 语言对、可用性、样句、草稿、应用、启用与中转确认分开，精读 / 语境查词仍只选择解释模型。

Apple 默认显示 OS / API / 传统策略未知；macOS 15 基础 API、26 的 headless / cancel 与 26.4 的 lowLatency 传统策略分别表达，highFidelity / Apple Intelligence 路线排除。Argos 默认显示候选运行路线未实现。能力未知、语言对不支持或运行路线未实现时不能进行样句测试、应用为可运行配置或启用；页外工具可显式模拟受审路线与传统策略已确认。启停按作用范围和引擎隔离，复制不继承设备目录、Bottle、资源路径或检测 / 授权 / 启用状态。

状态为 `pending_review`。新稿复用原整稿素材、材质、图标与自绘 Select，在独立目录新增入口和设置脚本；原 canonical settings / index、批准图、token 与历史确认保持原样。页外“设计工具 · 本机翻译模拟”可注入未安装、缺包、不支持、失败和超时；检查、译文、取消与迟到隔离只验证页面设计，不访问网络、不下载、不探测本机、不调用 DLL，不代表产品实现、兼容验证、真实离线通过或新的设计批准。候选依赖与文件身份见 [manifest](candidates/local-offline-20261030/manifest.json)。

## 本轮六项已评审设计：从这些入口查看

[六项修订与截图](interaction-review.html) · [专项PRD](../../product/reading-window-session-prd.md) · [34项待实施验收](../../product/reading-window-session-acceptance.csv) · [门禁为何漏检](../../development/design-review-gap-analysis.md)

| 反馈 | 设计入口 | 主要可操作内容 |
|---|---|---|
| 运行窗口预览 | [当前会话](index.html?scenario=window-preview#window-preview) | 自动常驻紧凑主卡、刷新/暂停、原采集时间与身份摘要；详情可查复制完整身份/原始原因/隐私；覆盖开启中、拒绝、退出、不可用及身份异常；画面为历史素材回放 |
| HUD拖动和快捷键 | [浮窗](index.html?scenario=reading) / [组件与恢复](index.html?scenario=components#hud-controls) | 拖手柄换位置；常用动作在浮窗、更多不重复；位置重置与设置快捷键为页面内演示，非系统全局注册 |
| 专用Select | [控件状态](index.html?scenario=components#select-showcase) / [设置](index.html?scenario=settings) | 自定义展开菜单、选中、键盘/取消、禁用和长标签，沿中性磨砂材质 |
| 当前会话全量内容 | [提取文本](index.html?scenario=session-texts#session-history) / [Hook日志](index.html?scenario=session-logs#hook-events) | 全部来源、原始序号、分页、筛选、跟随/暂停、错误/缺口，正文历史与原始事件分开 |
| 半透明与可见注音 | [仅注音](index.html?scenario=hud-ruby) / [减透明度](index.html?scenario=hud-reduced) / [词法失败](index.html?scenario=hud-lexical-error) | 半透底板/实底回退，分词/注音/词性独立，失败保留原文 |
| 组件和按钮差距 | [组件状态](index.html?scenario=components) / [原稿对照](comparison.html) | 同族按钮/图标/Select状态，候选与原确认参考分开 |

本轮新增画面只输出独立候选目录，保留原 `previews/`、批准PNG、material-tokens与acceptance-v3。R30作为修改前基线不自动刷新指纹；PRD/整稿变化产生的baseline drift明确待评审。旧51/51、15/44及正式包都不能据此成为新规范通过。设计检查只证明本页交互，不证明实时屏幕采集、全局快捷键、实际worker、原生材质和真实游戏。

## 高保真体现什么

不是设置表单和阅读卡片的页面合集。原型用完整macOS桌面表达游戏、独立HUD、主窗、设置窗、精读窗的关系；连接/选流是主窗上的附属面板，短词卡锚定原文，完整学习保留独立窗口。设置侧栏左上提供“返回主界面”，切回主窗并保留编辑草稿和已应用版本。主窗关闭后游戏与HUD继续，可从菜单栏恢复；隐藏、暂停、翻译开关与断开分别操作。

连接、模型和用途各有独立实体与引用。新增、复制、切换、保存、测试、应用和停用影响当前所选对象；全局/游戏覆盖与已应用版本分开；样例测试按对象和用途记录，不把一次连通演示扩成所有能力通过。修改配置后，旧翻译/精读请求取消，原句和历史来源不被改写。

## 建议按这条主线评审

1. **首次打开与添加**：默认空游戏库，HUD等待输入；选择或拖入程序，核对名称、环境、Bottle、启动/实际目标路径后添加。添加只建立记录；四款已有记录的演示可从 `?scenario=existing-library` 打开。在线用途尚未启用，须在设置明确应用。
2. **选窗口并接入**：连接运行中的游戏 → 从窗口或进程选择 → 核对宿主与Windows目标、Bottle/EXE/架构 → 一次确认“关联并接入” → 自动检查与接入 → 预览候选 → 选择正文 → 开始阅读。已有同目标会话复用正文和历史，不同目标先确认切换；失败可重试，解除关联保留取文。临时选流与记住选择仍分开。
3. **游戏上阅读**：游戏画面、原文和译文对应；原文先到，随后展示译文进度。暂停、隐藏、关闭翻译、重新选流、断开各自保留正确状态。
4. **点词与精读**：点日语词语先出现本地词卡；从词卡或更多菜单打开独立学习窗。模型未配时带着原句去设置，应用后返回，用户再开始；游戏继续推进不会替换旧句。
5. **记录与修正**：从主窗历史查看对应原文、译文、模型来源；重译保留旧版本；人工修正保存失败仍保留草稿。历史句可以独立打开精读。
6. **无游戏输入**：手动输入可以使用同一翻译/历史流程。可用样例为`data.js`中的手动日语句或已采集的游戏原句；任意未收录输入会保留，不伪造一条无关译文。
7. **偏好与恢复**：在设置切注音/POS、舒适/紧凑、200%字号、减少透明度；诊断区查看连接、游戏退出、服务失败及数据中断的不同恢复路径。

页外评审栏中的“推进台词”回放既有配对画面，故障菜单注入失败条件；它们不是正式客户端控件。模型和游戏主旅程从实际窗口里的入口连续点击，不能只切换一张成功截图。

## 真实场景与设计数据

| 游戏 | 实际目标与语言 | 配对场景 |
|---|---|---|
| サクラノ詩 10th | `sakuranouta10th.exe` · x64 · `Sakura-no-Uta-10th` · 日语 | 5个画面/原文状态 |
| Memories Off | `Game.exe` · x86 · `Steam` · 日语 | 4个一致画面/原文状态；未用跨页合并的第1条 |
| Never7 | `GameN7_Steam.exe` · x64 · `Steam` · 日语 | 5个状态、3个可观察页；保留同页追加关系 |
| Symphonic Rain | `SR.exe` · x86 · `Steam` · 简中 | 5个画面/原文状态，不冒充日语学习证据 |

19张游戏图与对应原文来自此前本机Core独立探针的机器可读配对记录。图片原样复制，来源、样本与SHA见[素材清单](assets/manifest.json)。这次没有启动游戏或采集新回调，旧证据不提升新Rust包的回归状态。

译文、词法标注、释义和文法解释为本次设计人工样本。它们以真实源文本为对象，token加空白/标点可逐字还原；不是实际MeCab、UniDic、释义资源或模型输出。未实现通用翻译与分词算法，不把任意输入的设计样本当成真实服务能力。

## 视觉与交互基线

本轮以用户确认的[暗色磨砂案例](../macos-material-case-v1/index.html)作为首版风格，统一整个窗体、内层表面、文字、控件、菜单与提示。准确规则见[视觉规范](style-guide.md)，颜色和材质由[共享tokens](material-tokens.css)维护，图标由[统一SVG族](icons.js)维护；设置去彩色底块，控件与链接采用中性色阶。默认暗色，浅色为同系列适配。

- 连续暗蓝灰磨砂覆盖整窗，内部使用较厚的灰色表面；不再沿用大面积白色正文或青色导航分区。
- 表单保留13pt正文、23pt设置标题和十类单层导航；业务密度、入口和数据关系保留。
- HUD舒适560pt/18pt/24pt，紧凑500pt/16pt/22pt；操作行固定32pt，200%正文可滚动。
- 游戏截图自身标题栏只出现一次。HUD、主窗、设置和精读可独立叠放和拖动。
- 系统减少透明度、浅深色、大字和小屏独立检查。浏览器表达不等同原生焦点、快捷键、置顶或VoiceOver已验证。

[视觉对照](comparison.html)将已确认案例与整稿主要窗口放在一起，并保留改版前主窗和原SCR-01/08/15资料。案例风格确认不等同整稿布局或实现通过。

## 维护与证据

[既有Playwright检查](previews/verification.json)保留其当时源文件哈希、连续操作、截图、控制台和HTTP请求，不作为2026-10-27修改后源码的通过证据。紧凑方案的独立设计检查保留在[冻结候选回执](../../../.build/stability-refactor-20261027/compact-preview-design-v1/evidence/qa.json)；完整稿整合须另行运行原设计旅程与交互检查，命令、源指纹和实际状态汇总至[本轮执行回执](../../../.build/stability-refactor-20261027/compact-design-integration/worker/final-tests.json)。该整合回执由本轮实际执行生成；不存在、未执行或失败时均不能声明整合通过。

原关联并接入、同目标复用、目标切换确认、失败重试、解除关联及配置、词卡、精读、历史旅程继续保留。[视觉对照](comparison.html#window-material)保留已确认案例与整稿的对应关系。所有浏览器检查只属于离线设计，不执行20条GL、22条MC或111条产品回归；实际模型、凭据、词典、持久化及正式包仍须研发后独立验收。

```sh
node scripts/report/verify_first_product_design.cjs .build/stability-refactor-20261027/compact-design-integration/worker/existing-journeys
node scripts/report/verify_interaction_gap_design.cjs .build/stability-refactor-20261027/compact-design-integration/worker/interaction-design
```

从工程根目录运行，使用工程已有的Playwright/Chrome，显式指定新的独立输出目录；不默认重拍 `previews/` 或运行会重建历史清单的旧发布脚本。[既有离线ZIP](../../../reports/first-product-high-fidelity-20261018.zip)保留历史快照身份，不代表本轮整合后的完整稿。当前原型仍可直接打开首页，不依赖开发服务器。

交互稿为普通HTML/CSS/JS，模块约定见[integration.md](integration.md)。旧字幕和模型配置专题已经进入[历史归档](../archive/README.md)，不再并行维护；原产品快照和已确认材质案例保持原位。

## 2026-10-19 设计确认身份

用户已确认“刚刚的设计稿进行 review 是没有问题的”。[确认记录](../../development/design-approval-20261019.json)逐项绑定当时的16份交互源及37张已评审PNG，旧指纹和批准图保持原样。2026-10-27按新的用户确认更新当前紧凑预览设计源，另行记录采用与整合回执，不回写该历史确认。设计确认不提升真实客户端、原生采集/拖动/快捷键、词法管线或发布状态。新增技术细节另见[专项技术设计](../../architecture/reading-window-session-technical-design.md)。
