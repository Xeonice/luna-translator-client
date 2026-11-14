# R63 design v3.3 提案：按固定阅读对象查询独立状态（编码前，待独审）

Root已将packet02标changes_required。两个P1：坏历史lane的graph错误传染全AppSnapshot；真正failed/cancelled head在旧tasks100窗外时状态漏供。旧packet02与全部失败/通过回执保留，不在v2继续修补哈希或声明通过。

## 一、先评估ReadingUnit交汇点

ReadingUnit同时用于snapshot current/history、historyPage后续页/readingUnit detail、session body、缓存输入、frozen LearningObject及serializedUnit；它是固定原文事实与已有译文的值对象。给它直接加瞬态状态会改变序列化/指纹及冻结学习对象含义，或把旧serializedUnit缺字段隐式变为当前状态，风险大于此修复。

推荐组合包装：新增ReadingUnitProjection={unit:原样ReadingUnit,translationState:独立ReadingTranslationState,profileId,serverInstanceId,profileGeneration,commitRevision}。原ReadingUnit结构/序列化/历史record/freeze与readingUnit旧API完全不改；新按需readingUnitProjection(unitId,textRevision)同一次Rust读取组合固定unit与状态。C17消费当前传入unit的这个包装，绝不从snapshot.tasks推当前归属。UI不得把translationState写回unit或frozen object。这个设计在原事实读取交汇点完成状态组合，同时保留旧数据兼容。

不新增全库head列表、不在AppSnapshot增加无限状态集合；恢复TaskView原业务字段与原bounded tasks集合，仅新增attemptId:null|string作为真实attempt身份证明（serde default兼容旧JSON），旧operation receipt/status/attempt语义原样保留。按需对象scope覆盖snapshot以外的真实historyPage(cursor,50)→selected readingUnit(id,rev)→C17，以及HUD/手动/当前会话；与默认snapshot recent50无关。

## 二、独立状态DTO与有界多态

ReadingTranslationState含unitId/textRevision/sourceHash及engines数组。engines只来自该unit.game_id当前applied的enabled effective binding与enabled engine，稳定scope/binding/engine ID显式保留，providerLabel只展示；无binding/无启用引擎时数组为空，不自动换模型或发请求。数量受实际配置引擎集合约束，绝不枚举全库lane。

每引擎TranslationEngineProjection含scopeKey/bindingId/engineId/providerLabel及typed status变体：
- unrequested：该lane无task；不表示失败/成功。
- current {task:TaskView}：完整DB合法graph恰一head，原样提供该真实task全部状态，即使它不在snapshot.tasks100内；queued/running/failed/cancelled/invalidated/completed均不丢。
- unordered {summary:TranslationHeadSummary,activeTask:Option<TaskView>}：合法graph多head，legacy/混写/分支均不能选winner。summary固定字段为headCount及各真实status计数（规范十进制字符串），不携带无限heads数组，不把completed当最新成功；若恰有一个active head，可附其真实TaskView供进度/取消（标为处理中请求，不称最新winner）；多个active heads不任选。UI明确“归属未定，含N次失败请求”，保留其他引擎错误。新显式admission仍合并全部heads，后续变current。
- unavailable {error:Diagnostic}：只捕获invalid_translation_status_lineage本lane校验错误，消息/错误码可见、不可冒充legacy/无请求/成功；其他引擎和阅读对象正常。SQL执行/数据库致命错误仍上抛，不掩盖真实代码错误。

因此响应每配置引擎至多一个固定状态，不随历史task数增长；完整DB只用于求graph/计数。类型在app-core中表达，现有exporter生成TS。TaskView保留原8业务字段，仅增加attemptId:null|string（serde default兼容旧JSON），读取该task的最新attempt_number对应真实attempt ID；不加current/superseded flags。旧receipt原status/ID/错误不改，增加的attemptId为真实事实，不是状态推断。

## 三、保留v2因果写入与局部故障隔离

translationStatusLineage schemaVersion=1/supersedesTaskIds typed/versioned metadata保持v2全部严格规则：缺key legacy，无序heads保守；present null/未知version/错误类型/重复键或ID/self/缺引用/跨lane/环拒绝；直接反序列化原JSON，不能先Value丢重复键。admission同Immediate事务取完整本lane heads、写新task指向全部既有heads，旧事实不动、operation重放早退不补边、自动retry不建新task。旧binary随后无metadata追加是新独立head，下次显式请求合并。无rowid/clock/ordinal因果、无DDL。

从旧task_views删除DAG/成员推导，恢复原查询，因此任何旧坏lane不会使AppSnapshot失败。按需projection对每个当前配置engine单独捕获lineage错误并产生unavailable；同unit另一engine正常，另一unit完全不受影响。坏lane新admission仍严格Err/事务回滚，其他lane可新admit；不自动修复/删除坏数据。历史移除engine/binding不属于当前engines但仍完整保留任务事实。

## 四、实际失效信号修正与attempt来源证明

独审确认原fail_task更新task/attempt并commit但不bump profile commit_revision。v3必须修实际writer：把失败事务收敛到queries/tasks helper，task/attempt实际变更时复用既有bump同事务更新commit（重复或终态noop不额外bump）；失败事实与版本同成同败。新增测试证明失败、取消、失效、完成、重试、配置变化均有能使按需query刷新的实际commit信号；尤其running→failed不能永远卡running。不是靠global tasks status推current。

TaskView仅补attemptId可空真实ID，由同DB读取按attempt_number取得；原八字段与任务集合不改。Runtime partial buffer改为带attemptId的内部结构，并持有当前active attempt token。每wire attempt创建捕获该attemptId的delta，只有task handle仍在且token匹配才可写；retry事务返回新attempt后立即更新token并清旧buffer，再publish/退避；旧attempt callback不能覆盖或删除新buffer。取消/结束/停用清token和buffer。新增嵌套锁统一tasks→active_attempts→partials，publish仍projection_lock→repository→partials，不反向获取tasks/token；调用publish前释放所有callback锁。

publish将partial合入TaskView时必须buffer.attemptId==该TaskView的DB attemptId且task处于queued/running，否则丢弃；因此DB retry已commit而token尚更新的极小采样窗口也不会发布旧attempt片段。C17只展示新query指定taskId+attemptId完全一致的Rust snapshot buffer，且source snapshot commit>=RPC commit、档/实例/代次匹配；不从global tasks推当前status/error/head。若旧query属于attempt A而snapshot已B，ID不匹配就暂不合片段，等待B的新query。该额外TaskView身份字段是必要的跨retry证明，不把瞬态buffer写入ReadingUnit或SQLite。

## 五、同一SQLite读快照与runtime边界

ProductRepository新增reading_unit_projection(id,rev)只读端口。SqliteStore在单写actor call内显式开启DEFERRED read transaction，首先读取profile_meta pin SQLite快照，然后在同一Transaction中读取原unit、当前applied、全部所需lane graph/计数与commit；helper要求&Transaction，不能只是多次&Connection查询。读事务完成后返回组合值，不修改任何事实或触发publish。

取消projectionRevision字段，也不在新RPC合并partials或获取runtime projection_lock/partials mutex；该接口只描述这次SQLite事务的profile/server/generation/commit快照，避免用watch新stamp标旧memory partial。ApplicationRuntime只是调用只读端口。C17的实时partial文本继续使用既有Rust snapshot.tasks的buffer，但只按新独立状态query选定的确切currentTaskId+attemptId取partialText（且profile/server/generation一致、source snapshot commit>=query commit、返回task仍running/queued）；绝不读取global tasks来推导engine/head/current/status/error。所有active task本来就在旧tasks集合，不受终态100限制；终态错误/取消/成功的状态供给全部来自新独立RPC。这样只复用现有Rust流缓冲展示，不引入第二份业务权威。

Tauri仅增加application_reading_unit_projection命令，main/hud/learning权限与原reading_unit相同；既有命令不改。ApplicationPort/TauriPort新增typed方法，无UI SQL/HTTP。

## 六、UI owner/floor/dirty的精确合同

C17共用useReadingUnitProjection hook，owner key=(profileId,serverInstanceId,profileGeneration,unitId,textRevision,sourceHash)。同owner最多一在途；新owner立即开始，不等旧owner未settle；旧owner resolve/reject/finally不能写新owner或清它pending。返回unit身份/hash与档/实例/代次必须严格匹配。原ReadingUnit/frozen对象不被状态更新mutate。

状态查询触发为owner建立、snapshot.commitRevision变化或显式重试；不是watch每个token的revision变化。partial更新只重绘确切task的buffer，不重发状态RPC。新请求开始时记录requestFloor=当时已知commit；只接纳response.commitRevision>=requestFloor且>=lastAppliedCommit的结果。绝不能按response到达时持续推进的latest global revision严格拒绝，否则连续raw写入会饿死显示。

在途期间新的commit合并为newestDirty，不并行同owner请求。合格响应可以落地为明确的as-of commit状态；若newestDirty比它新，UI标“状态更新中/上次读取”，不能把旧失败继续无标识地宣称当前失败，已有结果/原文不清空，然后安排一次合并的最新请求。后续请求start floor取最新dirty；同owner连续复读开始间隔至少100ms（首请求和新owner立即），不紧循环。返回低于start floor的响应是可见读取错误，不自行零延迟重试；只在新外部commit或用户重试时再读。持续无关raw commits下仍允许last-valid状态显示并标正在更新，不能永久只剩loading；停止写入后最终追上最新commit。

主窗各feature当前都常驻DOM hidden面板，因此将既有active显式传至History C17/SessionOverview；ManualInputScreen新增同样active prop，由MainWindow page传入，ReadingContent传递。inactive只停止该view只读状态查询，不停止Hook/翻译/preview，不清原文/译文/编辑状态；reactivate按当前owner重新读，旧inactive查询不得写回。这样只有实际输出的阅读对象建立按需scope，避免隐藏的三个面板持续查询。

SessionOverview与TranslationTaskStates复用同一hook结果；HUD/ReadingContent/History C17按传入的固定unit查询。HistoryScreen仍通过真实historyPage/readingUnit得到selected对象，不搬到snapshot50；retranslate一次性receipt不是持续状态权威。原文/译文/人工修正仍由原ReadingUnit呈现；学习窗冻结对象不挂live hook。

## 七、完整事务admission与receipt

read projection按engine局部隔离，不代表改变写入原子性：现prepare多engine全组Immediate事务保持，一个参与engine lineage坏则整次多engine admission原子拒绝，不给其他engine部分入队；其他unit的独立命令或用户明确改配置排除坏engine后的命令可以正常admit。测试必须断言坏组事务无新增task/attempt，而已有健康engine状态仍可读。

同operation重放早退，无新边/attempt。为保证旧receipt自身也不依赖tasks100，将PreparedTranslation携带按该operation实际taskIds从完整DB读出的原TaskView receipts，runtime.retranslate返回它们，不再用snapshot.tasks100过滤；status/attempt/ID仍原样，不把receipt当持续state。新增隐藏旧operation重放测试，原有效重放/人工修正/缓存断言保留。此内部类型改动仅在已允许domain/mod.rs、tasks.rs、runtime.rs内。

## 八、验收与原有效断言

1. 先通过真实测试证明fail_task实际bump且noop不bump；自动retry捕获old delta，在新attempt发布后再调用，旧片段必须完全不入snapshot，也不清新buffer。纯DAG严格拒绝分支保持；改变错误的snapshot.is_err负例为更强的snapshot成功＋坏lane unavailable＋其他lane/current读取/新admission可用，同时保留坏lane admission拒绝和历史/attempt原样断言。不能删metadata非法分支。
2. >100场景覆盖hidden failed、cancelled、completed（各有旧可见任务和101无关终态）；独立projection必须有真实head状态，snapshot.tasks原100/receipt不变；不得把最新head timestamp伪造成更大。
3. legacy多head/旧binary后追加/new explicit合并、同名多engine/配置成员/固定unit.game_id/旧callback/自动retry/旧operation重放/备份及逻辑复制；unordered summary失败计数真实，不选赢家。
4. 真实Rust runtime创建超过50条历史，实际history_page首屏＋opaque cursor后续页选旧句，调用reading_unit与reading_unit_projection，再实际重译并产生100窗外状态；前后raw ReadingUnit/serializedUnit/frozen学习对象不含live状态且原断言保持。
5. 在原exports_current_session_component_snapshots真实Rust producer追加独立readingStatusHistory payload（原4scenario/原assertions不变），同nonce/source hashes/fresh file绑定；原RBC38读取该真实旧页、selected unit projection和重译各阶段，使用生产HistoryScreen和C17验证queryHistory→detail→按需RPC→显示。所有100个RBC check ID/oracle保持、旧RBC38断言保留。额外producer payload严格源/hash/nonce验证，不能手工构造成功记录或搬入snapshot50。
6. 新Tauri查询角色/typed adapter、旧owner未settle/迟到、新revision/切档/快速选旧句、持续raw commit/token revision时仍可显示且无紧循环、停止写入后最终收敛的UI实际测试；组件/设计截图不代替安装包。原native IPC check加到verification；完整verify及实际storage adapter另跑，Root所有authority/critical link/final cohort登记单独处理。

## 九、状态与采用

v3必须独审锁定后才改candidate；精确paths另JSON。旧v2完整infra/Core/UI preflight是旧source范围证据，不提升v3。旧fullverify已按Root授权核实专属PGID57499后SIGINT，记录partial/terminated；Luna71820/Sakura3505/SSH34090不在目标组，未触碰。Main/真实DB/包/旧R62 WI和runtime输入不改，最终Root独审采用。


## 十一、V3.4：严格登记真实 history/status sidecar（编码前范围增补）

本段只补测试adapter输入闭包；前述V3.3全部产品/事务/legacy/attempt/owner语义维持。packet03因begin未重新采最新已知floor而撤回，补一行遵循第六节，旧包/partial记录不覆盖。另真实registered入口fresh nonce e066662c561e45c8a1098aea4b4762eb已执行指定Rust producer exit0，随后因目录包含未登记reading-status-history.json被拒，Node未执行；不能把受控Node100预检当注册通过。

精确追加源码范围仅`scripts/quality/reading_components.py`和`scripts/quality/test_reading_components.py`（M15/QA，C17验证闭包）。现25产品/测试路径不减；原4场景、原100RBC IDs、全部原有效断言、PNG检查/CSP/nonce/超时/固定Rust与Node argv均不删不弱化。无需改verify.py、storage注册、authority、baseline、debt、批准PNG/token/oracle或Main。

### 唯一扩展输入与严格身份

真实注册run必须要求manifest新增`readingStatusHistory`为exact `{path,sha256}`，path只能是`reading-status-history.json`。不接受null、未知key、缺失扩展、任意basename/嵌套/绝对路径/逃逸、symlink、丢失、过大（沿用每payload8MiB）或早于本次调用的文件。原4entries仍exact4，其原全部核对继续执行。输出目录仍exact manifest+原4payload+唯一sidecar；其他任何文件/目录仍拒。

sidecar从原始JSON字节解析，重复object key拒绝。metadata要求schemaVersion是int1（bool不算）、kind=`actual_sqlite_reading_unit_projection_history`、nonce exact wrapper32hex；sourceSha256必须exact以下8路径且每值为对应当前源SHA256，缺失/未知/错误hash一律拒：
- crates/app-core/src/application/runtime.rs
- crates/app-core/src/contracts/desktop.rs
- crates/app-core/src/domain/translation_status.rs
- crates/app-infra/src/storage/database.rs
- crates/app-infra/src/storage/queries/mod.rs
- crates/app-infra/src/storage/queries/tasks.rs
- crates/app-infra/src/storage/queries/translation_status.rs
- crates/app-infra/tests/application_runtime.rs

固定8源由adapter读取当前源取得，不信producer自行声明路径集合；原manifest两源身份仍逐项比较且须与8源一致。sidecar body保留真实DTO和实际historyPage opaque cursor，由Rust producer与原RBC38产品断言检验；adapter不实现head/status/分页业务或制造替代DTO，不因schema元数据验证成功宣称产品通过。

### Inspect API、receipt与执行一致性

原inspect helper的4场景-only调用可继续用于原guard负例；仅当调用方未提供新expected_history_sources且manifest无扩展时走既有合同，不能在真实run使用此legacy模式。真实run固定读取8源并通过新增keyword参数要求sidecar；有扩展但无expected8源必须拒。原guard mock run补真实结构sidecar与8源fake文件，只是guard数据，原断言保留且不计产品通过。

expected evidence附canonical `readingStatusHistory={path,sha256,sourceSha256}`。Node receipt同样附这三项，现有fileRootRelative只作可选定位字段；validate_report只忽略已知非权威locator，对这三项及source exact map逐值比较，缺失/额外未知身份key或错误值拒。原files仍exact4，无sidecar冒充第五场景。

同一次run在Rust完成后与Node完成后都独立inspect sidecar bytes/metadata/freshness，8源当前hash与full source tree也复核；Node改payload、manifest、source、目录或symlink仍拒。Node未实际执行或任一RBC失败时仍product_component_tests_executed=false。native/game/pixel三个False边界维持。

### 有意义验证与顺序

编码前保存两新增路径原字节/sha及当前候选source map；新设计和exact27paths经独审后才改这两文件，Root另决定Main采用。

原所有ReadingComponentGuards先原样保留断言，增正例与独立负例：合法sidecar完整通过；原4无扩展helper兼容；真实run缺扩展拒；null/错误shape/未知key/非法路径/symlink/过大/旧mtime/额外文件拒；payload hash/nonce/schema(kind/type)/重复key/8源缺漏额外及错hash拒；receipt少sidecar、错path/hash/source及未知身份字段拒，已知locator不能替代身份；mock Node改sidecar/8源均在post-inspection拒。

之后原registered入口实际生成fresh Rust fixture并运行全部100真实组件检查；完整npm verify包含原必要checks/nativeIPC/storage，不改命令或原oracle。旧storage carrier3指纹和lifecycle/source authority若仍拒，保留blocked，不自动刷新；本增补不授予那些路径。


## 十二、V3.5：按对象投影只接受明确正revision（编码前契约澄清）

独立packet05审阅实证P2：旧`unit(id,0)`具有latest别名语义，新projection复用它却用原入参0读lane，导致成功返回unit/state revision1而engine falsely Unrequested；明确revision1实际为Current failed。正常UI只传正revision且hook拒不匹配，不能据此允许Rust公共只读契约自相矛盾。

`reading_unit_projection(id,textRevision)`必须要求`textRevision>0`，0以typed AppError拒绝：code=`invalid_reading_revision`、stage=`reading`。错误文案说明翻译状态必须固定正原文版本，不尝试猜latest、不返回任何成功投影。u32反序列化原有负值/越界拒绝保持。参数验证在helper首个SQLite SELECT前执行；合法请求仍由既有Deferred transaction首读profile_meta pin一致快照，不改变事务顺序。

最窄实现仅现已批准`crates/app-infra/src/storage/queries/translation_status.rs`的函数入口验证和`crates/app-infra/tests/translation_current_status.rs`追加真实SQLite回归；现27paths不变、无新DTO/DDL/配置/UI逻辑。旧`reading_unit(id,0)`原latest语义完整保留；旧snapshot、history、serializedUnit、frozen与全部facts不因拒绝查询变化。新Tauri/Rust Runtime query继续透传同一typed错误，不在UI隐藏不一致。

TCS-011实际回归：同一真实unit明确rev1有Current failed，raw旧unit(id,0)仍解析rev1；新projection(id,0)必须Err上述code/stage，不能返回Unrequested或resolved unit；再查明确rev1仍同一真实task/attempt/error，查询前后task/attempt行数、snapshot/commit、原文/译文/frozen不变。运行全部既有15项状态测试并增加第16项，原所有有效断言保留。完整verify/注册组件/代码独审仍须基于新冻结源；packet05与原失败probe/partial回执保留，不能回填为通过。

此为原fixed-object显式版本合同的fail-closed澄清，Main/authority/包采用仍Root独占。代码前先冻结设计、独审、保存helper/test前像并落自有WI，再实现入口检查。后续先审完全部源码发现再启动慢全量，避免已知错误源重复运行。


## 十三、R63精确治理补充（候选，待独审）
原11TCS、1918 source-snapshot、产品27路径及原42生命周期守卫保持。新增final-cohort-registration delta追踪carrier三个当前来源、schema2 finalID与五条精确projection语义/critical link、真实catalog引用和UTC日期/公开契约增量。仅允许verification列出的确切authority路径，候选13类派生输出经独审后由Root采用；不把生成器运行当批准。database.rs两处已审runtime改动不变migration SQL/DDL/schema2；历史schema1/nestedreview/SQL hash拒绝保持。新finalR63登记不要求其WI此时存在，采用后新WI和正式新包/nonce另建；旧R59/R62不改。实际storage adapter与完整verify仍须运行，旧visual失败保留。


治理生成器显式source_evidence_additions仅向已有module/component/capability row追加；配置每项必须有真实product闭包内safe path、准确line/quote/hash与独审理由，重复/未知row/越界/错误身份全部拒绝。新增引用及理由进入exact-delta和manifest供独审；原112引用修复与4显式remap分开保留，不自动搜全库或删原引用。
