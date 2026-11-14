# 新工程验证记录

日期：2026-10-18。本文件只记录本次全新工程的证据，不继承旧 Rust 项目的完成度。

## 最新：サクラノ詩 10th x64 桌面实时取文已通过

Luna Reader 0.1.1 已在独立 `Sakura-no-Uta-10th` bottle 连接 x64 的 `sakuranouta10th.exe`。
原版 LunaHook / LunaHost 保持不变，选用 **Artemis** 流，连续取得 5 条不同对白并与游戏画面对照。
受控样本正常断开，宿主退出码 0；通过新增本地启动器冷启动后，重新连接并再次在客户端看到与游戏一致的原文。
两个候选流的总计 10 个事件并非 10 句不同对白；Artemis64x 在本次第 3 条漏掉「薄紅」，故选用 Artemis。

详见 [专项验证记录](sakura10th-verification.md)、[离线 HTML 报告](../../reports/sakura10th-validation-20261018.html)；
当前包为 [Luna Reader 0.1.1](../../dist/Luna-Reader-0.1.1-macos-arm64.zip)。
此次未验证在线翻译、长时间稳定性或完整游戏兼容性，也不将它视为原卡死问题的修复验收。

## 此前：可打包 Rust 桌面客户端实时显示已通过（SR x86 / 0.1.0）

本轮已构建并实际启动独立的 macOS arm64 release 应用 **Luna Reader.app**，通过窗口选择、连接 `SR.exe`，
推进游戏对白后，客户端“当前原文”、候选流样本、计数与历史随真实回调更新。
这次验收包含可见桌面窗口与打包产物，不再只验证控制台日志。原版 LunaTranslator 客户端未参与。

- 链路：`SR.exe → 原版 LunaHook32.dll → 原版 LunaHost64.dll → Rust lunahook-host → Rust hook-client → Tauri 桌面窗口`。原版 C++ Hook 未修改；前端每150ms读取Rust快照，无模拟台词或回放。
- 环境：CrossOver26.3.0.39832、Steam bottle；实际目标 `SR.exe` 为x86，Windows PID1736，创建FILETIME为 `134367348829636590`。连接前再次查询完整路径、创建时间与架构，未复用旧PID作为凭据。
- 初版界面会话：真实文本事件共118条，选中的完整 `TextOutW` 流收到30条不同原文事件。游戏与客户端截图15/16、23/24分别对照；事件可能为台词分行，不等同于30个完整句子。
- 最终release包复验：另一会话收到8条文本事件，其中同一 `TextOutW` 流2条不同原文。截图33/34中，客户端显示 `直到现在还是持续着。`，与游戏对应行一致。
- 最终包退出：修正macOS默认Quit可能绕过 `ExitRequested` 的路径，使用自定义退出菜单/Cmd+Q、窗口关闭处理与 `Exit` 兜底清理。实际退出后收到 `process_disconnected`，最终快照 `shutdown_acknowledged=true`、`exit_code=0`、`errors=[]`、`integrity_lost=false`。
- 退出后继续操作游戏，截图37显示新文本 `静静地关上抽屉，靠在椅背上，看着窗外的风景并确认着时间。`，证明本次退出后游戏仍能推进。断开回执和游戏可操作不单独证明目标DLL已卸载。
- 最终包第三次独立启动后重新连接同一仍在运行的SR进程，窗口已从 `夜晚即将到来。` 更新为 `可是，外头的风景几乎没有任何改变。`，并保留连接供用户查看。截图49/53为游戏，50/54/59为客户端；`live-preview-log-snapshot/` 是运行中日志快照，没有最终退出快照属正常，不作为额外一次关闭通过。

交付产物：[`dist/Luna Reader.app`](../../dist/Luna%20Reader.app/)；
分发ZIP：[`Luna-Reader-0.1.0-macos-arm64.zip`](../../dist/Luna-Reader-0.1.0-macos-arm64.zip)。
ZIP SHA-256：`0a9666b3da74eea853bdb9222bda391a732472a578d27e85c9eee575ff322428`。
内置Windows宿主SHA-256：`6438761fa81023e387d4be9eb384ed1235d33048c7b6e0086224e97b43c2cdcf`。
固定上游commit：`c8701872a5de83f8d3272f3967457d0b8f2983c4`。
当时产物信息见 [`build-manifest-0.1.0.json`](../../dist/build-manifest-0.1.0.json)。应用尚未完成开发者签名与公证。

离线图文报告：[`luna-desktop-validation-20261018.html`](../../reports/luna-desktop-validation-20261018.html)。
原始证据：[`desktop-live-20261018-r1/`](../../diagnostics/desktop-live-20261018-r1/)；
最终包退出快照：[`final-snapshot.json`](../../diagnostics/desktop-live-20261018-r1/final-quit-session/final-snapshot.json)；
原始协议：[`stdout.jsonl`](../../diagnostics/desktop-live-20261018-r1/final-quit-session/stdout.jsonl)、
[`requests.jsonl`](../../diagnostics/desktop-live-20261018-r1/final-quit-session/requests.jsonl)。
实现入口：[`Tauri主进程`](../../apps/desktop/src-tauri/src/main.rs)、[`Rust连接桥`](../../crates/hook-client/src/lib.rs)、[`窗口界面`](../../apps/desktop/src/App.tsx)。

本轮范围为SR x86的原文实时显示与本次退出路径；尚未接入翻译服务、完整HUD，也未完成长时间稳定性、完整P0或原卡死修复验收。
Never7本轮按用户要求不启动，保留另一台电脑的Steam游戏会话；未将SR结果外推为x64或所有CrossOver游戏通过。

## 基线：控制台 Rust 客户端真实取文已通过（SR x86）

本轮用原版 LunaHook/LunaHost v12.1.2.3 与新 Rust `lunahook-host`，独立连接 `SR.exe`（交响乐之雨）。
当时原版 LunaTranslator 客户端未运行，未修改 C++ Hook，也未引入翻译或图形客户端。

- 环境：CrossOver26.3.0.39832，Steam bottle；Windows目标PID1736、macOS PID740，架构x86。目标路径和创建FILETIME经只读Rust探针查询、宿主再次核验。
- 实际链路：`ready → attach pending → injection_finished(x86, exit=1) → process_connected → hook_inserted → stream_created → text → stream_destroyed → process_disconnected`。
- `TextOutW` 流收到5条不同非空原文，其中4条为完整短句，第5条是较长台词的行分段；已与游戏截图对照，不把5条事件等同于5个完整句子。
- 示例：`库里斯。你那边现在也正下着雨吗？`、`雅琍寄来的信，总是这样开始的。`、`这边依旧是晴朗的好天气。`。
- 原始stdout全部可解析为协议JSONL，真实Output回调已触发；stderr为空；无 `error`、`callback_invalid`、`overflow` 或观察到的完整性丢失。
- 宿主正常shutdown，收到disconnect回调并以退出码0结束；游戏仍运行，断开后继续点击可显示新原文。未据此单独断言目标DLL卸载完成。
- 样本耗时214.118秒，只证明本次最小实时接收路径；不代表30分钟长稳、完整游戏兼容性或原卡死根因已解决。

原始数据与画面：[`diagnostics/headless-live-20261018-r1/sr/`](../../diagnostics/headless-live-20261018-r1/sr/)。
可复现驱动：[`verify_lunahook_live.py`](../../scripts/verify_lunahook_live.py)；
只读Windows身份探针：[`target_probe.rs`](../../crates/lunahook-host/examples/target_probe.rs)。

Never7 x64本轮未接入：Steam显示另一台电脑正在运行《战争雷霆》，继续会断开该会话。
用户选择“保留另一台会话，先验证SR”，因此本轮以SR完成最小接收验证，不启动Never7。这不是新宿主x64取文失败。

下方“无目标启动”是此前基线记录，不能覆盖本节新增的真实取文证据。

## 更早基线：已通过

| 层次 | 结果 | 证据与限制 |
|---|---|---|
| 产品原稿 | 100个文件SHA-256一致；56项需求及22屏全部映射 | `python3 scripts/check_project_plan.py`；[来源清单](../product/source-manifest.json)；17个新工作包 |
| 资源准备 | 官方ZIP及10个文件逐项校验、仅提取窄运行时 | `python3 -m unittest discover -s tests -v`，5项通过；真实资源在忽略目录 `runtime/lunahook-v12.1.2.3` |
| Rust单测 | 9单元测试、1文档测试通过 | `cargo test --workspace`；覆盖契约、64位精度、ABI布局、队列丢失、帧边界、资源哈希与符号链接 |
| 静态检查 | 格式、macOS/Windows clippy、Windows目标检查通过 | `cargo fmt --all --check`；`cargo clippy --workspace --all-targets -- -D warnings`；Windows同命令加 `--target x86_64-pc-windows-gnu`；Windows `cargo check` |
| Windows实际产物 | Windows x64 PE构建完成 | Rust 1.89.0，`x86_64-pc-windows-gnullvm`，LLVM-MinGW 20261006 / Clang23.1.3，静态unwind；尚非签名/发布包 |
| CrossOver无目标启动 | 加载固定Host，ready、ping、status、shutdown通过，退出0 | CrossOver26.3.0.39832 / Steam bottle；目标为空、完整性无丢失；下述原始JSONL |
| macOS直接运行 | 返回exit2及一条 `unsupported_platform` | 不伪造Windows Host初始化、连接或原文 |

本轮两次代码静态审查及一次架构审查发现的问题已修正，包括完整设计映射、会话PID隔离、
控制状态不依赖可丢文本队列、关闭期限、重复参数、原生资源文件锁和链接检查。
这些审查不替代运行验收。

## 更早基线：CrossOver无目标启动证据

产物：`target/x86_64-pc-windows-gnullvm/debug/lunahook-host.exe`。
SHA-256：`ee99a63f05da68544ea6d06a29d5ee57816d9500fcefa5bdd13e0db2ad183a9b`。
原始请求、输出、诊断和结果：[`diagnostics/headless-host-smoke-20261018-r3/`](../../diagnostics/headless-host-smoke-20261018-r3/)。

```jsonl
{"v":1,"event":"ready","host_arch":"x64","upstream_commit":"c8701872a5de83f8d3272f3967457d0b8f2983c4","abi_profile":"lunahost-cdecl-v12.1.2.3"}
{"v":1,"event":"response","id":"smoke-ping","result":{"status":"pong"}}
{"v":1,"event":"response","id":"smoke-status","result":{"status":"snapshot","targets":[],"integrity_lost":false}}
{"v":1,"event":"response","id":"smoke-shutdown","result":{"status":"shutting_down","deadline_ms":2000}}
```

该基线只发送上述三个命令，没有attach或游戏注入；stdout四行均为合法协议JSONL，stderr为空。
当时只证明DLL加载、初始化与命令关闭路径。真实游戏回调的后续结果见本页上方控制台与桌面客户端验证。

## 可复现步骤

原生资源先按 [native说明](../../native/README.md) 提取。macOS交叉构建示例，编译器是独立工具，不使用旧项目源码：

```sh
rustup target add x86_64-pc-windows-gnullvm
PATH="<LLVM-MinGW目录>/bin:$PATH" \
CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER="<LLVM-MinGW目录>/bin/x86_64-w64-mingw32-clang" \
RUSTFLAGS='-C target-feature=+crt-static' \
cargo build --workspace --locked --target x86_64-pc-windows-gnullvm

python3 scripts/smoke_headless_host.py \
  --exe target/x86_64-pc-windows-gnullvm/debug/lunahook-host.exe \
  --runtime-dir runtime/lunahook-v12.1.2.3 \
  --wine '/Users/tanghehui/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine' \
  --bottle Steam \
  --output diagnostics/headless-host-smoke-new-run
```

输出目录须不存在。不要将GCC版 `windows-gnu` 的链接规则直接配给LLVM-MinGW；本次实际产物使用gnullvm。
`+crt-static`避免另需编译器的动态 `libunwind.dll`；上游Host所需VC runtime仍按锁文件提供。
Windows原生MSVC构建和执行另行验收。

## 尚待完成

- H2剩余：Never7 x64真实取文；SR x86最小取文样本已通过，扩大场景仍另行验收。原版成功结果不能代替新宿主证据。
- H3：本次最终桌面包退出已取得正常断开、回执、exit0与游戏继续操作证据；真实目标退出、其他重连/失败恢复、消费端阻塞、队列溢出、30分钟/100句样本及内存曲线仍待验收。
- P1/P2：最小客户端已管理所选bottle中的目标发现和连接组件；完整游戏启动/接入流程、Windows原机执行及原生macOS游戏后端另行验收。
- 已实施最小Tauri原文窗口；在线翻译、持久化、完整应用核心及原设计其余UI尚待实施，完整产品P0未验收。

原版SR/Never7对照记录仍在 `diagnostics/`，属于先前对照。Rust封装没有据此宣称修复原卡死或改进全部游戏兼容性。
