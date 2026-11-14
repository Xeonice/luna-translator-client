# ANSI窗口采集技术设计

状态：实现中；已按真实失败实验放弃GDI与自动宿主推定，采用下文明确的系统单窗口配对及特定兼容故障后备。尚未完成最终包实机。

## 保留的边界
Rust业务权威、Tauri窄适配、独立Core/语言worker不变。UI只用ApplicationPort；捕获是现有platform/runtime职责，不把帧混入Core text协议或DB。Mac原生路径继续使用精确已核验CGWindowID。ANSI路径通过用户明确系统选窗连接Windows完整目标身份与Mac窗口身份；原probe只读核验Windows目标，保留原native filter，不将HWND强转CGWindowID。

## 身份与生命周期
候选仍冻结Bottle+EXE+PID+创建FILETIME+架构+HWND+class。帧前后重验全身份；不存在、不匹配、不可采集、格式/尺寸/耗时超限均结构化失败。优先精确native filter；特定SC兼容失败仅对同一已明确选定CG窗口调用系统单窗口工具，禁止桌面截图和屏幕矩形裁剪。隐藏/最小化的实际可采集性由平台返回和有效帧验证，不硬编码成功。单一在途、latest-only、generation/epoch失效隔离、目标切换/关闭不混帧。helper超时只清理己方child，不碰游戏/bottle。原生permission每次取帧复核且继续覆盖后备路线。

## 状态
候选启动失败、会话启动失败发布可观察typed状态，区分权限/永久不支持/临时故障/目标失效。永久失败禁止定时自动重复启动；身份/能力变化或显式重试重新评估。UI不同时展示starting和失败。失败无成功时间/帧/自动封面。

## 资源与测试
有界图像预算、解析完整性与精确配对在下文及最终源码冻结；复用原Mac内存图像codec输出既有JPEG DTO。新增唯一显式选窗IPC已列明，原版GDI仅留研究失败记录，当前CrossOver实际画面才决定可用。组件测试不得替代正式包实测；当前视觉source_drift不改批准图/阈值规避。

## 前端传输恢复细化
保留既有RBC-46：有lease的preview_transport允许一次有界自动重新启动；目标变化、显式刷新或权限从非granted恢复granted重置预算。永久typed失败与首次start IPC拒绝不自动重复启动。无lease的失败显式刷新发一次start，不能误走空control。C13/C14/C35为现有组件映射，C10为设置侧栏不纳入。

## 2026-10-23 实验与选型决定（产品编码前）
真实GDI前台/后台8次均失败或纯黑，回执 .build/window-preview-sakura-20261023/gdi-experiment/handoff.json。自动唯一宿主映射被独立评审阻塞（header mmap不证明execute、Bottle未知、6个native窗口、race），不采用。保持Unicode随机challenge及ANSI写前拒绝。

采用macOS15.2+官方SCContentSharingPicker SingleWindow作为显式用户跨命名空间配对恢复。它是用户选择并核对的映射，不宣传自动证明；关联前预览可让用户核对，选定后常驻自动取帧。由明确『选择对应的游戏窗口…』意图打开，普通刷新/轮询不隐式弹系统picker。新增一个窄IPC application_preview_select_window，复用PreviewStartRequest→PreviewProjection，ApplicationPort selectWindowPreview(request,listener)，无新DTO/业务Command/存储字段。主窗口角色、当前candidate/session/epoch在native效果前后校验。

失败控制槽是内部Pending/Failed/Resolved backing，opaque控制句柄+generation不表示采集已建立。仅Resolved允许捕获/封面。Failed的既有resume只重评估普通启动；manual-pair-required不隐式弹picker，stop正确释放。系统picker仅一个事务、target epoch、90秒有界、回调cancel/fail/late隔离；主线程仅present/管理picker不等待，background不持锁等待。限定SingleWindow、allowsChangingSelectedContent=NO、filter.style Window且includedWindows恰1，保留原filter用于capture（不把includedDisplays/Applications空未经实测作为断言）。excluded IDs只帮助选取，不当安全证明；TCC原门控和每帧复核继续。

选后冻结精确CGWindowID、ownerPID/start、opaque filter资源；Window HWND完整身份前后核对，目标实际PE vnode(dev/ino)只作误选过滤，不称执行身份证明。当前KERN_PROCARGS2未含WINEPREFIX，不能冒称已证；允许有可核验环境时交叉核对，否则运行App实际bundle/helper BottleName+BottleTag与所选canonical Bottle的BottleID前后身份/内容一致作为launcher元数据过滤并如实标注。多个同实际PE活Windows进程、证据未知/冲突均拒绝；owner辅助窗的语义由用户在预览中明确核对，不自动猜最大窗口/标题。资源取消/停止/目标关闭/换代释放精确filter；每帧检查Win绑定、ownerstart、native对象、支持的关联证据与generation。SC实际可捕获离屏窗口的行为必须实测，不靠单纯删除onScreen检查声称解决。

Owner：Native worker统一拥有hook-client/window_binding.rs、app-infra/hook_backend.rs、platform/window_preview.rs、ipc/reading.rs、macos/mod.rs+bridge.m、main.rs、bridge/application-port.ts+tauri-port.ts（如需target_probe/lib.rs新增逻辑先说明）。UI worker继续仅WindowPreview.tsx/现有3组件QA，并增加tests/support/design-port.ts用于类型完整的显式picker受控端口。不改Core/语言worker、资源锁/依赖；Root拥有PRD/技术设计/独立HTML候选和治理。所有人不覆盖他人修改。

## 2026-10-23 单窗口系统CLI后备（实际取图后新增）
Root精确已知game CG80973/owner88536实际SC offscreen返回-3811；Apple /usr/sbin/screencapture -x -l80973 -tjpg命名临时目标实际成功，图与CUA完整游戏樱花/抬棺/日文一致，回执sc-experiment/system-cli-receipt.json，输出SHA085d8204302947ac8c7a79618eb0c8608637c8ed34afdbe184102fa19ba291fc。stdout/devfd/shm不支持，不能声称内存纯传输；exit0但0字节/错误stderr必须拒绝。此实验不是正式包通过。

Root接受独立code review的有界兼容策略条件，允许新增private macos/window_capture.rs（Native Owner）作为Apple单窗口CLI adapter，保持原SC优先、仅已保留明确picker filter的目标在具体兼容错误/离屏返回时后备。绝不为权限/身份/取消错误降级。全局TCC+Win/native完整前后核验保持，不使用shell、普通标题、桌面或-R矩形裁剪。

最多一个在途缓冲，独立0700不可猜目录，child仅在pre_exec设置umask077，使Apple工具输出0600文件；禁止symlink/外来文件，先根据native source pointPixelScale与尺寸检查总像素上界，再spawn绝对路径系统程序。真实Apple工具会原子替换预建文件，因此不要求写旧inode；child完成后O_NOFOLLOW绑定新regular文件，核uid/dev/nlink/0600、父目录身份和读取前后新inode稳定。运行期输出文件字节和墙钟时间有界，只清理己方child并wait；检查图像格式/尺寸/pixels/bytes后用内存codec归一至现有960x540/512KiB JPEG。成功时间取真实完成，错误不得制造帧/封面；finally清理文件/目录，异常退出遗留只能清理本adapter明确拥有且无运行owner的限定目录。

这属于短期图像处理缓冲而非截图历史；技术/授权说明如实声明临时处理后立即清除，不再绝对声称从不写盘。用户长期保存/上传政策不扩大，仍仅正式关联后的首个有效帧可作封面。规划总8MiB内存门禁未实现的旧限制保持，不冒称本fallback已满足总预算。冻结预算：源图最多4,194,304像素、输入最多2MiB、系统子进程最多3秒、输出至多960×540且524,288字节；一笔在途，25ms仅执行cheap native/TCC/代际校验，完整Win probe在帧前后执行。child RLIMIT_FSIZE限制2MiB；实际耗时、Luna授权与游戏连续帧仍须正式包记录。


## 关联提交时的精确配对移交与并行Owner
Root批准Native worker仅修改`ipc/commands.rs::application_associate`：已有业务关联成功后、回复UI前调用reading层窄移交，不新增公共IPC或业务命令。仅精确同一Windows selection identity且generation/CAS成功的已确认候选配对可转为新会话consumer；候选卸载的旧stop不能释放新会话，取消候选不能改变旧会话。此移交避免用户在绑定前后重复系统选窗。

并行Owner细化：UI worker在已有UI变更完成后，独占新增`platform/macos/window_capture.rs`及其单文件测试；Native worker拥有mod.rs/bridge.m及调用集成，双方先约定API，不能覆盖对方。Root继续拥有规范和最终安装/GUI。R36治理准备仅输出独立`.build`候选，未独审、源码未冻结前不采用。
