# 两项产品验收闭环

- work_item: IMP-ACCEPTANCE-CLOSURE-20261019；baseline: FP1-20261019-R13。
- 用户授权：对视觉和完整存储两部分开展验收、规划并使其真实通过；允许必要实现与测试，不提交或推送。
- 主责 root；视觉 Owner visual_acceptance；存储 Owner storage_gate_explanation；独立审阅 acceptance_context 与另行独立代码审阅。

## 需求、设计、模块与组件

- 视觉：FP-01/02 → 当前整稿 main-window/main-content 与游戏入库状态 → M13/M12，C08/09/11/12；FP-03/05 → connection-sheet 及窗口关联状态 → C05/13/14/15；FP-04/07/11 → HUD/词卡/偏好 → C01/06/16/17/18/19；FP-06 → settings/model-config → C07/10/26..31；FP-08 → learning → C20..22/34。
- 存储：FP-01/02/04/06/07/08/09/10/11/12，对应现有 STORAGE-01..22、431 字段与全部动态边界、PRE-14 十路线；M01/M03/M04/M08/M10/M15/M16。沿用当前 PRD、数据库字段/关系合同与既有公开端口，不增加产品能力。

## 实施与允许路径

1. 视觉先归类十五项截图：真实布局差异、比较尺寸/状态不一致、缺批准参照。输出逐项事实与候选到 `.build/acceptance-closure-20261019/visual/`。
2. 允许修复既有 `apps/desktop/src/{features,windows,ui}/` 中涉及上述组件的实现及 `apps/desktop/tests/visual/`、`apps/desktop/tests/support/` 的真实检查接入。UI仍仅依赖 ApplicationPort；Rust/公开契约不为视觉改变。
3. 存储新增 `crates/app-infra/tests/storage_product_acceptance.rs` 和 `crates/app-infra/tests/storage_acceptance_support/`；复用/增强 `crates/app-infra/tests/storage*` 与现有 credential_crash/disk_full/configuration_lifecycle/history/model_default 专项，保留原断言。实现真实 typed 往返、派生值/关系不变量和实际非空词典。
3a. 纳入 `crates/app-infra/src/storage/database_fault_tests.rs` 的 cfg(test) 私有故障验证及 `crates/app-infra/tests/{configuration_lifecycle,history_queries,model_default_templates}.rs` 的同次证据记录。保留旧断言，不改变生产 Writer 可见性；Cargo 同次 `--lib --test storage_product_acceptance` 真实编译运行库内与专用集成目标。
4. 存储接入限 `scripts/quality/storage_acceptance.py`、`test_storage_acceptance.py` 与专用 `storage_product_adapter.py`、`storage_product_process.py`、`package_identity.py`、`test_package_identity.py`。真实编译和执行单一 locked Cargo 目标，本次完整回执和关闭后的双库快照由独立程序再核验。
5. 根维护本 change 的 proposal/design/tasks/specs/verification；不修改原计划 CSV。受保护来源若因实际必要实现变化，仅在独立审阅后记录新的 revision，保留旧值和失败回执，不自动刷新 hash。

## 不变条件与待决事项

- 保护原 9a7ecf31 源码/包身份及原回归记录；新源码产生新回执，不继承旧包通过。原有许可证保留。
- 原始设计图、批准截图、token、零差异阈值、迁移 DDL/字段关系、431/22/10 清单不变。禁止贴图、假桌面、打印空 marker、删断言、拼旧日志或自动批准候选。
- 已知视觉基准存在完整桌面与独立窗/不同状态混比；先提供逐项比较契约与可评审资料。未具备可比批准参照的状态不得默认通过，也不得自行改变确认过的行为、视觉或验收对象。
- 未运行 not_run；规范比较待决单独记录，不伪装外部模型依赖；未通过不能宣称 49/49 或发布通过。

## 验证和完成条件

- 实际 Rust 存储目标与全部现有专项、Clippy（适用）、typed/边界/关系/非空词典/事务与故障/十路线；生产执行器独立读取本次候选库、schema、ledger 和版本身份。
- 实际 React Playwright 渲染、现有42+行为检查、每个视觉参照的同尺寸/同状态来源与差异图；原生材质结论不由浏览器代替。
- 五类负例、现行统一 `npm run verify -- --change implement-first-product` 全量复跑；成功必须有本次全部注册项回执（真实存储执行接入后会新增实际执行步骤，不固定以旧49项数量判断），任何仍失败项明确保留。
- 输出可离线 HTML 验收报告，列各项定义、计划/实际结果、身份、截图和可复现命令；独立审阅后交付。完整首版发布仍需其独立全部前置，不能仅由本工作项签发。

## 用户明确参照方案评审

用户已答复采用同尺寸/同数据/同状态的补充独立窗口内容对照，原稿/原PNG/零差异阈值保留；四项设计补充和原生控制/材质分开验收。精确审阅提案与SHA见visual-comparison-user-decision.json，登记时仍需独立技术审阅，不得用产品候选反生成期望或裁掉字段。

## 独立审阅后追加的真实缺陷与执行保护

- STORAGE13：真实miss矩阵已复现同文revision变化仍误用缓存。Owner storage_gate_explanation在tasks/cache/capture资源注册及app-core内部ports/runtime、SqliteStore writer转发中补固定版本化语义和显式重译Bypass策略，保留正常同文跨历史命中，操作重放不重发；无DDL/公开Tauri DTO变化。数据库迁移文件整体SHA若因writer转发变化，由独立审阅后新revision普通来源登记，旧值保留。
- 资源不可变声明须核对kind/version/manifest/contract/canonical metadata，版本不匹配真实拒绝并保留原引用。
- 同进程多窗口候选identity必须含真实HWND/窗口类，process-only单独domain；lease/session继续完整进程identity，避免不同窗口覆盖选择或绕过租约。
- 真实词典全量核验实测379秒，代码登记执行预算compile600/run1800秒；预算有界且守卫拒非法值，不减少任何源数据、断言或产品运行超时。
- Root在scripts/quality/package_identity.py和scripts/build_desktop.py加入签名保护的Resources/luna-source-identity.json（来源/Core/语言/版本），并加入包内luna-build-identity.json：记录本次编译源码、已签名EXE及Core/语言manifest身份，作为ZIP顶级成员避免与应用resource签名发生循环hash；外部build-manifest绑定整个ZIP/内嵌identity。验收前独立核源、EXE实际字节、Info.plist、资源锁及OS资源签名，拒旧包/文本/路径逃逸/重复成员/符号链接，inspection只在.build临时目录。它是本机可追溯构建身份，不冒充开发者证书或发布批准。
- 包检查器13项对抗单测仅使用合成ZIP和mock OS签名，不当作真实包通过；统一门禁增加package-identity-tests，正式执行必须使用实际codesign和新包。存储guard-only fixture明确mock包检查器，真实生产路径无跳过开关。

## 同态参照登记与配置页语义

新增补充目录仅为 `docs/design/first-product-v1/acceptance-v3/`；独立设计源、adapter manifest、相同数据和backdrop、15个稳定状态与完整连续滚动variant均固定hash。原previews不动；客户端捕获每个variant并由独立PNG解码程序重算零差异，漏状态、变体、字段或前置条件均拒绝。纯设计上下文示意图不冒充原生多窗运行。登记catalogue本身由新revision基线固定，禁止每次捕获自动改expected。

用途页保留“当前会话自动翻译”的独立即时操作，复用既有SetTranslationEnabled；无游戏会话时禁用并说明。该状态不冒充新增的跨会话持久化默认值；用途草稿enabled、运行许可、当前会话自动翻译仍分开，操作不保存或应用草稿。并列翻译保留REQ024的逐引擎启用、排序和参数。此视觉切片不签发全PRD产品发布验收。

## 同态检查揭示的客户端缺口修复

- FP-09/C23/C24：主导航统一原设计间距/颜色；会话侧栏来自真实game/session，空库不伪造Sakura；历史筛选/列表/元信息密度依设计调整，用户字号仍按真实偏好，分页/修正/迟到等既有逻辑不改。Owner root。
- FP-01/02/03/C08..15：资料仅名称必填，未核实路径不猜测；批次canonical目标为null则补充源也保持待核对。现有空白新增稿可接拖入；非空/批次稿不覆盖并在Sheet内提示。Sheet仍真实main内模态，补充设计需保留同数据父窗口与backdrop，不将产品父窗隐藏来匹配无UI底图。Owner library_capture_pixel_alignment。
- FP-06/AI-CFG01/10/13、C26..31：连接复制采用独立ID并显式选择凭据复用；连接/模型仅从草稿移除，列出引用影响/取消，保持旧applied不可变，保存通过既有SaveConfiguration端口。不会把多命令UI操作称作已应用配置的原子删除；完整applied删除与立即停用的原子契约仍是独立未完成发布用例。用途参数及测试/运行按原六段导航复用组件迁移，旧实际断言补导航而非删断言。Owner settings_pixel_alignment。

补充源修订仅纠正原型评审文案、真实输入/版本/可用入口、上述既有契约语义和比较环境；保留原设计与失败版本。不得按候选像素修改颜色/材质或删除PRD能力使其变绿。

## 原设计已有的启动与浮窗操作

- FP-02/M02/M12/M13：补`launchGame(gameId,rowRevision)`窄端口，Rust读当前游戏记录核身份/环境/路径/参数/工作目录，用无shell、有界的CrossOver launcher真实启动。只代表启动请求，随后仍须选择实际窗口并确认关联注入；无自动字幕成功。允许app-infra/src/platform/game_launch.rs及原application_runtime专项测试。
- FP-04/07/11、C01/C05/C09/C16：浮窗完整菜单复用真实重译/查词/配置/隐藏/断开；增加窄主窗history/capture导航意图，不能只打开主窗冒充直达。复用现Preferences.overrides bool键`reading.segmentationEnabled`(true)、`hud.compact`(false)、`hud.alwaysOnTop`(true)，不加数据库列。关闭分词只切呈现不改原文/token；紧凑派生16/18、22/24字号比及500/560宽比，保留用户原始偏好；原生窗口实际应用置顶/宽度。

所有新增UI仍只走ApplicationPort，业务身份/存储由Rust决定；公开窄端口接入main-only/允许role检查及实际编译/行为/参数负例。不以浏览器替代新原生动作的正式包验证。


## 统一执行前冻结（R14）

- 用户已确认补充比较方式；review7独立设计源15状态44执行变体（另1设计示意）经第三方45图重放、真实overflow覆盖和14菜单动作审核。正式登记catalogue仅改变注册状态与来源记录，原proposal与旧六轮失败/审阅全部保留。
- 同态实际预检57行为/44像素变体全部通过，sourceDrift为空；它仍为proposal预检，不能替代下述正式执行。
- 历史分页行为仍在包含前序Hook会话的原场景验证；其现场另存诊断图。正式历史像素捕获从独立空profile按同一手动输入/翻页用户路径建立，满足声明session=null/gameCount=0，不篡改实际游戏会话。
- HUD外壳改overflow:clip，阅读与菜单各自滚动，增加自动焦点也不得推动外壳的断言。减少透明度对popover/menu复用原设计可访问性实色回退；原token值不动。真实菜单动作走窄的Rust命令与原生窗口适配。
- R14为已独立审阅的新authority/tooling登记，保留R13；5项旧保护内容更新及新增保护项详baseline-review-r14.json。不声称所有R13原文都可逐行最小diff，不刷新legacy-debt或批准原图。
- 源码/资源预构建完成。最终实际结果仅记本轮外部运行回执/HTML；源任务状态不通过回填来制造成功，正式包source身份冻结后不改写源码以绑定自己的结果。

## R15 回执闭环修正

- 实际失败回执 `.build/quality/20261019T111716.813158Z.json` 保留：视觉生产端漏顶层 catalogue 身份字段，且存储编译闭包漏登记含库测试的 game_launch.rs；关联的两项存储守卫也正确失败。
- 需求 FP-04/FP-12、原设计状态和既有模块职责不变。Root 负责最小工具修复：UI producer 从同一独立 catalogue 写出顶层 SHA；完整存储 adapter 补登记实际参与编译的库测试文件。保留严格 consumer、闭包检查、全部命令、断言和零差异阈值。
- 允许路径沿本卡和 verification.json；新增精确保留 R14 基线的归档路径。独立评审 before/after 后记录 R15 普通基线修订，不更改原图、token、DDL、债务或审批，不手改旧回执。
- 完成条件：修复的真实组件回执、完整 storage adapter 与统一门禁均在重新构建的同源正式包上通过；旧包原生观察只保留，不继承到新包。实际状态由外部运行回执和最终报告记录，未提前勾选通过。

## R16 实际存储验收格式与拒绝路线修正

- R15实际统一回执 `.build/quality/20261019T113926.131496Z.json` 完整保留：51步仅完整存储执行失败，Rust真实28+118测试通过仍不能取代独立检查。
- 实际首条断言被libtest未换行的测试标题粘连，严格行首解析正确拒绝。仅修复真实输出formatter先换行，保留全部唯一标记/真实计数/库存检查；新增模拟无换行标题的实际格式测试。
- 冻结`reject_relationship_corrupt`路线要求坏关系原件被生产迁移拒绝且前后字节不变。原检查器错误地将该负例源库按健康候选检查。新增明确的拒绝快照检查：仍要求封闭只读、完整性和完整schema一致，并必须有实际FK违反；仅用于冻结关系损坏拒绝路线。普通候选仍拒绝任意FK违反，拒绝源另外核对冻结版本账本；矩阵、DDL、预期和审批均不更改。
- Owner：Root集成Python适配与守卫；独立协作者分别负责schema检查器/防线测试与Rust输出formatter。允许路径沿verification.json，新增精确保留R15基线归档。独审完整前后差异后才登记普通R16工具修订；不刷新legacy-debt或旧违规源码hash。
- 验证：格式精确测试；合法拒绝证明与健康候选差别、无FK/错误账本/错误错误码/健康候选坏FK等负例；全部旧守卫保留；重新打包并在同源新包上完整执行。旧回执不补字段、不改日志、不继承为新包通过。
