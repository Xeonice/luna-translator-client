# 2026-10-21 实施授权与精确 Owner

本记录在本轮产品源码编辑前建立。用户已明确要求修复五项真实客户端缺口，封面采用本机资源优先、无图时保存正式关联游戏首张有效帧；首版客户端只连接已经运行的游戏。旧批准 PNG、token、原始 PRD、旧运行结果保留。当前 R30 是修改前基线，任何后续 authority 指纹更新需要独立审阅与差异记录。

## 日志实施切片

需求 REQ-052 / FP-12 / RW-17～23；设计 `index.html#main-content` 的当前会话 Hook 日志；模块 M02/M10/M13/M14/M15，组件 C36/C33。Owner：诊断实现 worker。

允许文件：`crates/app-infra/src/hook_backend.rs`、`crates/app-core/src/ports/runtime.rs`（只修正安全合同注释，不增加未经审阅字段）、`crates/app-infra/src/storage/queries/session_records.rs`、`crates/app-infra/tests/session_records_batch.rs`、`apps/desktop/src/features/reading/SessionRecordPanel.tsx`。沿用现有 HookStageInput、message 和 metadata 投影，不新增依赖、数据库字段、Core ABI 或协议。若实现确需额外文件、公共字段或接口，先报 Root 审阅并补本卡，不能自行扩大。

原生内容必须有界、剔除控制字符并脱敏后才进入用户可见/持久化诊断；保留 Ready、Error、Log、HookInserted、InjectionFinished 的真实字段。Warning=1/EmuWarning=2，未知 kind 保留。默认诊断视图滤掉纯 raw 保存通知，但完整数据可选择查看，不能在已接收范围中删除原事件或削弱完整性。测试必须覆盖字段保真、Unicode/u64/控制字符/凭据脱敏、真实级别、默认降噪与全量分页/导出。

## UI 实施切片

Owner：Root。允许文件：`apps/desktop/src/features/library/LibraryScreen.tsx`、`apps/desktop/src/features/library/styles.module.css`、`apps/desktop/src/features/capture/CaptureSheet.tsx`、`apps/desktop/src/features/capture/WindowPreview.tsx`、`apps/desktop/src/features/capture/styles.module.css`、`apps/desktop/src/features/reading/ReadingPanel.tsx`、`apps/desktop/tests/visual/verify-ui.mjs`、`apps/desktop/tests/visual/verify-library-binding.mjs`。封面只走后端安全资产端口；统一页头连接入口；核对资料与目标选择同面板，一次明确确认接入，显式选正文。候选预览生命周期与 Rust 持有的会话预览不同，不以 React 卸载清理 session lease。新增端口等待资产架构精确合同，UI 不先写绕过实现。

## 规范同步切片

Owner：Root，或后续指定文档协作者。允许当前 PRD：`docs/product/README.md`、`first-product-scope.md`、`game-library-window-prd.md`、`reading-window-session-prd.md`；当前设计：`docs/design/first-product-v1/{README.md,app.js,game-library.js,window-preview.js,session-records.js}`（只修改实际存在文件）；技术：`docs/architecture/reading-window-session-technical-design.md`。验收 CSV、设计行为检查及冻结指纹需单独按实际受影响内容追加清单并独审，不机械重写全部权威文件。

## 当前完成边界

前置差异诊断与用户选择已具备，日志与 UI 切片可实施。资产/预览后端尚待架构报告后的精确路径授权。源码编译、行为、视觉候选、正式新包和四游戏仍为 not_run；静态方案不构成产品通过。不提交、不推送。

## 精确路径勘误与设计验收同步

不存在的 window-preview.js/session-records.js 不修改；实际相关源为 `docs/design/first-product-v1/review-gaps.js` 与 `review-gaps.css`，授权规范 Owner 同步。另授权 `docs/product/game-library-window-acceptance.csv`、`reading-window-session-acceptance.csv`，保留 ID 和 NOT_RUN/not_run 计划状态；设计执行器 `scripts/report/verify_game_onboarding_cases.cjs`、`verify_interaction_gap_design.cjs`、`verify_first_product_design.cjs` 仅按本轮明确用户 override 更新流程断言，保留拒绝/取消/身份/全量等原语义，旧源码前镜像可追溯；不得降低视觉阈值、自动重拍批准图、移除失败状态或把设计通过提升为产品通过。

## 诊断过滤查询合同补充

授权日志 Owner 修改 `crates/app-core/src/application/session_records.rs`。查询专用 stage=diagnostics 仅用于 HookEvents，SQL 在分页、count、搜索和导出前一致排除 info 级纯 text_* 保存处置通知，保留 warning/error/丢失；实际持久化 stage 枚举不变。不把 diagnostics 伪装成事件事实，不修改 DTO/DDL。全部事件 stage=null 仍为全量，过滤与总数文案必须明确。

## 资产/预览架构审阅与实施切片

Root已对照现有端口、预览服务与用户选择审核 `.build/hook-lexical-retest-20261021-010406/asset-preview-design/implementation-plan.md`（SHA256 257f60d57499cf67b512fa280ee1d419dd4f549c2e40b5baeeaa5d3c01b7f258）。批准按该最小方案实施，不增加依赖/DDL/CSP权限、不复制设计素材、三种身份hash区别保留。Owner为asset_preview_architecture；精确文件由同目录ownership-and-paths.json的assetPreviewBackendOwner existing/new/generatedOnly确定，另允许提取game_launch.rs安全路径逻辑以及application_runtime.rs精准回归。Root拥有application-port.ts、tauri-port.ts及ipc/queries.rs的启动用户端口退役。只允许对应职责变更，冲突文件按函数分区协调。

业务守卫与CAS在Rust，平台仅ImageIO/原生采集，UI只接字节一次并Blob展示。封面固定源8MiB、8192/边、32MP、输出960/长边/512KiB、最小160x90、profile托管256MiB；容量/格式负例必须真实执行。prepare I/O不持Hook绑定锁，commit窄CAS须绑定租约/代次再核。会话预览不能因React卸载或隐藏主窗停止；正常断开/解绑/退出清理，候选租约独立释放。结构变更后需另一Owner独审，完整门禁与最终包实测前不得标通过。

## UI 配套与受控传输

Root追加允许 ReadingPanel 的 `styles.module.css` 适配三页签外常驻预览列，`apps/desktop/tests/support/design-port.ts` 只实现新增窄端口的可控测试传输与真实行为断言输入，不进入正式包。`attachWindowPreview` / `detachWindowPreview` 只管理前端latest拉取，不新增Rust IPC，不停止Rust session lease；previewSession既有Rust查询负责实际生命周期。

## 真实组件门禁补强

诊断 Owner 完成日志后负责 `apps/desktop/tests/visual/verify-reviewed-reading.mjs` 与 `scripts/quality/reading_components.py` 精准扩展真实 React 组件行为执行；现有 RBC 语义仅按用户本轮覆盖的自动预览/默认诊断过滤合法迁移，并新增真实图像解码/封面每引用读取/同面板一次确认/无启动入口/会话预览跨页签常驻与完整日志范围断言，不能删除原有取消/竞态/迟到/分页等义务。Root将受控测试端口design-port.ts所有权转交该 Owner（其他Root编辑已完成），源不进入正式包。Root负责 `scripts/quality/product_execution.py` 的UI-57旧游戏启动行为ID迁移为本轮明确running-only行为，新旧对应留记录，不变零差异阈值/批准图/其他必需清单。

## 权威表的确定性生成同步

产品详规和原ID验收表已按用户明确覆盖修订，product_specs只读检查实际报告first-product/game-onboarding两份生成目标漂移。Root授权由现有product_specs --write确定性更新 `openspec/specs/first-product/spec.md` 与 `openspec/specs/game-onboarding/spec.md`；这是已改权威源的生成物，原PRD快照保持，不提升通过状态。不允许借此重写其他规范或刷新批准图。

## RBC新增义务的守卫测试

授权诊断/组件 Owner 更新 `scripts/quality/test_reading_components.py` 中旧固定38条计数以匹配本轮新增至45条，并新增缺少RBC39～45任一项、旧38回执不得通过的负例。不得仅删固定计数/放宽required集合或沿用旧回执。

## 独审故障的程序化回检

独审REV-01～06发现旧lease权限恢复、candidate传输失效与迟到resume资源泄漏、实际DLL/manifest事实缺失、封面来源未展示、接入pending期间可改目标。Root/资产Owner在既有授权文件修复，并新增RBC46/47到真实组件adapter，守卫必需集合增至47，缺其中任一项不得沿用45项旧通过。已有RBC45只验证UI附着同一个恢复lease，生产旧lease恢复由Rust状态机测试证明，不以测试端口制造新lease替代。日志资源事实从实际所选架构3个文件SHA/size核验取得，不把已请求注入当DLL已加载。

本change绑定既有product-ui-capture-v4及rust-storage-complete-v2真实执行器；新设计与旧批准对照产生的真实差异必须保留失败，不能自动替换图片。人工/原生/游戏尚不具备程序adapter，product_runtime_checks为空保留发布拒绝。

## 冻结前生成物及治理采用范围

Root授权按实际app-core依赖重编并stage `apps/desktop/src-tauri/resources/language-worker`（仅生成物，不改语言资源/manifest语义），避免封包准备再次改变源码闭包。精确治理范围为status JSON与三份确定性派生文档/CSV、RW coverage JSON的11条已授权预期、test-plan JSON源hash+7条GL预期及实际变化的executions CSV、technology-lock仅四设计源指纹、R30字节原档与R31 iteration-baseline、change内两R31审阅记录。生成候选不等于采用，必须在最终source map后由独审者核精确差异并Root授权采用；旧审批/历史结果不提升，新图不覆盖、既有零阈值不变。完整策略及16新增受控entry见.build/governance-reconciliation/strategy.md。


## candidate10实机缺口与第二修复切片（R32前置）

Root已读`.build/capture-initial-scan-diagnosis/association-mapping-plan.md`（SHA97ad7826f74831568cf85858358edd1bee75474631ab34242196e68b2a6bd660）并批准按`docs/development/iterations/FP1-20261021-runtime-followup/plan.md`实施。精确新增允许`crates/app-infra/src/storage/queries/capture.rs`，其余生产/测试路径沿上卡及既有授权。只读candidate身份和关联提交闭包是内部Rust port；四字段wire不变。NULL同会话补归属必须原子一致、不重注入、不改历史语言快照；错误不得断开此前仍有效会话。

存储当前指针候选新增精确允许`docs/architecture/database/v2/storage-acceptance.json`、`docs/architecture/database/v2/storage-review.json`，只限两chain与一个顶层source pin，须最终源独审与新记录后Root采用；旧嵌套review与原v1完全保留。新R31原档`docs/development/baselines/FP1-20261021-R31.json`只归档现a5f221原字节。Root扩真实组件/required/守卫至RBC48～50，不能让旧47回执通过。未经独审不得改视觉批准目录或自动更新视觉catalogue。


## 真实关闭互斥故障修复（RF-03）

独立受控复现已证明：fork子进程只持继承FD时，真实writer已join/drop仍不能立即重开同profile；原parallel聚合profile_in_use具体OS继承进程未采样，因果限制保留。Root授权repair_independent_review作为实现Owner仅修改`crates/app-infra/src/storage/database.rs`的租约RAII辅助与连接/guard字段所有权顺序及必要初始化；`crates/app-infra/tests/session_records_batch.rs`追加确定性真实Store fork继承关闭用例。必须记录acquirer PID，只有原owner Drop显式unlock，防child误drop解锁父；连接先关闭、再解锁，失败初始化也释放。原活owner/跨进程互斥、迁移/恢复/备份DDL与原断言不减，不sleep/retry或把聚合全改串行。如测试正确层需要其他精确文件先报Root，不新增依赖。Root和另一Owner独审该补丁，不能由实现者自审当通过。最终storage carrier纠正须同时解释本轮新增转发与已审RAII变动，不能继续称只有8行业务变化。


## StrictMode真实组件执行面

Root允许仅测试引导`apps/desktop/tests/support/bootstrap.tsx`提供显式受控StrictMode开关，默认所有原场景不变。RBC48实际重放CaptureSheet effect；DesignPort仅交付预设延迟响应，真实TauriApplicationPort另外验证单次native invoke、settle后可重试，不能在假端口实现业务再声称生产通过。此文件不进入正式bootstrap。


## 第二修复测试Owner与等价lint修正

Root将`apps/desktop/tests/visual/verify-reviewed-reading.mjs`中RBC48～50的实现独占移交repair_independent_review，Root不并发编辑；该Owner此前只写RF03租约，不写关联生产逻辑。本次只增加真实组件/真实Tauri适配器受控传输场景，原47义务保留。RBC48须独立React development受控构建实际证明StrictMode effect setup-cleanup-setup两次读取；原47仍使用原production构建，不能用生产StrictMode标签冒充replay。所有产物写.build，不改CSP或正式包配置。Root继续持有design-port.ts/bootstrap.tsx及REQUIRED50守卫。

现已授权的`crates/app-core/src/application/session_records.rs`查询验证条件由Root做De Morgan等价简化以修复实际clippy nonminimal_bool；diagnostics仅HookEvents可用的语义不变。资产Owner在原授权capture.rs/game_assets.rs将复杂查询tuple改为内部有义名结构，不新增DDL/DTO/依赖或allow。


## 完整聚合历史查询fixture身份修正

原完整并行app-infra回归在history_queries.rs四项以game_target_conflict失败，回执.build/storage-contract-followup/parallel-retest保留。只读定位为Database::game建立verified环境/EXE，而Database::hook(Some(game),不同session标签)却用session标签伪造另一环境/EXE；原历史断言并不要求跨程序绑定。Root授权asset Owner仅修改`crates/app-infra/tests/history_queries.rs`的Database::hook测试准备：Some(gameId)从真实snapshot该Game取环境/完整路径，None保持原独立目标，name仍作session/broker/identity区分。所有原断言、测试数量、分页/过滤/来源/迁移语义保持，不改生产守卫、不全文件格式化、不修无关末尾空行。原文件须保存并机械验证除该fixture窄块外字节相等。


## 启动程序提示与首次实际目标身份

完整存储回归新增失败提示必须区分启动路径与实际EXE。现真实importPrograms在actual canonical为空时也填launch PE的architecture；这是启动器提示而非已确认实际游戏身份。Root授权资产Owner在既有runtime.rs/capture.rs仅当非空actual canonical已经声明时严格比较既有architecture，实际EXE未知时以本次已核验candidate首次填值；环境/Bottle仍严格核对，任何非空actual路径及已确认架构冲突均保留拒绝。application_runtime.rs增加真实SQLite启动器位数与首次实际位数不同的正例，同时保留/补强已知实际目标错架构负例。无DTO/DDL/依赖变化，须Gov独审结论后落笔。

另拟精确允许`crates/app-infra/tests/storage_product.rs`两处测试前置迁移（等待Gov语义确认后才落）：hook(Some)取当前库真实环境及已声明actual路径、未知时保持原C:\game.exe fallback；confirmed_window_updates_real_target_and_preserves_launcher_arguments的输入明确canonical/pathKey均None、unresolved，launchPath/arguments及原全部更新断言不改。这代表UI真实导入的未知实际目标，不能只改unresolved但保留另一已声明路径以绕校验。备份原件并机械证明原断言/其余文件字节原样，原失败回执保留。


Gov已于本轮对上述启动器/实际身份两项窄修作只读独审同意，Root据此正式授权asset落地。Some(gameId)却找不到库记录时测试fixture必须失败，不得静默fallback；真实生产负例与旧所有assert保持。最终源码/载体审阅重新执行，旧11源review不自动继承。

## RF-04：窗口标题证明的安全收敛（2026-10-21）

本节承接Root明确授权及`.build/window-title-proof-api-lab/independent-api-findings.json`的42项跨进程受控实验与4项身份负例。MO实机只读观察为非Unicode HWND，旧W标题写入后遗留可逆nonce乱码；实验未证明真实MO WndProc内部实现或通用安全替代API。此前候选卡中的切换A/W、同位数helper及自动别名恢复不再作为本轮方案。

- 产品映射：FP-03/FP-05/FP-12、RW-01/02/03/04/05/06、REQ-052；模块M02/M12/M14/M15，组件C13/C14/C35/C36。预览失败与已核验Windows进程Hook独立；不能把未证明native归属标成已绑定。
- 生产仅精确授权`crates/hook-client/src/window_binding.rs`与`crates/lunahook-host/examples/target_probe.rs`及其同文件维护测试。非Unicode HWND在任何WM_SETTEXT前拒绝可变标题挑战，通过现有类型返回明确unavailable原因，不自动换A/W或位数重试。Unicode路径保留HWND/PID/路径/创建身份、唯一exact nonce、超时与恢复回执；恢复失败必须显式上报，不能吞掉后报成功。
- 不扩大公共wire DTO、native/UI、框架、依赖、DDL、容器配置或游戏窗口尺寸/焦点；不以相似标题、矩形、乱码别名绕过唯一nonce证明。不自动修写用户MO现有标题；实机恢复由Root单独控制。
- Root追加同一原allowed `crates/app-infra/src/hook_backend.rs`约151～164行窄错误映射，由natural_disconnect_fix负责：仅识别受控requires_unicode前缀，映射稳定code `window_title_proof_requires_unicode`与中文安全message，不透传raw；其它generic no_capture_window路径原样，无DTO扩展。
- 维护producer仅精确授权`scripts/quality/product_execution.py`与`scripts/quality/test_product_execution.py`注册并执行hook-client库测试、target_probe example维护测试；零测试、忽略/过滤、失败及源漂移均拒绝。guard夹具不等同产品实测。
- 复用既有`prepare_core_runtime.py`实际双架构构建/receipt验证，只允许生成`apps/desktop/src-tauri/resources/brokers/x86/target_probe.exe`、`apps/desktop/src-tauri/resources/brokers/x64/target_probe.exe`和`apps/desktop/src-tauri/resources/core-staging-receipt.json`。Core本体、ABI、native锁及语言资源不因本节自动授权变更；其它实际变化先交Root审阅。
- 治理Owner只修改本change、`docs/development/iterations/FP1-20261021-runtime-followup/plan.md`及上述两个producer；生产Owner由Root分配并独审。原source-snapshot、R32、批准PNG/token/阈值均不刷新；下一包须重新freeze/stage/build并按新源留证。状态implementation_in_progress，产品not_run。

## RF-05：退出后残留native active单独登记

SR进程已退出、UI已断开，但native broker仍占active并阻止MO接入，是独立于标题A/W问题的实机失败。初步修复义务为session-scoped ProcessDisconnected生命周期、有界异步stop/drain后以expected SID/target compare-clear；capture consumer不得等待自身DrainBarrier；迟到A诊断不得按latest写入B。精确生产路径与测试映射等待只读定位后补卡，当前不据此扩大修改范围或声称根因/产品通过。

### RF-05精确路径与执行授权补充

Root已授权natural_disconnect_fix作为生产Owner。只读定位证实ProcessDisconnected只改客户端snapshot、record_diagnostic按最新Hook会话改状态、backend active仍持旧目标，不能用UI强传confirmSwitch=true掩盖。精确允许`crates/hook-client/src/lib.rs`、`crates/app-infra/src/hook_backend.rs`、`crates/app-core/src/application/runtime.rs`、`crates/app-core/src/ports/runtime.rs`、`crates/app-core/src/ports/mod.rs`、`crates/app-infra/src/storage/database.rs`及`crates/app-infra/tests/application_runtime.rs`；优先同文件维护测试。这里的ApplicationRuntime实际属于app-core，不新增infra runtime目录。

本目标自然退出由supervisor有界停止owned broker，producer/relay/EOF与FIFO drain完成后，仅按expected SID与完整target compare-clear active/preview/锁。清理期间返回现有typed可重试状态，不占住consumer等待其自身barrier；仍活动的其它目标继续要求显式切换。生命周期与封句必须归属事件SID，不用latest；迟到A的断开/失败/诊断不改B。失败路径保留真实错误与状态，禁止sleep/retry掩盖/kill容器/忽略原FIFO与互斥断言。公共DTO/DDL/依赖/UI不扩；新增其它维护路径先报Root。实际producer复用product-hook-client-tests/product-core-tests/product-infra-tests及product-native-ipc-tests，正式包自然退出→跨游戏再接入单列not_run。

### RF-04实际WinAPI维护producer补充

Root另授权repair_independent_review仅新增`scripts/quality/window_title_proof_api.py`与`scripts/quality/fixtures/window_title_fixture.c`，check为`product-window-title-proof-api-tests`。实际命令为`python3 scripts/quality/window_title_proof_api.py --output <全新.build目录> --execution-nonce <执行器nonce>`，使用既有llvm-mingw/CrossOver，不改依赖。只创建自有随机类/PID/hash核验、offscreen(-30000)、NOACTIVATE、WS_VISIBLE fixture，不写/显示/输入真实游戏，仅fixture正常退出。挑战operator必须是真正本轮Rust target_probe双架构产物，不以C operator冒充；C只持有受控窗口及状态。source/EXE/build/call/log/nonce/逐项断言receipt须由执行器独立核验，仍不替代真实游戏/包验收。

### RF-04真实前置失败后的身份/安全追加决策

上述WS_VISIBLE+NOACTIVATE方案已实际失败：Wine仍使自有fixture进入前台。原attempt01保留，未据此调用真实游戏或宣称WinAPI通过。Root明确改为hidden fixture，严格要求visible=0、foreground=0，安全前置失败立即终止整轮，不放宽断言容许抢焦点。

生产并非仅为hidden测试放宽：真实窗口在挑战后隐藏/最小化/空标题时也必须完成安全恢复。Root仅在原两源内拆开discovery的visible/title过滤与proof identity，proof复用强bound_identity按根HWND、PID、完整image、FILETIME、冻结architecture/class全生命周期核验并严格读caption。枚举仍只列visible、native唯一exact nonce不变，hidden可恢复不等于支持hidden预览。helper内部CLI明确为`--window-title-challenge HWND PID IMAGE CREATION NONCE ARCH CLASS`；新增arch/class只属内部helper，非公共wire DTO。每次写入及恢复前后重核完整身份，不据相似标题/矩形放行。

维护receipt必须索引原始fixture/probe stdout/stderr/process.json与observations；执行器独立复算UTF16 caption hash、WM_SETTEXT计数差、实际ready/restored/error、fixture PID/HWND/class/FILETIME/loaded EXE hash、命令参数与真实DOS drive symlink映射，不只信case汇总。汇总与raw矛盾负例必须拒绝；合法空stdout/stderr保留真实空字节hash，不能造内容补齐。
