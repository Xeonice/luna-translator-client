# ADR-002：独立维护仅取文的 Luna Hooks Core 源码

日期：2026-10-18。用户要求从自己的 Fork 复制独立仓库，在源码层物理移除旧客户端和无关功能。

新工程位于 [`luna-hooks-core`](../../../luna-hooks-core/README.md)，来源为
`Xeonice/luna-translator` 的 `5a11c55e2cc8c0095a77f9f2145eb01bd6cfc410`。
原始文件哈希、删除与修改清单均随新工程保留；已独立提交并推送至私有仓库
[Xeonice/luna-hooks-core](https://github.com/Xeonice/luna-hooks-core)，基线提交为
`e9242ebac32a09e911f306b7c90714891c217302`。客户端重建见[ADR-003](ADR-003-client-rebuild-on-core.md)。

新工程仅保留 Hook、无界面 Host、独立注入器及其必要源码依赖。除移除原 Python/GUI、翻译、
OCR、TTS、更新等目录，也删除 Hook 内部译文等待、回写、字体替换和相应协议字段。
保留全部原引擎源码集合和原文解码、分句、搜索；V8 本机 HTTP 原文桥及游戏取文脚本属于取文能力。

Core ABI 1 使用七回调 `Luna_Start`，Create 回调不含内嵌标记，且 DLL 名和通信标识独立。
**它与当前十回调 FFI 不兼容，不能直接替换现有打包运行时。** 当前 Luna Reader 0.1.1
仍使用原版固定二进制；后续需要单独增加 Core ABI 绑定、资源校验及新注入器 CLI 适配。

新源码的 x64 Hook/Host/注入器已构建，通过受控 GDI 检查，并在 CrossOver 的
サクラノ詩 10th 中实际取得五条日文，与游戏截图一致。
x86 完整 Core 已通过 macOS Clang 23 + Microsoft ABI／CRT／SDK 交叉构建，
全部 225 个 engine32 源文件和异常保护保留；GNU i686 的 SEH 后端限制仍存在，
没有通过删除引擎或异常处理来规避。受控 x86 GDI 三条日文通过后，进一步在 SR 中文版
取得 TextOutW 的八条分行回调，对应五屏对白；正常断开后游戏仍能推进。
这些结果来自独立原生回调探针，不代表当前 Rust GUI 已迁入新 Core。
MO 也已用新 Core32 完成 5 屏日文字幕对照和正常断开；首片段存在相邻页合并，先前图形与内存异常仍未归因。Never7 现已用新 Core64 完成 5 条新增日文对照（5 个变化状态、3 个文本页），并在正常定时断开后继续推进。Windows 原机 cl.exe、重复重连和长期稳定性仍按各自实际证据验收。

完整证据与限制见新工程的 [验证记录](../../../luna-hooks-core/docs/verification.md)
和 [脱敏机器可读摘要](../../../luna-hooks-core/verification/published-summary.json)。
