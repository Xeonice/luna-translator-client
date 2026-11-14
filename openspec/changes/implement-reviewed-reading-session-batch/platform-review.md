# 平台实施审阅与资源政策

审阅者Root；实施者native_window_batch；日期2026-10-20。依据已确认架构中的M12、RW01～12/24～25。审阅不代表原生实机或发布通过。

- 采用已锁定`cc=1.8.0`的macOS条件build依赖，编译同进程Objective-C薄桥；只链接系统Cocoa/CoreGraphics/ScreenCaptureKit/CoreMedia/CoreVideo/ImageIO/Carbon。不新增运行框架、服务、Swift/Python daemon，不变Core ABI1/JSONL2。Cargo/build/技术锁的精确变化仍在最终独立基线差异核对。
- 预览使用macOS14及以上`SCScreenshotManager`及`SCContentFilter(desktopIndependentWindow:)`；低版本明确unsupported，不抬升原应用最低系统要求。官方API/本机SDK核对由native计划留证。仅显式开启触发权限；拒绝不影响取文。
- 冻结PreviewPolicy：最多2租约、1个同时采集；每租约目标1秒间隔；有效采集4秒期限；最大960×540、JPEG512KiB、每租约一个最新帧槽、应用自有帧预算8MiB；启动5秒、停止3秒。页面隐藏/离开停止相应租约；不音频、不默认保存、不进入AppSnapshot/数据库/全局事件。
- 允许已验证Win32目标窗口的有界一次性随机标题挑战，用于建立HWND到CGWindowID/宿主PID和创建身份的证明。仅对服务签发候选，前后复核完整Win身份/Bottle/窗口归属，短期限内恢复原题，验证恢复，nonce唯一匹配。不得长期改游戏标题、纯同名猜测、整屏裁剪或抬起窗口。异常应恢复/报告不可用，不标实时成功。
- Native Owner扩展至target_probe.rs及hook-client/window_binding.rs的窄挑战辅助。app-infra/hook_backend由backend Owner串行接公共验证方法，Root负责IPC装配，不并发改同一文件。探针注释同步表达新增有界变更能力，不能继续把全部模式称只读。
- 快捷键采用Carbon真正系统注册，3 intent沿设计候选OptionCmdH/P及显示模式未设置；空字符串明确禁用。唯一coordinator prepare→durable偏好提交→commit/清理，失败补偿保留旧effective；注册候选不提前派发。只按下边沿，录入时抑制。平台不自行读SQL/解析业务scope。
- 几何使用原子平台偏好文件，记录真实native bounds、逻辑坐标/scale/显示器身份，支持负坐标与可见手柄夹取。实际无第二屏时只完成纯逻辑测试，不伪称拔屏/多屏硬件通过。
- 原生材质及系统reduce-transparency实际投影；透明背景不能用游戏壁纸代替。UI文字alpha独立。

具体DTO由`contracts/window_capabilities.rs`权威定义并由Root导出TS；候选草图见`.build/next-batch-20261020/native/plan.json`。原生预览、global shortcut、拖动、恢复和材质需同一最终包实测，不据源码或浏览器通过。
