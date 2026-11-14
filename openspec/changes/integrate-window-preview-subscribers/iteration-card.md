# B正式接入迭代卡

日期2026-10-24，用户授权当前窗口缺陷修正，并指出只有技术进程、当前界面并未改变不能满足目标。Root在既有首版授权内按本卡实施，A失败不提升。基线精确R40/1688见source-snapshot；先于任何B代码修改记录。

需求FP-03/04/12、RW-01～04；设计当前整稿ansi-preview-recovery.html#native-pairing和既有预览/控制状态，不改视觉图/token；模块M02/M12/M13、组件C13/C14/C35。

Native Owner仅9Rust文件：main.rs、bootstrap.rs、platform/window_preview.rs、platform/window_collector/{mod,protocol,worker}.rs、ipc/{reading,commands}.rs、app-infra/hook_backend.rs。UI Owner仅5TS生产源和3测试源：application-port、tauri-port、ReadingPanel、CaptureSheet、WindowPreview；design-port、ReviewedReadingProbe、verify-reviewed-reading.mjs。QA Owner仅scripts/quality/{reading_components,test_reading_components}.py。所有实际路径由verification列精确白名单；无新增产品源/依赖/Native桥/Windowsbroker/DB或公开Core DTO。其他必要路径先报Root，不越界写入。

Root独占本OpenSpec/两架构文档/精确基线与catalog/安装GUI；A阶段顺序覆盖只记在本B卡与design，A三原稿维持R40字节；并行Owner不回退他人。Rust格式化只逐文件edition2024+skip_children=true，禁止递归rustfmt。

确认本地TS attach返回精确unsubscribe、每lease监听上限4、单上游pull；所有调用点机械迁移，不弱化七红断言或用fixture分支硬编码。Rust所有者数量max2/global1、完整proof、旧帧拒绝、预算/drain与安全退出见design。

A顺序修订：因独立PoC无统一UI/重试，用户已选旧入口仍未得到修复效果，Root决定先实现B正式控制/订阅路径，A真实变化帧/消费隔离作为B交付硬门槛。它仍not_run/blocked，原失败不追认为通过；不得据单测/代码或预检宣称修复。当前Main75789、game88536/Win2148保留，A子77146会按原预算收尾，不自动重启。

验证：Native真实协议/队列/上下文/身份/取消/退出正负例；真实React与生产Tauri adapter永久红转绿，原81+新增82–97；编译/5负例/统一verify；同源签名包默认入口实际系统窗口与多帧、反复切页/无慢消费者、Hook/词法与适用游戏。缺环境blocked/未执行not_run，原视觉app.js冻结指纹漂移、全局8MiB、完整发布不冒称通过。


### 开工后的精确增量记录：系统选择回调与RBC72/73

独立审阅及Native Owner一致查明：SDK selector、ARC强持有及worker主RunLoop存在，但桥对非nil stream/current operation失配/已结束回调静默返回，既有90秒超时无法区分OS无回调与被拒回调。Root在编码该增量前授权Native Owner额外且仅`apps/desktop/src-tauri/src/platform/macos/bridge.m`：增加有界安全回调事实诊断（PID/currentMatch/streamNonNil/mainThread/timeoutSource）；当前操作的未知stream继续拒绝并投影明确typed unavailable，不接收别的stream、不放松身份核验、不让迟到旧操作终结新操作。此后Native精确10文件，生产精确15文件。无新增桥头文件/DTO/依赖；需要其他文件先报Root。两次真实零帧超时并不能据此认定已找到排他根因。

核对实际runner后，原RBC72 effect自动start重试与纯订阅新权威冲突；RBC73仍验证target-change/cancel晚到结果隔离，不迁移其目的。Root明确允许保留其原身份和防无界资源重启/可恢复目的，将断言机械迁移为同producer下有界前端读恢复、effect零start/stop、CaptureSheet显式刷新发恢复；不保留隐式重建producer的错误行为，不删除旧失败证据或缩小故障覆盖。RBC42/46与其他原81目的仍保留。


### 首轮真实组件执行后的旧合同迁移核对

首轮97项实际产出93通过/4失败、1543断言/errors0，同时因并行源码变化而adapter整体拒绝，通过状态不继承。原回执保留。RBC11暂停保留原URL属于产品缺陷，修代码后原断言须通过。RBC72 resume必须清transport失败状态，fixture只忠实模拟该控制合同而不能代替生产恢复断言。

Root明确核对RBC43/78的内部API迁移：start/select已不再暗装监听，view使用明确attach与独有unsubscribe。RBC43的暂停控制返回新generation，view必须以新代次明确reattach，因此原固定attach数量3不再代表同一合同；可机械更新为对应真实操作的4次并补唯一代次/旧闭包不能取消新代次/producer不重启的独立断言，不能仅改数字掩盖多余轮询或泄漏。RBC78迟到select结果本来没有安装监听，应验证其从未attach且不污染新session；卸载仍须精确释放此前真实存在的那份监听，不能要求取消不存在的listener，也不能省掉已有资源的收尾。上述原ID、取消/晚到隔离及producer所有权目的保留，并由独立QA再审。


### 异步picker的真实释放确认（编码前精确授权）

独审确认异步等待线程只调用系统picker，不携带Windows verifier/PE FD，结果回单一scheduler完整核验；但Rust future取消仅排入Native mainqueue，丢Receiver不能证明原生reservation与callback context已释放。Root明确授权在既有`bridge.m`增私有只读`luna_preview_picker_transaction_active(token)`，仅由已授权`worker.rs`内部FFI使用，不新增桥头、macos/mod.rs或公开IPC/DTO。精确transaction在首次异步提交前纳入受锁生命周期，查询不得把尚未登记/未知状态误当实际完成；先fence，保留retiring context，真实取消/完成与filter释放后才Released/复用槽。继续3秒返回cleanup_pending并后台drain，不以drop Rx/100ms sleep冒充回收。新增实际晚到取消、Mainqueue尚未处理、完成后复用测试及独立审阅；不得放宽身份、许可或global1约束。


### 冻结前端后独立复现的跨会话饥饿（源码修复）

独立QA真实React counterexample：A的gameWindowBinding请求保持未完成，切换B并自然等待5600ms，仍queries1/settled0、B无preview查询/订阅；释放A后B才开始。证据`session-starvation-1792784472871/receipt.json`，不是实机帧结果。此项阻断UI冻结版v1通过。Root授权原UI Owner在既有8路径内解冻必要ReadingPanel及原测试源：新context不等待旧context未完成查询，每context仍single-flight，旧finally/结果不能清新pending或新绑定，真实backend绑定读取仍纯cache。保留v1冻结/97原回执，另建v2冻结及缺陷修复实跑；RBC87/97强化为A未完成时B已实际绑定、订阅、显示有效帧，随后旧A拒绝/结果/帧全部无效。禁止通过先释放A或只看查询计数掩盖饥饿。


### 独立复现的元数据替旧图改时间（源码修复）

QA真实React复现：只将projection元数据capturedAt从1792368001000改到1792368009000、没有新frame且blob不变，下一次3秒读取仍把页脚画面时间从08:00:01改成08:00:09。证据`metadata-image-time-1792784979166/receipt.json`。Root明确授权原UI Owner仅在原8路径内窄修C35页脚：有实际已显示图像时以displayedFrame真实capturedAt为准，无有效显示帧不得由projection伪造图像成功时间；新真实frame才能推进图像时间，原终态清图语义保留。RBC92/97补metadata-only未来时间/同blob旧时刻、随后真实新帧才更新的直接断言，不新增ID/DTO/样式；v2回执保持，另建v3冻结及实跑。固定4秒SLO依旧未验收，不借本修复宣称已完成。

### 历史字节差异证据的限制

无Git且preB仅保存hash，UI Owner和QA实核只找回与原SHA精确匹配的3/8源，未找到旧runner完整原文；v1源不能冒充preB，不重建或伪造旧源。本轮不声称完整preB逐字diff或所有旧断言字节未变；原81身份、原七红实际断言、逐项迁移目的与当前真实行为另由独立QA审阅，受此限制的结论明确保留。v1/v2及后续候选已额外保留完整源码字节与回执，便于后续精确核对。


### v2永久场景暴露的测试探针接线错误

v2最终实际96/97、1601断言，97等待B超时，另有并行全树漂移拒绝；源/失败回执保留。已定位`PreviewReadingAdapterProbe`仅读fixture.state初始prop、未订阅publish，导致永久场景切B未实际触发React更新；独立正确接线的真实组件14断言仍通过。Root授权在原ReviewedReadingProbe范围内复用现有ApplicationStore/useApplication订阅controlled fixture快照，被测绑定/预览仍走真实TauriPort，不复制ReadingPanel逻辑或改断言等超时迁就错误。QA独立复核真实publish→组件新session→Tauri绑定/帧完成，v3仍要求A不结束时B成功与旧回调隔离。


### 缓存末帧不等于健康恢复

独立双真实组件+TauriPort复现：A transport失败缓存stale末帧，后续read永远pending，新B订阅拿缓存；跨3秒metadata后B误升Live，A仍stale，blob与在途read不变。证据`stale-cache-remount-1792785479849/receipt.json`。同原B“无假实时”合同，修C35仅有效Live新帧可清降级；缓存/nonLive末帧不作为恢复证明。92/96增加真实双组件、pending、自然metadata周期及随后真实新帧恢复断言，旧v3保留并产新候选，不改ID/DTO/样式/阈值。

冻结说明：当前v1/v2/v3均是评审中的候选点，不是Root最终采用。原路径、原合同内发现的实现或测试接线错误由Owner立即修复、通知Root与QA、保留旧源/失败并另建新候选，无需逐个普通bug等待许可；新增路径/公共合同/架构职责仍先报告。Root启动最终采用前才由各Owner共同确认停止修改并锁定全部hash。

### 最终审阅拦截的入口、许可和队列问题

Native候选01将已提交的Windows关联与Native图像就绪混为一体，初次选择器尚未完成时返回Unavailable，导致ReadingPanel不挂载选择入口。修复必须分别表达：保留M02可信关联及其真实原核验时间；Native selection_required/Selecting/权限失败不抹掉该关联，Windows明确目标关闭/身份失效才使其Stale。另有真实Native返回`preview_selection_busy`未被识别为未接收事务，未知token不能被当已释放，也不能把明确拒收当永远pending；补真实拒收、排空与正常退出断言。

UI候选04在capability-only denied清图后，迟到同generation高序号Live仍能复画。候选05在原C35与92/94/97范围内修复：拒绝许可立即终态清图、无成功时间，同context迟到帧不得恢复；实际许可恢复后仍需合规的新有效帧才能恢复。首次M02 Bound但无Native帧的实际ReadingPanel+TauriAdapter组合必须可达显式选窗按钮，真实closed/stale仍无入口。

独审还拦截原A负例迁移成Shutdown底稿导致空绿：必须以合法非终止控制为底稿，仅突变replay/instance/未知字段，并有合法控制继续运行的正控制。B的Live状态不能挤占关键Ack队列；保留有界latest合并与关键状态独立背压，真实压力例验证大量Live、Stop及无读端，不通过扩容掩盖问题。以上均在原10Native/8UI路径内，旧候选完整源码、真实失败与已执行有限通过记录保留；最终精确冻结与独审之前不采用新权威基线，不提升签名实机或产品验收。


### R41登记失败后的原范围修正（R42，元数据限定）

candidate19在精确R41源码上实际构建117.07秒exit0，但统一verify13.548秒exit1：`allowed_paths[29..31]: may not grant another change\'s directory`。原字节独审没有运行此change-contract分支，新增发现与原失败回执均保留。19未安装、未做包审计或实机，不据编译成功通过。

修正不改变产品实现：A的design/card/spec恢复精确R40 SHA，B删三项跨目录授权；顺序修订只由本B卡、proposal/design/spec说明，A旧稿作为历史来源不再改写。spec原末行从已启用Orca索引定位的真实本会话18:35:45.680Z工具输出恢复，非猜测原文；恢复候选与来源在`.build/window-preview-sakura-20261023/scope-recovery-candidates/`。原B source-snapshot整份保持R40，既有guard、批准图/token、catalog产品459源与497定位、Native04/UIv6全字节不变。

在R41原字节归档、合法change-contract与原始snapshot范围检查实际通过、R42精确新delta独审后才重新采用、构建candidate20和完整verify。R41/19失败不改绿，视觉source drift、性能与发布缺口不提升。


### candidate20实机零首帧后的缺陷修复（编码前记录，2026-10-24上午）

用户报告候选与已接入会话均无法显示窗口。Root保留Main3750/worker4683/游戏88536现场，已确认Hook正常收到正文而C35尚无首帧；旧超时不视作用户未共享的证明。只读链路核对发现生产TauriPort将起始targetIdentityHash=null与首帧解析后的真实hash作全等比较，正常首次解析被丢弃。UI Owner在原tauri-port.ts、WindowPreview.tsx及原三测试路径内修复并先保留真实生产adapter红例：同lease/generation/consumer仅允许未解析到经frame/projection一致验证的首次身份，已知hash变化、错上下文、旧序号继续拒绝；同一producer不因此重启。C35也不得独立再次丢弃合法首次解析。补首帧、第二帧、多订阅、异上下文/已知目标变化拒绝及无图超时状态区分的检查，旧97项与旧失败保留。

Native Owner继续只读核对真实系统picker/图像捕获/Parent投影的错误来源，不凭SCK调用日志认定图像已生成；现有安全诊断若不能定位，应先报告最窄增量再编码。仍遵守现有10Native/8UI范围、原快照不重写、不改批准图/token/阈值，不用构建成功代替实机两张变化帧。新候选使用21或以后编号，20原包和证据不覆盖。


### candidate21可靠故障证据增量（编码前精确授权）

原Native统一日志在真实worker查询中未取得本轮picker前缀，stdio又明确关闭，不能用缺日志判定用户没共享。Native完整错误/阶段在worker只剩error_code，确有观测缺口，尚无证据认定SCK图像尺寸、回调路由或frame socket为现场根因。Root授权Native Owner在原10路径及额外且仅`apps/desktop/src-tauri/src/platform/macos/mod.rs`薄适配内实施有界typed诊断：系统picker回调received/processed/timeout与Native捕获固定阶段、错误domain白名单/numeric code、配置和实际像素尺寸，经同worker私有Status绑定source/epoch/capture/request返回Parent，再在既有PreviewProjection.detail中安全格式化。只保留当前操作快照，不写正文/帧/路径/标题/token/环境、不可全局日志dump或新增HTTP/DB/Core公开DTO。picker诊断需绑定对应transaction，不能把旧操作事实投影到新source；capture仍global1、生产者独立、原filter与完整身份门槛不放松。

Native Owner不改批准图、资源、预算、compat eligibility或架构职责来迁就失败；真实超尺寸以实际尺寸和固定reason报告，不能直接扩大预算或泛化fallback。新增串源/迟到、上限/白名单、可见timeout及端到端Status透传的自动检查；诊断本身不等于画面成功。UI Owner可在C35原路径展示safe detail/错误码，禁止暴露Native原始NSError正文。Root独占本卡、verification和架构登记；原snapshot不重写。


### candidate21完整门禁拦截的错误提示结构回归（编码前修复记录）

Root的诊断可见性窄补将C35 selection_failed友好说明与安全detail拼成一个p；完整100实际99通过，原RBC94要求原独立提示可定位，第三个selection_failed分支断言0!=1。权限/身份原断言不改；不可删测试或改为模糊匹配规避。Root仅在原WindowPreview.tsx恢复原独立友好说明p，并以另一独立p呈现既有安全detail，保持诊断可见与原文案语义，无新CSS/token/DTO。原runner全部字节保持R43，先实跑RBC94与98–100四项再执行最终完整100。

21包构建与静态85文件审计成功，但fullverify56/58（原视觉source漂移+本RBC94失败），未安装、未退出Main3750/worker4683或游戏。21原包/100项失败/审计保持，不改绿。修后另建R44精确映射与candidate22，旧R43作精确归档；只更新本B/原架构状态证据，R40snapshot/A原稿与批准参考不变。


### candidate22已共享成功后的休眠恢复缺陷（编码前精确授权，R45）

用户确认已完成共享，并明确故障发生在放置休眠、唤醒后。Root实机先见14:49:05真实游戏预览，后保存shared-regression/frame-a及frame-b：同Main72289、worker74609、游戏88536/Win2148、Session1792824430994-72289-0；Hook正文225～232继续到238，但预览末帧15:01:19.778、Native操作114不再前进。诊断picker_update_processed收到1/处理1，capture_complete/window_not_on_screen/Native4/APIshareable_content。不能再归咎未共享；Native4与fallback最终失败要分开，目前最终errorCode不可从现成GUI读取，不猜定具体terminal原因。22的100/1802组件、148Native及静态包审通过，fullverify57/58原视觉来源失败保留，不追认实机完整通过。

Native Owner仅在既有bridge.m、macos/mod.rs、window_collector/worker.rs及确需的window_preview.rs内修正：持原始已核验单窗口filter时，onScreen=false不在SCScreenshotManager调用前硬拒绝；自动映射且没有filter的路径保留该判断。权限、PID/start/CGID、filter范围、pre/post proof、source/epoch、预算/global1、兼容fallback仅4/9全部保留。不为休眠重启Main/worker/Hook、不切换目标或自动弹picker、不改变游戏窗口/容器/系统休眠。

补固定白名单的最终capture/fallback失败码与阶段，保留Native4原事实；禁止原stderr、标题、token、路径和用户内容。仅对可再次完整核验的图像编码暂态、同窗口尺寸变化暂态允许现有有界重试保留choice；权限撤销、身份/selection真实失效和预算失败仍拒绝，不把坏帧标实时，末帧不刷新时间。若需额外文件或改变预算/终态规则先回报Root，不自行扩大路径。先实际红例证明编码暂态会永久清choice及offscreen早退，再修复并跑重试后新有效帧/相同资源、旧帧/撤销/错身份/预算负例。

Root独占本OpenSpec、两架构、catalog/精确R45及唯一新candidate23安装GUI；UI8和QA2本轮保持R44全部字节。原R40 snapshot/A3/批准图/token/视觉零阈值保持。Native结果独审、精确范围采用后构建同源23并完整verify，再实机验证新帧/连续变化/跨页订阅；睡眠恢复只能按实际执行记录，未实测不填通过。
