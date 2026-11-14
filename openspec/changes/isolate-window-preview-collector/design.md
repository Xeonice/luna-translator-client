# 独立采集进程设计：A阶段实施依据

状态：A阶段授权实施，B阶段未实施。以下原只读方案已由Root核对范围并纳入此工作项；其“待Root”历史措辞保留，授权以iteration-card为准。

# R39 建议：同一签名可执行文件的独立常驻采集进程

状态：只读方案，未实施、未改任何产品源或规范、未启动 PoC。沿用 M12 desktop-platform；不把取文 Core、翻译、语言 worker、SQLite 或 Tauri WebView 放入采集进程。原候选16周期故障及未获精确 terminal errorCode 的限制见同目录 diagnosis.md。

## 决策建议

先验证 `Luna.app/Contents/MacOS/luna-reader --capture-worker`：同一已签名 executable、同一个稳定安装路径，真实另一个常驻 PID。在 main 最早参数分派，worker 分支完全绕过 bootstrap/profile/DB/语言/Hook初始化、Tauri Builder、窗口恢复、单实例和WebView。只运行 M12 Rust 调度、有界私有 IPC、AppKit accessory RunLoop、系统picker/filter、SC/既有单窗口CLI兼容路径、独立只读 Win 身份核验。

父正常 App 在显式开发 PoC 开关下建立私有通信后直接 spawn 精确同 executable；不经 `open`/LaunchServices，避免系统将请求转给已有主实例。裸 worker 参数没有正确继承FD和随机握手立即失败，不申请权限、不弹picker、不打开DB。PoC开关不能默认为普通用户启动行为；这不是新增产品页面或公共Tauri命令。

同 exe/cert/designated requirement 可缩小签名身份变化，但不能口头保证 TCC responsible attribution、两个进程的picker归属和退出效果；必须用实际签名正式包先证明。不要让外部Terminal单独启动的helper成功代替“正常Luna父进程启动”的TCC结果。

| 方案 | 建议顺序 | 必须验证的限制 |
|---|---|---|
| 同exe早分派worker | 第一PoC；不用新bundle、新框架、新依赖 | 同bundle两个PID的picker/Control Center归属、TCC attribution、main关闭/退出行为 |
| 独立嵌入helper .app | 同exe实证失败后再评审 | 新CFBundleIdentifier/签名对象，可能需要独立一次授权；不能假定继承Luna权限 |
| XPC service | 不作为本轮第一实现 | 可用于进程通信，但未证明XPC进程能满足picker UI/RunLoop/TCC需要；不能借XPC传输既有filter来绕开 |

## 必须让picker、filter和采集在同一进程

本机Xcode macOS SDK声明 SCContentFilter、SCStream 为 NSObject，没有公开 NSSecureCoding/跨进程序列化合同；SCContentSharingPicker是应用级sharedPicker，observer回调交给应用真实filter。不能把opaque指针、Objective-C对象或filter token的数值当作授权跨进程传走。正确做法是：主UI明确点击选择→Rust应用权威发送请求→worker自己的主RunLoop呈现系统singleWindow picker→worker自己保留原filter并创建/更新采集源。父只收到不透明capture ID、精确身份/状态/帧。

这是对公开SDK的工程约束，不声称系统内部绝对不存在私有传递机制；本方案不用私有机制。保持已有全局录屏许可策略：在实际执行捕获的worker进程做preflight及真实帧核验；权限撤销/用户系统停止共享不走CLI降级。若worker权限与主App不同，明确投影实际授权主体并按系统要求授权，不能伪报“主App已授权所以helper可采”。

参考：[Apple SCContentFilter](https://developer.apple.com/documentation/screencapturekit/sccontentfilter)、[Apple SCContentSharingPicker](https://developer.apple.com/documentation/screencapturekit/sccontentsharingpicker)、[Apple WWDC23 picker/filter/stream流程](https://developer.apple.com/videos/play/wwdc2023/10136/)。本机SDK依据 SCStream.h:107/475、SCContentSharingPicker.h:74-96。

## 当前对象为什么不能直接搬成IPC

`VerifiedPreviewTarget.pairing.validate` 是 Rust `Arc<dyn Fn()>`，reading.rs构造的closure同时捕获主进程 `Arc<CoreHookBackend>` 和 `SelectedPreviewWindow`。前者读取主进程 candidates/preview_binding/client会话状态，后者持本进程 SCContentFilter registry token。Arc、闭包、锁、File和Native对象均不能作为此协议的可序列化所有权。复制token字符串到child只会引用child中不存在的registry，复制主进程Hook状态还会保留原耦合。

需要传的是新冻结目标描述，不是闭包：协议版本、请求ID、目标代次、完整 Windows PID/FILETIME/架构/EXE/HWND/class、已选Bottle canonical身份、expected PE vnode/严格文件stamp。worker从实际签名包发现自己的broker资源，复用现有资源校验；自行以只读方式打开PE并核对期待dev/ino/stamp后持有FD。主进程File/FD不序列化。实际帧只引用worker实例、capture target ID、代次与proof序号，不引用可变化的主进程session状态。

## M02身份业务复用，而非复制

复用 `hook-client::Client::new`（只构造状态，不会连接Host）、`Client::verify_bound_window`、`discover_launch_targets`、现有target_probe.exe与资源身份检查。worker绝不调用connect/associate/inject/disconnect、消费Core回调或构造真实Hook会话。

最小抽取位置为 `crates/app-infra/src/hook_backend.rs`：将现有 `verify_preview_pairing` 中纯外部核验抽成可复用的冻结context verifier（Win完整身份、PE文件、drive解析、同PE实例唯一性一份实现）。原CoreHookBackend方法继续先核对其本地trusted候选/association，再委托它；worker使用经完整复核后自有的冻结context，不读取父active/lease。新的Rust内部描述/方法不是公共Tauri DTO，也不改Hook Core ABI。不能把M02的session状态机复制到collector。

第一PoC可复用既有短时target_probe命令，所有只读probe都归worker自己的队列/实例，故其Rust WINDOW_PROOF_LOCK与父进程不共享。若真实pre/post成本导致周期达不到指标，下一小步才评审同target_probe的常驻只读请求模式；不提前改Windows broker或删减逐帧核验。独立进程可以隔离共享锁，不能宣称消除Wine/系统层资源竞争。

## 协议与Owner

采用匿名私有继承FD（控制/状态与帧分开），不监听TCP、不新增HTTP、不依赖XPC或新包。父只向已验证同exe子进程授予必要FD；其余FD close-on-exec，worker不能继承父DB连接或凭据。启动握手绑定协议/worker随机实例/PID及进程start；FD capability和单次随机nonce约束父子连接，迟到旧进程数据不能进入新实例。PE文件由worker自己打开并持有，经期待vnode验证，不通过JSON传FD数值。

建议消息：Hello/Ready，PrepareTarget，BeginSystemPicker（仅显式操作），CommitTarget(expectedEpoch)，Subscribe/Unsubscribe，Pause/Resume，ReleaseTarget，Shutdown；状态与帧统一带workerInstance/captureId/targetEpoch/sequence。控制请求先做Rust应用权威校验，UI订阅者不能任意指定CGID、启动进程或终止producer。文本头严格有界，JPEG仍≤524288字节，采用长度前缀二进制载荷，不base64放大全局snapshot。每目标只留最新一帧；慢订阅者丢旧帧，不反压控制线程或无界排队。具体总内存仍须实测/硬限证明，不能继承“8MiB已满足”。

控制串行点只提交epoch/取消标志，不能持锁执行probe/SC/CLI/编码。采集线程在同一目标的完整pre-proof→capture→post-proof完成后，只在workerInstance/epoch/native owner仍匹配时发布。保留源采集时间和核验时间；延迟的旧像素不能重标为刚采集。暂态proof忙/超时可暂停新帧发布、保留有时间的最后帧与配对，不伪报Live，也不销毁producer。明确身份冲突/窗口退出/TCC撤销/系统停止共享则停止该目标发布并释放对应filter，worker进程仍可在Idle等下次明确目标。

Windows逐帧完整身份、exact CGID/native PID+start、原filter及许可前后校验保留；helper metadata仍仅是用户显式配对的误选过滤，不升级成执行证明。一次成功的用户窗口配对在candidate→session只是消费者映射，不重复picker。换目标/取消迟到CAS不能影响旧目标或新owner。

## 生命周期与订阅

producer由App生命周期监督，一次启动，跨页签、UI卸载、Hook状态/会话更换与订阅数量保持独立。Hook disconnect不是Shutdown，主窗close不是退出；明确释放目标、真实目标退出、权限撤销有各自准确状态。正常应用Quit才发送有界Shutdown并回收自己那个Child，禁止匹配名字终止其他Luna/game。child异常显示一次真实故障，按受控策略恢复，不能无限重启/反复弹系统picker。worker重启会丢原filter，不能将旧token复用成授权。

父WindowPreviewService在B阶段变为client proxy+有界latest cache+消费者映射。UI仍经ApplicationPort attach/detach，不直接管worker；reading的1秒coordinator不再拥有采集起停，封面只是有效帧消费者，SQLite/翻译/语言故障不能阻塞worker。game_window_binding对当前已提交目标读取collector只读proof投影，不再每5秒新开主动probe并争用Hook lease；缓存投影只用于UI事实显示，不用于跳过下一帧的完整身份校验。换绑定、显示游戏等显式操作仍经Rust应用权威命令，订阅回调不自行变更采集目标。

## R39-A 最小PoC白名单（待Root正式授权）

1. `apps/desktop/src-tauri/src/main.rs`：最早worker分派；开发显式PoC开关下在正常App setup后创建父监督，不进入worker的Tauri流程。
2. `apps/desktop/src-tauri/src/platform/mod.rs`：模块注册。
3. 新 `apps/desktop/src-tauri/src/platform/window_collector/mod.rs`：worker入口、父监督和PoC明确启动。
4. 新 `apps/desktop/src-tauri/src/platform/window_collector/protocol.rs`：私有有界FD协议/代次/实例。
5. 新 `apps/desktop/src-tauri/src/platform/window_collector/worker.rs`：子进程调度、picker、采集、独立proof；仅产独立PoC回执和已授权验证画面。
6. `apps/desktop/src-tauri/src/platform/macos/mod.rs`。
7. `apps/desktop/src-tauri/src/platform/macos/bridge.m`：窄accessory RunLoop/唤醒/退出，复用已有Native采集边界，不重写捕获安全策略。
8. `crates/app-infra/src/hook_backend.rs`：上述冻结目标context和纯verifier抽取，原wrapper语义不变。

A阶段不改公开IPC/前端/Cargo/Core/DB/Windows broker/既有CLI文件/批准图；也不将旧原生producer与PoC producer一起当产品双采集。父PoC使用独立显式请求，普通main功能照常，正常启动不开PoC picker。真实游戏输入/target描述从现场生成并复核，禁止写死PID/游戏名。A只证明系统与进程能力，不冒充产品UI已集成。

## A通过后，B阶段另授权的集成增量

追加：`bootstrap.rs`（管理一份父监督）、`platform/window_preview.rs`（proxy/cache与既有control适配）、`ipc/reading.rs`（消费者/封面与显式命令，不再采集watcher）、`ReadingPanel.tsx`（只读状态/临时失败保留）、其真实组件测试与`tests/support/design-port.ts`。如端口保持现有公开签名，不增application-port/tauri-port；确需公共DTO改变再单独评审，不能先扩。部署与资源ledger/schema要随精确改变登记，但A没有新二进制或依赖。

## 逐层验收（现在全部not_run）

- worker早分支无正常bootstrap/DB/Host/语言/WebView，裸参数拒绝；实际两个PID、精确签名与DR回执。
- 从真正Luna父进程启动，worker实际preflight+系统singleWindow picker+真实樱之诗frame；核对TCC attribution与系统共享停止；不能用外部工具预检替代。
- 主App正常Hook和每5秒旧窗口轮询、页签卸载/回挂、文本推进期间，worker同PID/instance/captureId/epoch持续，序号与真实画面变化一致。全量source/proof/capture/last-success时间和typed错误有界记录，明确区分UI隐藏与producer故障。
- 挂起/减慢父订阅、模拟DB/翻译忙、无订阅时worker不被反压或stop；慢消费者缓存有界，控制可取消旧epoch，过期帧不转live。
- 完整Win身份变化、CGID/PID复用、错误窗口/同EXE多实例、许可撤销、系统停止共享、取消/迟到回调保持fail-closed；强验证不因进程隔离删掉。
- Hook结束而目标仍运行不终止worker；目标退出只释放目标；正常Quit有界收尾无孤儿；worker崩溃不进入重启循环，不复用旧filter。
- 原四游戏与正式包连续画面、封面/主窗/HUD/取文/词法回归留给B及其最终同源包；A绿不提升发布、视觉、全局内存或游戏产品验收。
