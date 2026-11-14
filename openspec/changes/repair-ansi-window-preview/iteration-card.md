# ANSI窗口预览修复工作卡

2026-10-23，用户明确要求处理解决缺陷。基线FP1-20261023-R35；开发前source-snapshot冻结。

- 需求FP-03/04/12；完整整稿保留，新增状态候选为docs/design/first-product-v1/ansi-preview-recovery.html#native-pairing，M02 hook-client、M12 desktop-platform、M13 product-ui；组件映射以现有engineering-components目录实际核对后登记。
- Root：本change治理、方案集成、唯一GUI/安装、实机回归；UI worker：WindowPreview.tsx与三份既有组件测试文件；实验worker：仅.build/window-preview-sakura-20261023/gdi-experiment，未授权生产修改。
- 允许路径见verification.json，不做其他业务修复。新增strategy在实验成功后明确字段/模块/测试再启动产品代码。
- 依赖：现有llvm-mingw、CrossOver26.3、已授权录屏的稳定签名Luna、可推进真实游戏及桌面；Steam保留另一台会话，不登录/接管、不改宽高/DPI/图形、不重启bottle。
- 完成：真实ANSI目标画面连续变化，标题/几何完整不变；失败状态终结、显式重试/权限恢复、退出/取消/超时/错误身份/预算均验证；正式包资源与签名身份、工程检查和实际Hook/分词词卡分开记录。原视觉源漂移仍可阻断总体发布，不把编译或实验当全部完成。
- 原则：不删ANSI保护，不猜native窗口，不伪造帧；不更改批准图、token或放宽门禁；无提交推送。

## 2026-10-23 实验与选型决定（产品编码前）
真实GDI前台/后台8次均失败或纯黑，回执 .build/window-preview-sakura-20261023/gdi-experiment/handoff.json。自动唯一宿主映射被独立评审阻塞（header mmap不证明execute、Bottle未知、6个native窗口、race），不采用。保持Unicode随机challenge及ANSI写前拒绝。

采用macOS15.2+官方SCContentSharingPicker SingleWindow作为显式用户跨命名空间配对恢复。它是用户选择并核对的映射，不宣传自动证明；关联前预览可让用户核对，选定后常驻自动取帧。由明确『选择对应的游戏窗口…』意图打开，普通刷新/轮询不隐式弹系统picker。新增一个窄IPC application_preview_select_window，复用PreviewStartRequest→PreviewProjection，ApplicationPort selectWindowPreview(request,listener)，无新DTO/业务Command/存储字段。主窗口角色、当前candidate/session/epoch在native效果前后校验。

失败控制槽是内部Pending/Failed/Resolved backing，opaque控制句柄+generation不表示采集已建立。仅Resolved允许捕获/封面。Failed的既有resume只重评估普通启动；manual-pair-required不隐式弹picker，stop正确释放。系统picker仅一个事务、target epoch、90秒有界、回调cancel/fail/late隔离；主线程仅present/管理picker不等待，background不持锁等待。限定SingleWindow、allowsChangingSelectedContent=NO、filter.style Window且includedWindows恰1，保留原filter用于capture（不把includedDisplays/Applications空未经实测作为断言）。excluded IDs只帮助选取，不当安全证明；TCC原门控和每帧复核继续。

选后冻结精确CGWindowID、ownerPID/start、opaque filter资源；Window HWND完整身份前后核对，目标实际PE vnode(dev/ino)只作误选过滤，不称执行身份证明。当前KERN_PROCARGS2未含WINEPREFIX，不能冒称已证；允许有可核验环境时交叉核对，否则运行App实际bundle/helper BottleName+BottleTag与所选canonical Bottle的BottleID前后身份/内容一致作为launcher元数据过滤并如实标注。多个同实际PE活Windows进程、证据未知/冲突均拒绝；owner辅助窗的语义由用户在预览中明确核对，不自动猜最大窗口/标题。资源取消/停止/目标关闭/换代释放精确filter；每帧检查Win绑定、ownerstart、native对象、支持的关联证据与generation。SC实际可捕获离屏窗口的行为必须实测，不靠单纯删除onScreen检查声称解决。

Owner：Native worker统一拥有hook-client/window_binding.rs、app-infra/hook_backend.rs、platform/window_preview.rs、ipc/reading.rs、macos/mod.rs+bridge.m、main.rs、bridge/application-port.ts+tauri-port.ts（如需target_probe/lib.rs新增逻辑先说明）。UI worker继续仅WindowPreview.tsx/现有3组件QA，并增加tests/support/design-port.ts用于类型完整的显式picker受控端口。不改Core/语言worker、资源锁/依赖；Root拥有PRD/技术设计/独立HTML候选和治理。所有人不覆盖他人修改。

## Native同路径复用补充
Root批准Native worker仅将platform/game_launch.rs既有drive_mappings和resolve可见性pub(super)改pub(crate)，供hook_backend复用已存在Bottle驱动映射/EXE解析；不复制解析器、不改行为，纳入实际diff与独审。

## 治理前置缺口修正（2026-10-23T13:18:54.048969+00:00）
Root检查发现最初脚本只创建delta目录但未写spec.md，verification已有四个scenario引用。此时UI第一批源已写、Native实施进行中；不能宣称全部前置在最初编码前已满足。现将原计划四scenario及已决定picker细节写入实际delta文件，保留这一发生顺序，不伪造初始完成时间。后续统一verify必须检查实际文件及映射。

## 2026-10-23 单窗口系统CLI后备（实际取图后新增）
Root精确已知game CG80973/owner88536实际SC offscreen返回-3811；Apple /usr/sbin/screencapture -x -l80973 -tjpg命名临时目标实际成功，图与CUA完整游戏樱花/抬棺/日文一致，回执sc-experiment/system-cli-receipt.json，输出SHA085d8204302947ac8c7a79618eb0c8608637c8ed34afdbe184102fa19ba291fc。stdout/devfd/shm不支持，不能声称内存纯传输；exit0但0字节/错误stderr必须拒绝。此实验不是正式包通过。

Root接受独立code review的有界兼容策略条件，允许新增private macos/window_capture.rs（Native Owner）作为Apple单窗口CLI adapter，保持原SC优先、仅已保留明确picker filter的目标在具体兼容错误/离屏返回时后备。绝不为权限/身份/取消错误降级。全局TCC+Win/native完整前后核验保持，不使用shell、普通标题、桌面或-R矩形裁剪。

最多一个在途缓冲，独立0700不可猜目录，child仅在pre_exec设置umask077，使Apple工具输出0600文件；禁止symlink/外来文件，先根据native source pointPixelScale与尺寸检查总像素上界，再spawn绝对路径系统程序。真实Apple工具会原子替换预建文件，因此不要求写旧inode；child完成后O_NOFOLLOW绑定新regular文件，核uid/dev/nlink/0600、父目录身份和读取前后新inode稳定。运行期输出文件字节和墙钟时间有界，只清理己方child并wait；检查图像格式/尺寸/pixels/bytes后用内存codec归一至现有960x540/512KiB JPEG。成功时间取真实完成，错误不得制造帧/封面；finally清理文件/目录，异常退出遗留只能清理本adapter明确拥有且无运行owner的限定目录。

这属于短期图像处理缓冲而非截图历史；技术/授权说明如实声明临时处理后立即清除，不再绝对声称从不写盘。用户长期保存/上传政策不扩大，仍仅正式关联后的首个有效帧可作封面。规划总8MiB内存门禁未实现的旧限制保持，不冒称本fallback已满足总预算。冻结预算：源图最多4,194,304像素、输入最多2MiB、系统子进程最多3秒、输出至多960×540且524,288字节；一笔在途，25ms仅执行cheap native/TCC/代际校验，完整Win probe在帧前后执行。child RLIMIT_FSIZE限制2MiB；实际耗时、Luna授权与游戏连续帧仍须正式包记录。


## 关联提交时的精确配对移交与并行Owner
Root批准Native worker仅修改`ipc/commands.rs::application_associate`：已有业务关联成功后、回复UI前调用reading层窄移交，不新增公共IPC或业务命令。仅精确同一Windows selection identity且generation/CAS成功的已确认候选配对可转为新会话consumer；候选卸载的旧stop不能释放新会话，取消候选不能改变旧会话。此移交避免用户在绑定前后重复系统选窗。

并行Owner细化：UI worker在已有UI变更完成后，独占新增`platform/macos/window_capture.rs`及其单文件测试；Native worker拥有mod.rs/bridge.m及调用集成，双方先约定API，不能覆盖对方。Root继续拥有规范和最终安装/GUI。R36治理准备仅输出独立`.build`候选，未独审、源码未冻结前不采用。


## R37：candidate14实机发现的正向映射见证修复
candidate14同签名包实机明确返回`owner_image_mapping`，用户确认选窗后无效。只读复现真实game owner88536：3171regions/约39ms，在index756已找到目标PE的4KiB精确vnode，扫描末尾0/EINVAL却被旧complete条件拒绝。此回执与candidate14失败原件保留。

Root批准Native worker仅修bridge.m的有界正向映射过滤：精确dev/ino见证找到立即成功；未找到、短读、无进展、预算或身份不稳仍拒绝。该header仍仅误选过滤，不升级执行或Bottle证明，不取消显式系统选择；其他完整Win/native身份与TCC/共享停止/输出预算保持。增加实际Darwin mmap前/中/后与另一inode负例，真实game/Luna对照、ObjC/Native/desktop/Clippy定向复测，独立review后Root精确采用R37。R36逐字归档，原R35 source-snapshot不更新，不覆candidate14或原视觉失败。


R37永久回归补充授权：Native同时拥有macos/mod.rs的cfg(test)测试声明与真实Darwin mmap/unmap测试；bridge新增仅借用pid和live owned file fd的private readonly FFI wrapper以调用同一OwnerMapsImage，不接路径、不暴露公共IPC/DTO，不加依赖。测试在原有product-native-ipc-tests中实际执行，覆盖映射前/映射中/另一inode/解除映射后；原31策略用例及独立scratch结果仍保留各自有限范围。R37最终产品差异严格为这两个文件。


## R38：持续预览的临时竞争与目录身份
candidate15实机已能明确选窗并由用户观察到预览，但画面变化后预览丢失；Root实际会话最终回到ANSI初始selection_required而无帧，不能据此认定唯一原因。独立只读发现确定的错误路径：正常每5秒绑定核对持lease跨probe，coordinator每秒try_lock返回capture_cleanup_pending却停止所有Session并清key。另Bottle目录的size/mtime被跨帧冻结，Wine原子写注册表可改变父目录时间；目录本身identity未变仍可能被误拒。00:10:30实际system.reg和父目录mtime相差约0.25ms的事实保留，不能冒充已持有此前pair证据。

Root批准Native仅在reading.rs、bridge.m及必要mod.rs/window_preview.rs既有测试接线中修正：临时capture_cleanup_pending保留已确认pair/key；consumer_update读取错误不当不存在而重启。真实身份终态仍释放，逐帧Win/native/TCC/共享停止校验与边界不降级。Bottle目录跨帧仅冻结canonical根/dev/ino/目录类型，普通目录内容size/mtime变化不作身份变更；真正配置/启动器文件的稳定读取、hash/字段核对及目录替换拒绝保留。固定安全诊断不能泄露路径/环境或用自由文本决定恢复。

永久回归必须调用同一生产决策并验证service lease/generation/filter保留、正常绑定轮询竞争、读取错误不启动新预览、真正终态释放；真实临时目录模拟原子注册表写入与目录替换。无新UI、公共DTO、DB、Core/worker、依赖、批准图或视觉阈值。Root负责R37原字节归档/R38精确采用/下一同签名包与同游戏连续帧实机；candidate14/15及统一视觉失败保留。


R38测试producer补充：批准Native对crates/app-infra/src/hook_backend.rs现有cfg(test)绑定测试增加真实backend.lease持锁时cover_session_guard精确返回capture_cleanup_pending的断言，无生产逻辑/锁API变更。desktop层使用该已证typed输入，调用实际coordinator执行helper及真实WindowPreviewService，分层验证保留/释放/不启动；读取错误用真实poisoned Mutex产生，不能复制锁实现或只断言字符串。最终R38源码集合为上述4源加本infra test-only源，精确5源。


R38最终范围核对：infra的第5路径许可未使用。已有`natural_exit_shuts_down_owned_child_and_keeps_lease_until_fifo_ack`已真实覆盖持lease返回capture_cleanup_pending，生产者源逐字未变；最终产品增量严格为reading.rs、bridge.m、macos/mod.rs、window_preview.rs四源。candidate lookup读取错误同样只诊断并暂缓，属于同文件防御性一致性处理，不认定它是候选15竞争根因。实际目录测试额外暴露Foundation对/var规范化差异；保留首次103过1败日志并修生产realpath前后核对，不修改预期规避错误。

四源冻结后Desktop105/105（新增7项永久回归）、all-target Clippy、ObjC语法、31项Native策略、真实Darwin映射harness和既有真实producer测试均通过；独立复核与原包15运行失败另留独立回执。分层producer+生产coordinator helper/实际WindowPreviewService组合不宣称完整GUI竞争已复现。R38采用/候选16打包与正式游戏连续帧仍待执行；原视觉源漂移、8MiB全局预算及未完成验收不提升。无新UI/公共契约/DB/依赖/资源/Core变化。
