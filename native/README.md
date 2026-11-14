# 保留的原生边界

当前客户端使用独立裁剪仓库 [luna-hooks-core](https://github.com/Xeonice/luna-hooks-core)，
固定提交 `e9242ebac32a09e911f306b7c90714891c217302`。准确文件、架构、大小、SHA-256
与许可文件清单以 [`core-runtime.lock.json`](core-runtime.lock.json) 为准；裁剪边界见
[ADR-002](../docs/architecture/ADR-002-source-trimmed-hooks.md)。

## 当前运行与打包链

每个架构各保留 `LunaHostCore32/64.dll`、`LunaHookCore32/64.dll` 和
`LunaInjectCore32/64.exe`，共六个 Core 文件。Rust Windows broker 与目标进程使用相同位数，
通过 `lunahooks-core-abi1-cdecl-seven` C ABI（版本 1）注册七个回调：进程连接、进程断开、
文本流创建、文本流销毁、文字输出、Host 信息、Hook 插入。签名依据为
[`crates/lunahook-host/src/abi.rs`](../crates/lunahook-host/src/abi.rs)。

macOS 客户端通过 CrossOver 启动对应 Windows broker，以版本 2 JSONL 交换命令与事件；
macOS 进程不直接加载 Windows DLL，也不启动原版 Python、Qt 或 WebView 客户端。
Hook 接入、关联身份和收尾由 Rust 适配层负责，窗口画面采集由独立采集器处理。

正式打包入口 [`scripts/build_desktop.py`](../scripts/build_desktop.py) 调用
[`scripts/resources/prepare_core_runtime.py`](../scripts/resources/prepare_core_runtime.py)：
先校验 Core checkout 的固定 HEAD 和已跟踪文件无修改，再核对六个二进制及六份许可文件，
构建 x86/x64 broker 和目标探针，按源码指纹及构建回执确认位数与字节身份，最后复制资源、
许可和 `core-staging-receipt.json`。`--skip-windows-build` 仍要求匹配源码的既有构建回执，
不能用旧原版 Host 替换当前 Core。该入口还准备独立语言 worker、语言资源并完成 macOS 应用打包。

Host 的生命周期由所属 broker 管理；退出需要真实生产者停止和事件排空确认，不以请求返回
或窗口消失代替完成。ABI、资源或 Core 升级须单独评审，并验证真实游戏回调；编译、资源校验
和 Host 初始化成功均不等于已收到台词。

## 许可与历史迁移资料

Core 锁文件列出原仓库 LICENSE、THIRD_PARTY_NOTICES、CMakeHelpers、Windows classic samples、
MinHook 和 uchardet 六份许可材料。打包时按锁文件验证并保留其内容；仓库原有 License 与
声明继续保留。

[`upstream.lock.json`](upstream.lock.json) 保留原版 LunaTranslator **v12.1.2.3**
（提交 `c8701872a5de83f8d3272f3967457d0b8f2983c4`）的历史迁移身份。
[`scripts/prepare_lunahook_runtime.py`](../scripts/prepare_lunahook_runtime.py) 属于旧原版十回调
Host ABI 的准备路径；其旧十文件资源清单及
[`scripts/smoke_headless_host.py`](../scripts/smoke_headless_host.py) 的 JSONL v1 smoke
均不是当前七回调 Core / JSONL v2 运行或验收入口。

历史源码依据继续保留，便于追溯裁剪来源：

- [原版 C ABI 与回调](https://github.com/HIllya51/LunaTranslator/blob/c8701872a5de83f8d3272f3967457d0b8f2983c4/src/NativeImpl/LunaHook/LunaHost/LunaHostDll.cpp)
- [Host 连接、管道与线程](https://github.com/HIllya51/LunaTranslator/blob/c8701872a5de83f8d3272f3967457d0b8f2983c4/src/NativeImpl/LunaHook/LunaHost/host.cpp)
- [输出缓冲与回调](https://github.com/HIllya51/LunaTranslator/blob/c8701872a5de83f8d3272f3967457d0b8f2983c4/src/NativeImpl/LunaHook/LunaHost/textthread.cpp)
- [ThreadParam 布局](https://github.com/HIllya51/LunaTranslator/blob/c8701872a5de83f8d3272f3967457d0b8f2983c4/src/NativeImpl/LunaHook/include/types.h)
- [注入器及返回值边界](https://github.com/HIllya51/LunaTranslator/blob/c8701872a5de83f8d3272f3967457d0b8f2983c4/src/NativeImpl/LunaSubprocess/dllinject.cpp)
