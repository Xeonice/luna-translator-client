# 修复 ANSI 游戏窗口预览与失败状态

## Why
用户于2026-10-23明确要求解决樱之诗窗口预览缺陷。candidate13真实截图返回window_title_proof_requires_unicode，当前按临时标题挑战映射Mac窗口的路径在ANSI窗口前拒绝，尚未开始取帧。UI同时残留正在开启并每3秒反复重试。恢复FP-03/04/12既有自动常驻预览，不新增产品模式。

## What Changes
已用独立有界GDI实验核对，实际失败/纯黑，明确不采用；自动宿主映射也因证据不足被拒绝。采用macOS15.2+显式系统单窗口配对，保留原filter及完整Win/native身份；SC优先，仅具体兼容/离屏失败可用Apple同CGID单窗口CLI后备。临时处理文件以私有权限、有界预算和立即清理管理，不建立截图历史。复用Rust预览租约/generation/单帧/封面逻辑，候选成功关联后精确移交session。保留TCC门控及原标题/几何，不把实验替代正式包。
失败由Rust权威投影为typed终态；候选UI启动拒绝也必须结束starting并停止永久失败的重复尝试，目标/权限变化或明确重试可恢复。会话页面不可只返回generic pending。

## Scope
精确允许路径见verification.json，新增内部采集模块需独立架构审阅。Root负责治理/集成/安装GUI，UI worker拥有WindowPreview及现有组件测试，并在API约定后独占新private macos/window_capture.rs；Native worker负责调用集成，具体窄路径和职责以迭代卡为准。改变实际实现策略先补design/work card。原Core、语言worker/词典、DB、模型、依赖锁、材质token、批准截图保持；现有R35冻结指纹不预刷新。当前canonical视觉源漂移失败保留。

## Verification
纯逻辑/真实组件、实际Windows API helper x86/x64和同签名正式包实机分别验收。包括ANSI/Unicode、同宿主重复标题、错误身份/退出/句柄复用、超时/预算/取消、权限、无无限重试、封面只来自有效帧以及Hook/分词词卡隔离。没有执行写not_run。不得继承candidate13或旧包结果，不提交推送。

## 2026-10-23 实验与选型决定（产品编码前）
真实GDI前台/后台8次均失败或纯黑，回执 .build/window-preview-sakura-20261023/gdi-experiment/handoff.json。自动唯一宿主映射被独立评审阻塞（header mmap不证明execute、Bottle未知、6个native窗口、race），不采用。保持Unicode随机challenge及ANSI写前拒绝。

采用macOS15.2+官方SCContentSharingPicker SingleWindow作为显式用户跨命名空间配对恢复。它是用户选择并核对的映射，不宣传自动证明；关联前预览可让用户核对，选定后常驻自动取帧。由明确『选择对应的游戏窗口…』意图打开，普通刷新/轮询不隐式弹系统picker。新增一个窄IPC application_preview_select_window，复用PreviewStartRequest→PreviewProjection，ApplicationPort selectWindowPreview(request,listener)，无新DTO/业务Command/存储字段。主窗口角色、当前candidate/session/epoch在native效果前后校验。

失败控制槽是内部Pending/Failed/Resolved backing，opaque控制句柄+generation不表示采集已建立。仅Resolved允许捕获/封面。Failed的既有resume只重评估普通启动；manual-pair-required不隐式弹picker，stop正确释放。系统picker仅一个事务、target epoch、90秒有界、回调cancel/fail/late隔离；主线程仅present/管理picker不等待，background不持锁等待。限定SingleWindow、allowsChangingSelectedContent=NO、filter.style Window且includedWindows恰1，保留原filter用于capture（不把includedDisplays/Applications空未经实测作为断言）。excluded IDs只帮助选取，不当安全证明；TCC原门控和每帧复核继续。

选后冻结精确CGWindowID、ownerPID/start、opaque filter资源；Window HWND完整身份前后核对，目标实际PE vnode(dev/ino)只作误选过滤，不称执行身份证明。当前KERN_PROCARGS2未含WINEPREFIX，不能冒称已证；允许有可核验环境时交叉核对，否则运行App实际bundle/helper BottleName+BottleTag与所选canonical Bottle的BottleID前后身份/内容一致作为launcher元数据过滤并如实标注。多个同实际PE活Windows进程、证据未知/冲突均拒绝；owner辅助窗的语义由用户在预览中明确核对，不自动猜最大窗口/标题。资源取消/停止/目标关闭/换代释放精确filter；每帧检查Win绑定、ownerstart、native对象、支持的关联证据与generation。SC实际可捕获离屏窗口的行为必须实测，不靠单纯删除onScreen检查声称解决。

Owner：Native worker统一拥有hook-client/window_binding.rs、app-infra/hook_backend.rs、platform/window_preview.rs、ipc/reading.rs、macos/mod.rs+bridge.m、main.rs、bridge/application-port.ts+tauri-port.ts（如需target_probe/lib.rs新增逻辑先说明）。UI worker继续仅WindowPreview.tsx/现有3组件QA，并增加tests/support/design-port.ts用于类型完整的显式picker受控端口。不改Core/语言worker、资源锁/依赖；Root拥有PRD/技术设计/独立HTML候选和治理。所有人不覆盖他人修改。
