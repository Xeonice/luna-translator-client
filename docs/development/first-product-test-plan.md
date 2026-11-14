# 首版逐层测试与上线准入规划

当前guided完整源版本已由用户明确审核通过，身份见[源版批准记录](design-approval-guided-20261031.json)；其他页面按[全页与状态闭包](all-page-design-coverage.md)逐项对齐。设计源批准、独立参照、真实组件和正式包验收分别记录；原29个阅读/配置增量状态继续保留，不能代表整套页面。

本计划面向 macOS arm64 客户端通过 CrossOver 接入 Windows 游戏。实施与验收目标为 [guided 当前阅读](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading)和[必要配置](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=settings)：当前句独立优先，历史/全部提取/Hook 诊断后置且各自默认折叠，配置按用途、品牌、明确接口和必要字段引导。完整设计源版本已获用户审核确认；文档、工程守卫或历史执行记录均不授予当前产品对齐或发布批准。

## 完整绑定旅程与顺序准入

[绑定旅程机器计划](binding-journey-plan.json)固定BJ-01～18，来源为FP-03/05/09/12、现行GL/RW及已批准目标面板/候选交互，累计于原153、RW、70来源/576观察与901设计义务之外。当前CaptureSheet的22态全部缺独立参照，C13～15为partial/source_only；绑定流程仍未完成。

| 用例 | 必须可观察的行为 | 层 |
|---|---|---|
| BJ-01 | 同一目标核对入口与系统确认位置；选择本身不注入 | component, native |
| BJ-02 | 生产错误状态与首次确认动作；准确提示需要系统确认 | domain, component, native |
| BJ-03 | 候选到会话移交单窗口选择；保留原选择filter/owner和核验 | component, native |
| BJ-04 | 无意图时不弹系统选择器；无意图调用为零 | component, native |
| BJ-05 | Rust身份复用切换与过期审查；同目标明确复用且不重复注入 | domain, component, native |
| BJ-06 | 真实operation和Core阶段驱动；阶段只依据真实事实 | domain, component, native |
| BJ-07 | 面板关闭隔离；M12独立Core-active订阅；S1待handoff→S2停旧会话前失败仍保S1时预览最终建立，旧callback零且新候选不被动 | domain, component, native |
| BJ-08 | 待确认多选与显式开始阅读；勾选不发提交或窗口命令 | domain, storage, component, native |
| BJ-09 | 保存选择与临时策略；临时不覆盖或清除既有saved | domain, storage, component, native |
| BJ-10 | 完整样本与高级来源；u64以规范十进制字符串原样传递 | domain, component |
| BJ-11 | schema5原子集合与旧库迁移；001-004原字节与历史FK/trigger保持 | storage |
| BJ-12 | 集合成员消费和双链自动资格；两个当前成员均获合法资格 | domain, storage |
| BJ-13 | 同authority原子选择→HUD可见成功→Main隐藏；before真实Main身份、completed最终OS与loss-free milestone顺序分别证明 | domain, component, native |
| BJ-14 | C1排布和目标核对可达；按批准源核区域和滚动归属 | design, component, native |
| BJ-15 | 真实帧、时间与撤权；新成功帧才更新时间 | domain, component, native |
| BJ-16 | 最终同包四游戏与真实翻译；同源同签名最终包和真实PID/资源 | game, service, native |
| BJ-17 | 失败恢复、数据边界与清理；保留真实状态、草稿和旧成功数据 | domain, storage, component |
| BJ-18 | 顺序和证据防绕过；写前非作者回执且精确源未漂移 | guard |

原P1r2/P2r2与首次P3已经真实准入；r3先用原守卫实际回执和非作者封存已授权P3全图/产品图及原23/16字节，再plan-only独审→gate-only fresh32hex同源真实语义RED、原五类与全部续写负例→非作者审阅→新目录独占准入。plan-only/gate-only产品不得漂移，旧签字不授新oracle。编译/浏览器/脚本基础设施错误不能算合格RED；若真实产品失败已消失，必须重新独审等效合同，不制造RED或默认接受PASS。阶段准入仍保持产品incomplete。

观察合同采用：唯一sourceId可见来源行/复选框，不因高级详情重复Hook名而误定位；pending核真实面板/单次请求/无重复提交；候选阶段由当前operation真实事实和同一Rust来源集合的可用复选框/开始/稍后控件证明，Node与Python/保留DOM同义独算。BJC05的有限范围保留，只有同源/同nonce/同run的完整BJS10独算通过才可展示discharged_by，不能全局忽略blocked。

BJ11私有cfg(test)故障保留父integration与子libtest的实际编译/list/exact/二进制/PID/时间/DB链，并消费真实故障arming与native码。BJ13改用before真实Main身份＋completed最终OS/HUD原图＋同authority完整事件commit<show成功<hide成功；不要求定格同步中间瞬间，不改时间或截断watermark。仅BJ13采用该观察形态，BJ14整页Native独立参照和零RGBA阈值完全不变。

运行资料：真实组件需固定Playwright与Rust可编译依赖；存储需真实冻结Store4及原测试源/编译锁闭包、schema4备份副本、独立v5完整合同与故障注入。历史4继续验原81表/734字段及升级/备份/故障，当前5另验83表/748字段、14新增字段/5FK/6trigger/当前成员view与0～4→5/reopen；原v4比较器仍精确拒绝5。实际SQLite/ledger/字段/route原件按同nonce独立复核，不能只改版本预期或借complete-v3执行器宣称完整v4/v5通过。Native需同证书包、可见桌面/系统单窗与录屏能力；四游戏需合法资源/可推进存档及真实服务地址/model。原件维护在fresh `.build/binding-journey-alignment-20261109/`，用例定义不回填通过。

## r3可执行补充合同与责任

P1r3初次计划独审`attempt-01`为`changes_required`，原回执/发布前后图原字节保留；本次覆盖的是待审计划，不签准入、不执行产品。完整机器义务见[binding-journey-plan.json](binding-journey-plan.json)的`execution_obligations`与`resume_epoch.observation_contracts.storage_parent_child.production_timeline`。所有新增登记均为**planned/not_implemented/not_run**；P2只在原16个门禁文件注册/消费，P3必须等fresh准入后才可在原63个实现文件增加真实writer/测试。文字负例、登记、构建或guard自测均不能关闭产品义务。

| 固定义务 | Owner与真实执行路径 | 原件与独立判断 |
|---|---|---|
| EX-LIB-GLOBAL / EX-LIB-SCOPED | UI＋AP/HK；原38组件scene全部保留，新增`BJS-01-library-global-owner`、`BJS-01-scoped-reconnect-owner`；现有Rust `binding_journey::bj_component_transport`＋生产MainWindow/CaptureSheet | 实际SqliteStore/Runtime `SaveGames`保存A/B，返回gameResults与初始snapshot；DOM选A，页头全局入口选运行B应默认既有B。seed后save_game/save_games为0、确认前associate/attach为0、确认后B恰好一次；scoped恢复保留真实意图。Python从实际Rust snapshot/target/review上下文独算ID，不接受自由expectedGameId或单game样例。 |
| EX-LIFE-CORE | AP/HK；扩现有`cargo test --locked -p app-core --test binding_journey bj_07_domain -- --exact --nocapture`，原断言保留 | 分别实际S2资源/target-lease停旧会话前失败、S1仍active，获取fresh current-preview authority；profile/server/full active witness竞态的回调必须0。窄authority仍待P3实现，不是现成API。 |
| EX-LIFE-NATIVE-UNIT | PL；coordinator现文件内固定`platform::session_preview_coordinator::preview_cancellation_tests::bj_07_active_session_preview_reconciliation`及机器合同另四个exact test；实际`cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --bin luna-reader <固定完整名> -- --exact --nocapture` | 真实M12独立订阅Core-active，受控低层barrier/collector ack，不挂UI、不靠直接ensure。旧handoff0、新candidate不变、陈旧witness拒绝；busy/release重复singleflight，healthy/paused不重启，无window/无明确授权不picker。编译artifact/list/exact/源/nonce/PID/原日志和真实事件表由Python独算；不冒充OS。 |
| EX-LEASE-NATIVE-UNIT | PL；WindowPreviewService现文件内固定`bj_07_nonblocking_retirement_admission`、`bj_07_old_retirement_cannot_release_new_candidate` | 真实2active＋1draining/pending原预算、精确ticket/owner/Fence、guard进出与enqueue、实际release ack/admission顺序；guard内不能等待Stop、不能旧ticket释放新candidate。 |
| EX-STORAGE-TIMELINE | Storage实现、QA门禁消费；实际source_selection_sets父integration＋database私有cfg(test)子libtest | Scope创建≠API入口，arming≠native故障；当前AfterDdl备份时间0是缺证，不能过。P3在生产open/备份落盘完成/事务/DDL/native失败/return及回读处补观察，按route适用性与严格事件次序核验；缺失用null＋not_reached原因，不用0或父relay顶替。 |
| EX-NATIVE-SUCCESS-ROLES | PL/Root真实签名包；原固定名`bj_08_native`、`bj_09_native`（当前未实现），实际app-infra Native writer | 成功beginReading后SourceSheet已卸载且Main已隐藏，committed/temporary/saved采真实HUD＋此前识别Main隐藏＋原DB/journal提交权威。BJ09保存意图/保存后检查另以正常显示Main→重开文本流只读观察，禁止重注入/重提交/改set/重复原效果；不能重开掩盖原窗口效果失败，重开draft为false合法，saved须真实remember=true及saved_snapshot。temporary要真实非空保存基线。 |
| EX-NATIVE-PROGRESS-TRACES | PL/Root真实签名包；固定`bj_06_native`（当前未实现） | 固定waiting-observed/fast-candidates两条清单，按实际当前operation/source事实与OS区间独算选路。waiting实拍等待→候选，fast只要求实际当前候选，历史握手/连接/source顺序由loss-free账本证明，不强造四张中间截图或产品延迟；无首次progress原件不宣称首次轮询，域/真实React迟到与过早阶段义务不减。 |
| EX-LIFE-SIGNED-NATIVE | Root/PL/QA；同源最终签名包BJ07，另同包四游戏/真实服务 | Native-unit不能代OS。保留原五句/款、Never7跨三页、预览/Hook/词卡/翻译与退出恢复、原153/RW/901全部独立义务。 |

P2任务`R3-02-EXEC`/`R3-02-STORAGE-TIME`/`R3-02-NATIVE-OBS`先落精确registry、raw字段、独立消费和缺失/篡改拒绝；新产品producer缺失时仍blocked/failed，不因注册而绿。P3任务`R3-04-EXEC`/`R3-04-STORAGE-TIME`/`R3-04-NATIVE-WRITER`再实现真实producer并以相同源/nonce原件复验；完整P1/P2独审和新准入前不可写产品。LIB01～05、LIFE01～08及LEASE01～04均逐项绑定机器义务，原76定义不删改；Native十四类反例另累计，BJ14整页零RGBA与全部存储不变。

## 累计验收集合与当前阻断

| 集合 | 保留的身份与范围 | 独立验证要求 |
|---|---|---|
| 原回归与首版计划 | 原111条CR定义保留；首版CR55+MC22+GL20共97来源，另31机制定义，合计128定义/153执行组合 | 原计划JSON和CSV不回填；`spec_contract.validate_release`核对全部153个 `case@variant` |
| RW阅读合同 | RW-01～34及设计、组件、管线、语言、原生、游戏六层 | `interaction_coverage`独立检查，不由153或设计样例抵扣 |
| 存储专项 | 原用例绑定的schema、字段边界、迁移及故障恢复子验收 | 真实Rust生产adapter及实际SQLite/Repository回执，不以设计DDL或字段字典替代 |
| 当前业务增量 | PA12+OT16+SS12+FL10+GD12+GUI8，共70来源/576观察组合 | [机器登记](current-product-test-coverage.json)保留原断言、层、Owner和精确变体，独立侧录验证 |
| 当前设计对齐 | guided与R79完整组合及适用状态、61份精确设计源及组件/需求/Owner映射 | [对齐合同](current-design-alignment.json)要求当前目标真实React和正式包执行、独立参照及确认；旧v4无当前能力 |
| Windows原机资格 | OT-08/OT-16×J北京/金山，共4项 | `separate_platform_qualifications`独立保留 `not_run`，不属于当前Mac同包576项，不可删除或挪入其他义务 |

这些集合累计且互不替代。当前guided与R79完整源版已获用户确认；现有4态独立参照获批且有相应有限真实React能力，其余状态与完整Native层仍未齐，已执行像素/有效render失败不因注册能力消失，产品对齐与发布仍为 `blocked`。开发产品UI变更也必须通过当前目标能力检查，不能仅声明旧 `product-ui-capture-v4` 得到放行。静态合同可以通过，不等于产品执行通过。

## 1. 唯一范围与可执行登记

首版以[FP-01～12](../product/first-product-scope.md)、[游戏库与关联PRD](../product/game-library-window-prd.md)、[模型配置PRD](../product/model-configuration-prd.md)、[AI精读PRD](../product/ai-reading-prd.md)、[当前设计](../design/first-product-v1/README.md)和[架构基线](../architecture/first-product-engineering-baseline.md)为准。对应16模块及当前组件权威表，沿既有端口测试，不能为了测试给前端增加HTTP、SQL、FFI或凭据旁路。

机器登记在 [first-product-test-plan.json](first-product-test-plan.json)，它保留原验收来源和内容身份，登记层级、职责、环境、前置、步骤、可观察预期及证据。供人工执行的[组合清单CSV](first-product-test-executions.csv)与[需求追溯CSV](first-product-test-coverage.csv)从登记生成；手工改表会被检查拒绝。

- 原111条CR原样保留。其中M1/M2/M2-L的55条属于首版，M3的56条 `CR-11-REQ-*` 留作后续完整原需求复验。
- 首版继承 **97个验收定义 = CR55 + MC22 + GL20**；另补 **31个机制级定义**，合计128个定义，按预定环境展开为 **153个执行组合**。一个定义多种环境分别有结果，不把重复计数当功能覆盖率。
- 新增TP不是新增产品功能：展开SQLite/Keychain崩溃点、多窗一致性、协议分片、取消竞争、Unicode、语言质量、权限与正式包恢复等现有承诺。
- 12项FP、16项AI-CFG、全部原97个来源及M/C均可追溯。**登记齐全只证明计划无遗漏，不证明实现或测试通过。**

支持范围为macOS arm64客户端，通过CrossOver接入Windows游戏，包含实际Rust PE32/PE64 broker。四款样本为SR、MO、Never7、サクラノ詩10th；不测Ever17。Windows原机分发、Mac Intel和原生macOS游戏取字须单独规划与验收，不从本机结果外推。OCR、语音、Anki、文件批量翻译、对外Web API、内嵌回写及后续AI场景不进入首版；原路线保留。

## 2. 九层验证路径

层号表示粒度和集成范围。依赖针对正在交付的垂直切片，不要求先完成全产品所有低层用例才能验证B1原文包。冻结契约后，领域、ABI/协议、存储网络和组件分支可并行；进入正式包时收齐该切片所依赖的分支。最终完整首版须收齐全部必需组合。

| 层 | 核心问题与代表用例 | 使用的真实依赖 | 退出证据 / 主责 |
|---|---|---|---|
| L0 规范、资源与工程前置 | 范围/设计/技术栈/目录/模块有无偏离；资源来源、许可与测试政策是否冻结；TP31 | OpenSpec、实际源码/锁文件、构建与资源清单 | 来源与追溯检查、实际编译回执；资源/政策批准另验。QA协调，各模块Owner负责 |
| L1 领域规则与状态机 | 身份、合法重复台词、分段、配置三态、语义指纹、缓存与人工修正；TP10/11/16/27 | 纯规则及受控时钟，可替换端口 | 正常/边界/负例断言、可重放种子；不证明网络/DB。AP、AI |
| L2 ABI、IPC与跨进程契约 | Core ABI1七回调的真实Rust双架构调用；JSONL/DTO/worker、字符串寿命、Unicode位置；CR02/04、TP12/17/18 | 对应PE32/64、同Bottle受控Windows目标、真实进程通信 | 两架构回调/字段全宽、借用字符串即时复制、损坏/超限/乱序拒绝。HK、JA、AP |
| L3 真实基础设施 | SQLite事务/WAL/迁移、系统凭据、锁、HTTP/SSE、真实词法worker；TP01～05/09/13/15/25/26 | 临时真实DB/Keychain/文件锁/本地HTTP socket/实际语言资源 | 提交与故障恢复、脱敏请求字节、进程终止/有限重启、离线资源证据。AP、PL、AI、JA |
| L4 应用编排 | 关联即接入、复用/切换/取消、投影snapshot竞争、慢窗重启、旧请求归属；TP06～08/14等 | Rust应用用例与前置已验端口；受控故障时序 | 不变量与可观察结果共同验证；多窗无第二业务权威。AP、AI |
| L5 组件、视觉与可访问交互 | 完整状态目录、轻返回、灰阶图标与毛玻璃、ruby/长文/200%；CR10-VISUAL、TP22/28 | Playwright运行实际产品组件；批准设计为基准 | 基准/候选/diff、键盘/可访问名称与人工结论；设计HTML不算产品截图。UI、PL |
| L6 正式包与原生系统旅程 | 非源码启动、四窗口、无游戏翻译/查词/精读、入库与选窗口、持久化、真实服务；全部GL/MC、TP21等 | 最终RC、原生WebView/系统窗口、真实服务与内置资源 | 正式包录像/截图、真实请求与写盘恢复；浏览器结果不能证明置顶/穿透/焦点。QA统筹，各Owner |
| L7 四游戏端到端 | 逐款原文→译文→历史→学习→故障恢复与断开；CR03/05/13-FINAL/07-SOAK | 同RC、四款实际游戏、实际Core与服务 | 游戏/客户端/原事件/阅读对象配对，逐款结论；SR中文分支单独判。HK、QA、AI、JA |
| L8 质量、稳定性与分发 | 高频压力、语言质量、隐私、干净安装、签名身份、受支持迁移和回退；TP19/20/23/24/29/30 | 同RC、真实语言资源/模型、干净用户/设备、隔离恢复样本 | 预冻结政策达标；安装与恢复可复现；与L7共同支撑B5。QA、PL、JA、AI |

历史与停止会话继续由`verify-history-layout-refinement.mjs`、`verify-stopped-session-layout.mjs`执行真实WindowRoot/现有Port受控流程。通用设置现行执行为`verify-general-settings-structure.mjs`：固定Playwright、私有Vite、生产组件，1000×720/760×560深浅及200%，核三组首屏/内部滚动/保存区、单层标签与真实Select指针，该18项脚本核标题字重700与General局部字体回退链；新增必验义务为侧栏footer实际贴底位置/37px导航行高及3px间距、8组合heading/scroll/三卡/footer几何，本轮由`.build/general-visual-closure-20261108/review/ui-leaves/`独立Playwright执行，不能冒称18项脚本已覆盖。200%还须比对排布，不能只查未溢出而漏掉错误窄容器分支；原始V3到永久源仅两条URL迁移，byte-only、8画布、保存游戏态及无mask/threshold0义务保留。Chromium同URL新文档已能复现24RGBA，原39RGBA迁移失败亦保留；不可重跑挑图。G01的source-only观察器改由公开WKWebView执行9组、每组2新文档×2固定帧，须经非作者原件校准审核；这只校验设计源迁移。A另用真实WindowRoot/隔离端口与独立源在公开WK分别完整截图，固定Playwright仍实跑键盘、指针、布局；B另在签名包的私有有限组合根逐项取得原始OS截图、进程/签名/ACL拒绝及正常退出，不能重用A图。两层均为14状态/27固定capture（含首次无缓存、展开菜单及200%自然滚动末尾），整窗0RGBA，未执行或非零不得通过。Native在观测和所拥有窗口核验后，对所有状态及参照/候选统一固定等待3秒，再保留原两帧间隔与整窗采集；检查器独立核实际单调时钟和OS/命令区间。不得按差异重拍、挑帧或改变阈值；原4个滚动条淡出非零案例保留。 原ICC/显示器属于Native验收环境：每批27项绑定同一primary及完整窗口包含、三阶段只读probe和原PNG iCCP；环境不同提前拒绝，不做颜色转换。环境锁同时核实际parent cohort与完整成功前缀，失败批次不能挑case续跑。原跨ICC正式54条像素拒绝与同屏诊断0分别保留，后者不追认前者通过。参照生产工件和最终候选工件记录不同epoch，有限参照renderer/source闭包必须相同，最终候选绑定当前源码与包。测试涵盖scope加载/迟到、脏草稿阻切换、字段mask及全局后续变更、恢复只改draft/放弃保留/保存CAS失败、有效签名不可见的active-B mask情形回到前台/focus/visibility后重新核对、同签名在途focus失效不丢和卸载清理、POS禁用保值/ruby独立、历史/会话导航pending/错误隔离、真实MainWindow消费后的最终页面（不能只验请求队列）：无/停止/活动会话的session均无选窗，已有capture弹层须关闭，capture与history旧意图保持原语义，revision去重/迟到重放按既有权威核对、OS状态读取/写后回读/异常详情及卸载迟到。测试输入来自永久captured设计源、两依赖以及精确三叶before fixture，不依赖开发机旧.build；在blank root复制声明输入后重建原布局、实开两canvas验证干净环境可用。它明确是memory-only端口，不授DB/Native系统注册通过。 本轮General14态清单及缺口见`.build/general-visual-closure-20261108/context/minimum-closure-matrix.json`：已批准源重放、独立参照审核、真实React、同包Native须分别完成，当前未据此授予验收；loading/error/save_failed与Native参照容器已由用户分别确认，见design-approval-general-states-20261108.json；确认仅授源和参照环境，不授实际层通过。独立WK宿主只能证明native_component_render，不能冒充packaged_native或实际OS/DB行为。

原`verify-settings-layout-refinement.mjs`与其五项/所有原件完整保留：它绑定旧三页补充的“阅读行为/内置分词与词典/24px组距”和前一前像，相关排布前提已由用户新结构批准替代，不能把旧五项绿当当前UI通过，也不能删断言或硬把新实现变回旧布局。独立开关、POS保值、scope/保存失败、200%保存区等原义务由新执行器继续覆盖；新用例与旧已执行结果分别记录。测试定义迁移由代码固定新批准、旧脚本原hash/字节、旧5项与新check及保留义务，不接受JSON任意skip。runner先实际执行新检查并核TAP非空、命令/退出码，再记录旧定义not_run_superseded_design，不能记旧5项passed；新检查失败或未执行则迁移合同失败，既有失败不被清除。旧SC07/08接收控制的责任由当前会话真实pause/resume承接，SC09～12登录读取/串行写/回读/失败义务保留在新流程；旧全页adapter未补新参照的部分仍阻断。共享Select在200%须真实鼠标选首尾及scope，不用键盘绕过屏外选项。源authority负例只证明批准/迁移/映射身份，不授组件、Native或产品准出。

L5阅读差异的维护测试包括`reading-typography.test.mjs`（默认角色层级、独立自定义及有限正数边界）、`verify-reading-ui-alignment.mjs`（七动作、控件归属、单/多/人工修正来源、流式一致、滚动和旧词卡归属）和`verify-live-preview.mjs`（真实视口/HMR/隔离/公开同数据模式）。各自通过只证明对应受控行为。随后使用锁定Playwright按1040×720与760×560同数据截图，保留未遮罩零阈值差异及源指纹；原Main未实现富文本状态须显式报参照缺口，不能关闭真实ruby、造全量计数/采集图或覆盖批准图取得绿色。Native字号、材质、标题裁切和同包游戏另验。手动页另由`verify-manual-ui-alignment.mjs`沿真实组件验证直接编辑/常驻结果、Hook无gameId不冒手动、保存失败与跨导航草稿、源语言及真实payload、清空与任务取消分离、固定手动结果/词卡/精读及宽窄几何。`verify-settings-navigation.mjs`另实际运行Rust序列化/非法目标及revision单元，再验证首次/现有/早事件/乱序/重复、搜索与脏编辑同页和真实离页确认；未运行/零测试/ignored不能通过。候选包再验证实际分类/同窗/最小化和草稿往返。原UI-04断言及原fixture保留；原稿缺少的紧凑服务、可编辑语言和富文本结果组合状态单列参照缺口，不用受控测试替代真实服务和Native。

HUD顶部间距由`verify-hud-header-spacing.mjs`以隔离生产组件和独立设计源验证：同尺寸前后截图、上下6px、舒适/紧凑/窄窗/长服务名/空态/浅色、拖动与按钮命中分离、正文滚动时栏位固定。选择器继续走真实共享弹层，测试不改服务资格或Rust偏好。浏览器请求拖动不代表Native移动；同签名候选包另观察实际HUD。新组合缺机器authority/批准像素，完整产品准出不因该局部通过而提升。

推荐次序：L0 → L1/L2/L5并行 → 对应L3 → L4 → L6 → L7/L8并行 → B5发布评审。L5需要冻结DTO，最终视觉放行需要整稿正式批准；不能因材质已确认自动通过全稿。

## 3. 对应研发门禁，逐段形成可用产品

| 门禁 | 要收齐的能力及层级 | 可以作出的结论 |
|---|---|---|
| B0 | 测试登记、当前设计批准范围、资源/中文辞书/许可、工具链、性能/质量政策及支持设备/分发策略 | 具备相应研发与测试前置；工程守卫绿灯不能单独通过B0 |
| B1 | L1身份/原文规则 + L2真实新Core双架构 + L4接入 + L5必要界面 + L6正式原文包 | 新Core原文技术版；不是完整首版 |
| B2 | L1/L3存储与历史 + L4阅读投影/窗口控制 + L5/L6原生多窗与无游戏来源 | 可持续阅读与保存，尚不能宣称翻译/学习均完成 |
| B3 | L1配置解析/调度 + L3协议/秘密 + L4在途归属 + L5/L6真实配置与服务 | 传统、远程LLM、本机服务以及模型配置各自可用 |
| B4 | L2/L3实际语言worker/中文辞书 + L4固定学习对象 + L5/L6词卡与按需AI | 内置离线语言能力和按需学习均可用；M2-L实施仍遵循既有M2-12前置 |
| B5 | 最终RC上的L6复验、L7四游戏与L8质量/稳定性/分发，全部必需组合通过 | 完整首版发布候选，仍须范围、设计、缺陷和实际证据评审 |

每个切片先写OpenSpec场景与适用测试，再实现并沿层验证。低层失败先修问题代码，不靠改期望或硬编码通过。修复后重跑失败场景和共享边界的影响组合；最终RC重建或签名/公证处理改变字节后，更新hash并重跑全部必需正式包组合。

## 4. 测试环境与数据怎样准备

原[机器登记](first-product-test-plan.json)冻结18种环境及18种有意义的变体；当前增量另按70来源合同展开，避免无意义的全笛卡尔积。后续增加OS、架构或provider承诺须先改计划，再增加必需组合。

- **构建来源与隔离**：同一实际工作区依次执行`cargo check --workspace --locked --offline`和`cargo test --workspace --no-run --locked --offline`，再运行适用用例；库编译不能代替测试目标编译，编译也不能代替运行。独立冻结副本、旧源码与修复后epoch必须各用独占`CARGO_TARGET_DIR`，不得复用Root或另一副本的target；记录Cargo metadata中的manifest/target/src_path及实际extern、dep-info和源码hash。来源不符或运行中源码变化的绿件作废，保留原件并重新冻结复验。
- **隔离性**：使用专用临时配置档、DB、Keychain前缀、日志和受控目标；故障注入只能针对测试副本及自有worker/broker。不得杀游戏、整个Bottle或其他会话；保留Steam另一台电脑连接。
- **双架构Core**：锁定独立 `luna-hooks-core` 的 `e9242ebac32a09e911f306b7c90714891c217302` / ABI1 / 七回调，分别记录PE32/64 broker和Core产物hash。旧十回调GUI与独立C++探针不能替代Rust正式客户端回归。
- **可控HTTP**：真实本地socket注入任意分片、拒绝、429、超时、网络断开、重定向/路由、取消和截断。它只证明协议及故障时序；真实OpenAI兼容、Anthropic、Azure传统翻译、本机兼容服务分别在RC中完成。Azure仅翻译及不支持AI的准确降级。
- **语言资源**：从实际包读取MeCab/UniDic、中文基础释义与JMdict英语补充的版本/hash/许可；断网首启，不允许固定样例和隐式下载冒充内置资源。词元回退、未知词、资源损坏和worker崩溃分别验证。
- **固定语料**：授权或自制，版本化且人工审校。覆盖活用、复合文法、口语、省略、敬语、否定/时态、人物实体、专名未知词、合法重复台词、增量片段、标点、组合字/emoji/UTF8与UTF16差异。保留可接受解释范围，不要求LLM逐字输出唯一答案。
- **数据恢复**：DB迁移、WAL备份、磁盘满、只读、进程崩溃与凭据pending/撤销失败使用真实隔离I/O；迁移范围在B0声明。没有正式前版时，验声明schema与候选升级，不能默认支持任意PoC数据。
- **本机显示**：记录支持OS、CPU、内存、CrossOver版本、Bottle配置、GPU后端、显示器scale/坐标和权限。1x/2x、多屏、全屏、减少透明度不足的组合记blocked；不借浏览器截图补齐。

## 5. 四款游戏逐款完成，不跳过未证实结果

每款执行CR03原文、CR05真实翻译、CR13-FINAL完整阅读学习和CR07-SOAK；统一使用同一个最终RC及实际安装bundle，不能拼旧客户端/不同Core的历史结果。

| 游戏 | 正文与位数 | 最低采样与特别检查 |
|---|---|---|
| SR.exe | 现有中文版，x86 | 至少5个真实不同字幕状态；验中文原文、配置方向的真实翻译及非日语降级。手动日语可另测，不算SR日文取文 |
| Memories Off | 日文，x86 | 至少5个不同字幕状态；正文流与当前画面一致；记录接入前渲染/内存基线，取文正常不等于既往图形问题修复 |
| Never7 | 日文，x64 | 至少5个不同字幕状态且至少3个实际对白页；区分同页增量、事件数和页数。登录/异机冲突单独记blocked |
| サクラノ詩10th | 日文，x64 | 至少5个不同字幕状态；相似/同名流按完整身份与正文质量核对，不能固定索引自动选中 |

每款的完整旅程：从库/选窗口核对目标 → 一次确认自动接入 → 显式正文选流 → 停在当前句核对 → 逐句推进且原文先显示 → 真实译文归属正确 → 历史可查 → 日文点词离线中文释义、按需固定句AI → 暂停/关闭翻译/隐藏/断开语义分别核对 → 断开后游戏继续操作。SR在语言步骤验非日语分支。

每款连续样本在开始前选择“至少30分钟”或“至少100次不同字幕变化”的目标，保留停住和恢复推进；同时仍满足五状态配对。十分钟高频fixture与真实游戏连续样本各有用途，不互相替代。正常链路要求无丢失；故障负例出现正确loss/unknown提示只能证明诊断正确，不能抵补正常无损。

## 6. 视觉测试的基准与判定

整页测试按[整页交付计划](current-client-alignment-plan.md)绑定完整用户旅程、页面区域与按钮清单、真实端口/前置和适用层。局部卡片或去Tab断言通过不关闭父页面。有效render须由原DOM/资产独立浏览器复测，不信相同偏好；target、browser_viewport与whole_client分别记录。实际逻辑client宽度/字体/词法状态不符或缺证据时保持不可比/未完成，不能用模拟桌面尺寸冒充真实客户端，也不调整游戏尺寸取得一致。

继承CSV的 `CR-10-VISUAL` 保留旧aligned-v1及固定500×132原文，只供定义追溯；当前视觉由 `current_design_alignment.py` 的完整页面状态/61源组合合同约束。acceptance-v4和 `design_source_versions.py` 解析的是历史已登记参照，不批准新guided布局。舒适默认560宽、原文18/译文24，紧凑500/16/22，控制行32，高度随长文适配；当前4个独立参照仅在各自状态有效，其余状态仍需独审；有效渲染、整页和Native资格分别核验。

TP28逐状态登记“入口→设计锚点→实际组件→基准/候选/diff→人工结论”。至少包括空库/添加/拖入/取消、关联接入/失败重试/选流、原文先到/译文等待与部分失败、ruby/POS/词卡、未配模型往返、设置轻返回且不丢草稿、保存/测试/应用/启用分离、AI等待/取消/失败/文字降级/新句到达、历史人工修正失败保留、诊断恢复。

按当前全页状态逐项绑定阅读卡、三details、必要配置、浅层选择器、HUD和共享Select等生产组件。`product_change_gaps`读取真实改动路径；无法确定影响时保守要求完整集合。当前目标adapter必须由代码登记，并具备分离的实际执行、inspect和独立recompute；JSON不能自授能力。正式React组件与正式包原生层分别取证，source/component不得冒充native，旧v4或HTML原型截图不得重标为当前目标。

用既有Playwright工具运行实际产品组件；冻结数据、字体、视口、scale与时钟。动态mask必须提前列出理由，不可遮住正文、状态或核心布局；阈值先批准，不自动更新基准图。像素差异与人工布局/可读性判断并用，不能拿“有截图”或设计原型自比当通过。原生毛玻璃、桌面透色、失焦材质、置顶/穿透/焦点、菜单/快捷键与多屏另在RC中验。

## 7. AI、词典与性能不能只测“请求成功”

**结构与行为**：请求计划和实际协议字段一致；参数继承/设置/省略有三态；上下文、术语、模板、密钥版本和用途可追溯；固定旧句不漂移；关闭云用途无外发；模型/词典内容只能作为文本，不获得脚本、文件或工具执行能力。等待/取消/拒绝/截断与结构化降级分别断言。默认用途并发1、超时60秒和重试关闭按既定设计检查，不据此承诺云端耗时。

**内容质量**：实际模型ID、版本/日期、参数和prompt全部记录。具备日语判断能力的人按语义、否定时态、人物实体、术语、中文可读性、文法证据与歧义表达独立审阅，争议复核。词典另评中文基础覆盖、词元回退与未知词诚实提示；MeCab/POS着色不代表已有中文释义。不用模型自报置信度或另一模型一次评分代替人工质量结论。

**指标先冻结**：已有PRD的本地关键反馈≤200ms保留，但B0先明确操作、负载、样本量、计时起止和统计判据；它不约束云端翻译完成时间。冷启、事件到原文、词法/查词、峰值/回落内存、队列上限、CPU及恢复等待的数值预算，以及语料样本量/分级/可接受比例，当前均待B0批准。TP19/20/29在政策未冻结时不得执行后自行放宽并标pass。

每项性能记录逐样本及p50/p95/p99/最大值、总样本、失败/超时/拒绝，不只统计成功请求。模型首字/完成与本地调度分开。十分钟高频压力下暂停/取消/断开/退出仍可操作，队列与内存有界、worker可真正终止且有限重启。语言严重语义错误、上下文串台、秘密泄露、原句错配等发布阻断分类在运行前固定。

## 8. 结果与证据如何落地

定义与结果分开保存。`case_id@variant_id` 是稳定执行身份；153行CSV是执行工作表，**不是已运行回执**。后续实际runner/harness随对应研发切片实现并注册到统一门禁，不能把计划校验器当runtime adapter。

| 结果 | 含义 | 对必需组合的影响 |
|---|---|---|
| not_run | 未实现或未执行 | 阻断发布 |
| blocked | 外部设备/登录/凭据或预冻结政策不足，记录解除路径 | 阻断发布，不算兼容失败也不算通过 |
| failed | 运行后不满足预定义期望；包缺辞书/组件属于产品缺陷 | 修复并按影响重跑 |
| passed | 当前身份、当前环境按步骤完成全部断言且证据完整 | 仅该组合通过，不外推 |

低层记录：源码revision或完整dirty快照、锁文件hash、构建身份、Core ABI/双架构产物与语言资源身份、实际命令/时间/退出码及断言数。正式包记录还须包含最终分发hash、实际安装bundle/resource清单、环境、provider/model/prompt/术语版本、画面与对象关联、逐项断言/指标/人工评审/缺陷ID及证据文件hash。人工操作记录步骤和评审人；截图、文件hash或合法JSON本身不能证明实际运行。

任何重建、重签名、公证处理或重新打包后重新冻结RC。最终必需正式包组合全部在该RC执行；未变源码层结论只能经影响分析关联复用，不能伪装为新包运行。秘密/完整原文/私人路径不默认写日志或公开提交；保留本地受控证据，输出脱敏索引。

旧DTO、schema、迁移和公共入口的原断言继续维护。批准的新版本采用独立合同核对，原完整字段/关系/边界在对应冻结旧版本另验；不能删除旧断言、由当前候选学习期望或扩大忽略列表。测试适配和产品代码修复须分别独审，首次红件保留。

授权和资源操作从第一个预检前登记原操作、预算和应用监督；覆盖预检等待时取消、调用方消失、授权在I/O期间撤销、返回前变更，以及退出时等待真实blocking任务结束。future返回、取消标志或观察到permit占用都不能单独证明物理任务已排空；这些回归须落入受管测试，不能只留在`.build`临时探针。

取消/迟到/异常测试不能依赖固定sleep碰运气，优先用受控时钟、barrier和明确事件等待；真实超时与进程终止另验。失败重试保留第一次失败，禁止“重跑到绿”掩盖不稳定；需要查原因、修复并重复覆盖相关状态。

## 9. 上线放行规则

只有以下条件同时满足，才提交首版发布评审：

1. FP12、AI-CFG16及97继承来源和31机制定义的原153冻结组合通过；当前PA12/OT16/SS12/FL10/GD12和GUI8的70来源/576观察组合另行全部通过。两份执行清单分别强制，无必需not_run/blocked/failed；不删旧项或以设计/旧包结果代替新目标。
2. 完整设计最终批准、B0资源/中文辞书/许可与指标/质量政策冻结、架构债务清零，OpenSpec及代码/编译守卫通过。
3. 同一个最终RC的L6/L7/L8证据齐全，实际安装身份可核对；四游戏各自结论明确，真实服务与离线资源均完成。
4. 无未关闭发布阻断缺陷；身份串台、正常原文丢失、保存/迁移破坏数据、凭据泄露、无法启动、核心入口不可达均阻断。一般缺陷须记录影响并在不违背已承诺验收的前提下正式评审，不自动豁免失败用例。
5. 按冻结渠道检查签名/公证与普通用户冷启、受支持数据迁移、备份恢复及兼容回退；不要求新增自动更新功能。不得通过开发机移除隔离属性冒充干净安装成功。
6. 真实产品capture/runtime adapter已接入，统一入口有实际执行回执及人工内容/视觉审阅；远端CI及分支保护必须另有实际配置证据。不得仅凭CI配置文件宣称远端已运行或已设必需检查。

静态校验只证明登记/来源/覆盖/DAG/导出与守卫行为。当前已有Rust、产品组件/截图、存储和SR6正式包稳定性执行器；其中SR6不是完整153回归，也没有登记为当前576组合的执行能力。当前附加业务闭包缺完整真实执行能力，已批准guided与R79完整源版不再待源版评审；现有4态React检查不能覆盖809项状态及正式包Native，发布继续阻断。架构债务与各执行器结果以当次机器回执为准，不沿用原规划时的数量。完成本轮同步不提升设计、产品或发布状态。

## 10. 后续测试设施任务与执行节奏

[测试设施任务CSV](first-product-test-enablement.csv)拆出TE-01～12，全部planned，并映射B0～B5；它是现有55任务的测试补充，不把旧任务标完成。

按如下顺序推进：先完成B0测试/资源/质量政策与设计批准 → B1新Core双架构/正式原文包 → B2阅读数据与原生多窗 → B3配置与真实服务 → B4离线词法和按需学习 → 冻结最终RC → B5四游戏、稳定性和分发回归。每段都跑适用的L1～L6；共享端口变化扩大影响回归。自动化用于可重复规则和故障时序，真实服务、原生行为、语言内容与四游戏保留各自证据。

计划及合同检查入口：`npm run verify:test-plan` 校验登记；`python3 scripts/quality/render_test_plan.py --check` 校验导出；`npm run verify` 连同OpenSpec、来源/基线、工程守卫及实际编译检查。正式产品执行命令随TE设施实现后注册，当前不提供会伪造产品pass的占位命令。

## 11. 测试前置与长期维护

供给与维护总表见[测试前置、代码维护与证据归档](first-product-test-operations.md)及[机器登记](first-product-test-operations.json)。18项准备分别说明谁提供、最晚门禁、当前状态和解除办法；每个执行组合关联对应准备项，每个用例关联维护位置。双屏仅影响DISPLAYS，本机/云服务依赖按组合判断，纯规则/合成协议可独立推进。

测试代码靠近所属crate/组件或放在既有根tests/integration下；工程守卫、产品测试、批准截图基准、授权fixtures、私有原始证据和可分享报告分开维护。模块Owner维护逻辑，QA维护计划/覆盖/证据与发布汇总；未经批准不建立空产品测试目录，不把规划设施当现有实现。报告QA源码纳入scripts/report长期维护，运行证据须另做受控归档，不能只依赖.build缓存。

## 存储模块专项子验收与字段边界

用户要求存储验收和字段边界一并纳入程序约束，详见[存储验收登记](storage-acceptance.json)与[执行规范](storage-acceptance.md)。这些子检查逐一绑定原有TP/MC/CR的case@variant，不增加或改写128个定义/153组合，也不预填产品通过。

TE-04/AP、PL主责真实存储设施，TE-06负责凭据与取消竞争，TE-08负责随包只读辞书，TE-10负责实际运行adapter/完整回执，TE-12负责正式包恢复；PRE-13与PRE-14分别供给实际设施和已冻结schema版本/备份恢复样本。测试维护沿MA-02/05/10/16，产品实现源码/测试与工程守卫分开。

设计阶段即可运行独立schema锁、逐字段适用边界及守卫负例；正式存储验收必须实际编译/运行Rust目标，由程序读取生产迁移后的SQLite并比对全部字段、关系与约束，同时核对迁移路线版本、事务/故障/备份等业务断言。仅运行设计DDL或生成一致字段字典不满足本模块验收。生产存储adapter的能力及结果按当前代码登记和当次回执核对，任何既有运行记录都不自动覆盖新增会话全量、Hook日志或保留政策。PRE-14支持路线及新增字段政策仍需逐项冻结，实际存储命令按当次源码和证据返回结果，不能靠历史设计检查提升上线状态。

## 阅读与服务逐层执行

[RW覆盖合同](reading-window-test-coverage.md)按34项权威来源闭包检查设计、真实组件、真实数据管线、真实语言、原生与游戏六层。`interaction-design-execution`实际启动的是设计层；`interaction-coverage-contract/tests`验证登记与负例，均不能冒充产品运行。缺层、漏状态、旧源码/包、零断言、无nonce、截图未关联和跨层冒充均应失败。所需Chrome、Playwright、正式包、Core/词典、桌面权限、多屏/快捷键冲突与可推进对白的游戏由[供给矩阵](first-product-test-operations.md)逐项维护。

当前70来源/576观察的准出由 `current_product_coverage.py`独立读取 `current_product_coverage` path/hash侧录；新ID不进入原153执行集合。25服务精确指21 HTTP预设+4离线引擎，HTTP协议13种。GUI01～08的设计回归来源保留，真实产品层分别取证；GD权威验收ID与设计runner旅程编号含义不同，不按数字相同推断覆盖。

OT-16按4引擎×4游戏展开16个正式包观察，每项冻结engine/contract revision/backend/runtime、合法资源版本/hash/许可、逐状态实际结果和独立质量审阅。四款各至少5个不同游戏/客户端配对，Never7至少3页。SR既有中文版仅记同语种直通、非日语降级或真实非日语翻译；未调用引擎不能记翻译质量，其他三款须真实日语翻译及质量结论。资源未授权、unknown或route不支持继续blocked，不因别的引擎通过而解除。4项Windows原机资格保留独立环境、同位数ABI、合法DLL与Windows工件，不复用Mac包hash。

检查与评估命令见[当前覆盖](current-product-test-coverage.md)和[发布证据](release-execution-evidence.md)。每次实际运行另建run记录；未执行写 `not_run`，缺外部条件写 `blocked`，不重写原计划状态。

旧正文原字节保存在[文档归档](../archive/documentation-r75/docs/development/first-product-test-plan.md)，只供追溯，不作为现行执行入口。
