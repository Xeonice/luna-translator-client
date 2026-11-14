# 当前客户端入口与历史原文切片

## 2026-10-31 当前工程与最新目标

当前工作区依据冻结 `FP1-20261030-R74` / 2087源核对；统一任务与准出入口为[当前客户端对齐总计划](current-client-alignment-plan.md)，技术入口为[研发基线](../architecture/first-product-engineering-baseline.md)与[阅读/会话迁移合同](../architecture/reading-window-session-technical-design.md)。最新 [guided候选](../design/first-product-v1/candidates/guided-service-reading-20261030/index.html?scenario=reading)仍 `pending_review`；本轮只更新文档、PRD、测试和准出规划，不改源码、运行包或发布状态。

当前已有独立新Core/broker、Rust app-core/app-infra、真实SQLite、系统Vault、语言worker、主窗/HUD/设置/学习、真实平台预览与会话全集端口。下文0.1.3所述“新Core尚未接GUI”“只有原文”“最近100条”属于当时切片，不能描述当前客户端或限制当前数据保留。

| 实际源码入口 | 已有行为 | guided目标尚缺 / 产品状态 |
|---|---|---|
| `native/core-runtime.lock.json`；`crates/lunahook-host/src/abi.rs` | 独立Core锁、ABI1七回调与Windows broker；不运行原版Python/Qt整客户端 | 新文档不能重标历史工件；具体包/资源/签名/环境身份仍以本次实际manifest与执行记录为准 |
| `apps/desktop/src/features/reading/ReadingPanel.tsx::ReadingPanel:21`、`SessionOverview:166` | 当前会话/概览及三数据集Tab；`SessionRecordPanel:45`已有分页/搜索/导出 | 独立首屏当前卡、三个默认折叠且互不重置的记录区尚未实现，新呈现 `not_run` |
| `crates/app-infra/src/storage/queries/session_records.rs::page:281`、`prepare_export:444` | 固定范围/水位与真实Repository分页、全会话/筛选导出；展示50条不决定全集 | 新布局下超过多页、重开/磁盘失败/保留与同包游戏仍须真实验收 |
| `apps/desktop/src/features/model-config/ModelConfigScreen.tsx::ModelConfigScreen:22`；`crates/app-infra/src/providers/mod.rs::execute:124` | 六实体Tab、草稿/测试/应用、独立用途；3种集中式协议 | 25项必要字段目录、注册/typed凭据、共享浅层选择及细粒度激活尚未实现；13协议/21预设/4离线是目标 |
| `apps/desktop/src-tauri/src/platform/session_preview_coordinator.rs::SessionPreviewCoordinator:27` | App监督预览、候选/会话消费者与只读投影 | 折叠/切页不重建、权限/休眠/旧帧恢复与最终包持续帧须按新场景独立执行，不由旧局部截图继承 |

当前卡/记录容器可先复用现有DTO与共享Select；typed认证、离线资源和细粒度绑定涉及契约/codec/持久化时先另审迁移，不改旧seal。Rust业务权威、窄Tauri、独立Core/collector/语言worker及拟定翻译worker边界继续有效。现有真实存储、组件和有限原生adapter保留，可复用但不得外推新范围已通过；缺真实adapter的新增层、服务账号/资源/平台条件和同一最终包四游戏须分别登记。新产品未执行记 `not_run`，实跑时前置不足记 `blocked`，原失败保留。

## 历史切片：2026-10-18 Reader 0.1.3与0.1.2

以下所有版本、命令、产物、架构图、100条缓存及“当前范围”均限定于各自历史切片。保留原实机/截图/IPC替身证据，不将它们转写为R74或最新设计的产品通过。

> 以下为2026-10-18 Reader 0.1.3历史记录：确认窗口关联后自动启动Hook接入。仍使用原版 LunaHost，新 Core 尚未接入 Rust GUI；重建任务及后续产品回归见[新计划](plan.md)与[回归计划](client-regression.md)。0.1.2 的独立关联证据单列历史章节，不用于证明此次接入流程通过。

这是正式 release 构建的原文阅读切片。用户验收以实际 `.app` 窗口随 SR 台词更新为准；
先前控制台输出仅证明底层回调链路，不再称为可视化客户端验收。

## 运行和构建

macOS Apple Silicon 当前产物为 `dist/Luna Reader.app`，压缩包为 `dist/Luna-Reader-0.1.3-macos-arm64.zip`。实际构建和验证结果见下方[0.1.3 状态](#关联并接入版-013)。
本轮是本机可运行应用包，尚未做 Developer ID 签名或公证。CrossOver 和游戏由用户现有安装提供。

1. 启动 SR；客户端的“兼容环境”可调整 CrossOver 的 wine 路径和 bottle。
2. 点“选择窗口…”或“点击游戏窗口…”，核对SR的窗口、Bottle与程序身份后确认“关联并接入”。客户端重新查询身份并自动启动Hook接入，无需另点进程连接。
3. 推进对白，选择有正文的具体文本流。界面显示当前原文、最近 100 条历史和更新时间；不会达到样本数自动结束。
4. 使用“断开”或退出应用。只请求关闭本次宿主；不会结束游戏或整个 bottle。

在仓库中运行 `python3 scripts/build_desktop.py` 可重复构建。要求 Rust 1.89、Node/npm、
macOS Xcode 构建工具、已准备的固定原生运行时与 LLVM-MinGW 交叉工具链。
`--skip-windows-build` 复用已构建 release 宿主；`--no-install` 复用 npm 依赖。

## 代码边界

```text
SR → 原版 LunaHook → 原版 LunaHost → Windows Rust lunahook-host
  → 私有 stdout JSONL → macOS Rust hook-client → Tauri 命令快照 → React 窗口
```

- `crates/hook-client`：后台枚举、身份复核、宿主管理、完整流标识、100 条每流历史和会话日志。
- `apps/desktop/src-tauri`：资源按 `.app/Contents/Resources` 解析、命令桥和退出清理；不依赖仓库 cwd。
- `apps/desktop/src`：150ms 顺序轮询快照、目标/流选择、原文与历史显示。
- `scripts/build_desktop.py`：交叉构建宿主，校验并打包原生资源，构建 macOS release `.app` 与 ZIP。

保留原版 C++ DLL，不加载 Python/Qt；前端不构造示例台词。流按 PID、地址和两个上下文完整区分。
查询和连接通过后台任务执行；stdout/stderr 持续独立读取；退出等待会话和输出收尾。
每次重连创建新的宿主与日志目录，不复用旧宿主尝试过的 PID。

窗口保留原设计 SCR-04/05 的连接、候选与预览关系，0.1.2 改用已确认的暗蓝灰、中性线性图标；macOS 窗体启用 HUD 原生材质与透明 WebView，内层采用较厚的灰色表面。
当前范围是连接与原文展示；翻译、HUD、数据库、全部游戏枚举及跨平台发布仍按原计划独立实现。
0.1.1 通用枚举所选 bottle 内可核验身份的普通 Windows 进程，过滤系统进程和自身助手；
不依赖游戏文件名白名单，不自动启动任一游戏或接管 Steam 会话。

实际游戏验证见[最新验证记录](verification.md)。完整 56 项产品需求不因本切片完成而视为通过。

## 关联并接入版 0.1.3

用户操作统一为：选择窗口或点选游戏窗口 → 核对 → “关联并接入” → 自动接入 → 收到候选 → 选择正文。选择与预览不发起注入，最终确认才触发；“正在接入”“已连接，等待文本”“已有候选”分别呈现，不把窗口关联完成当成取文成功。

- 无活动会话：确认后将已核验窗口的目标交给现有Hook连接流程。
- 同一目标已有活动会话：复用会话、已选正文和原文历史，不重复注入。
- 不同目标已有活动会话：在核对面板明确说明将切换取文目标，确认后再替换；取消保留旧窗口与会话。
- Hook接入失败：保留仍有效的窗口关联，显示失败阶段并提供“重试接入”；重试前重新核验目标身份。
- 解除窗口关联：仅解除窗口关系，继续保留取文；停止取文使用“断开”。

同目标判断包括兼容环境、Bottle、完整映像路径和本次进程身份。窗口核验、Hook宿主状态、回调到达与正文选择分别记录；关联失效不得冒充Hook断开，Hook失败也不得冒充已收到正文。

### 本轮验证状态

Reader 0.1.3 已构建为 `dist/Luna Reader.app` 与 `dist/Luna-Reader-0.1.3-macos-arm64.zip`，资源签名和压缩包内容校验通过，保留31份许可。后端21项测试及相关Clippy通过；[前端8组契约检查与7张标记截图](../../diagnostics/bind-inject-20261018/frontend-contract/qa.json)通过，该组使用IPC替身，不是原生运行证据。

[真实运行记录](../../diagnostics/bind-inject-20261018/runtime-verification.json)来自CrossOver 26.3 的SR x86：宿主报告进程连接，并收到至少4条TextOutW对白；[客户端现场](../../diagnostics/bind-inject-20261018/12-client-ready.png)显示实际原文与候选。重新关联同目标、解除后再关联仍使用同一宿主会话，日志始终只有1条attach请求；不同目标的切换提示及取消已实测，取消保留SR会话。

首次自动化确认开始前，客户端已由现场操作进入连续取文，本轮保留该会话完成复用/解绑回归，未为自动化另行断开并做冷首连。统一确认只调用新接入命令的路径由契约检查与代码审查覆盖。实际切换到另一游戏、故意制造注入失败再重试仍待实机验证。WOW64模块快照未显示Hook DLL，未将该快照当作成功凭据；本轮依据真实连接回调及原文事件。完整GL20/MC22/111用例不因本切片而标为通过。

现有高保真HTML、49张截图及离线设计ZIP仍保留此前独立关联交互；本轮原生客户端功能先行，设计交互尚未同步和重新截图。

## 历史窗口关联测试版 0.1.2

以下仅记录0.1.2当时交付的 macOS Apple Silicon / CrossOver 独立窗口关联切片，已由0.1.3的新确认流程替代。该历史版本不依赖网页设计样本，不根据 macOS PID 或窗口标题猜测 Windows 目标，也不将绑定视作 HOOK 已连接。

1. 打开 `dist/Luna Reader.app`，在兼容环境设置核对 Wine 路径和 Bottle。
2. 点“选择窗口…”，从真实列表选游戏，在右侧核对容器、EXE、架构、Windows PID 和完整路径后确认。
3. 也可点“点击游戏窗口…”。客户端临时隐藏，15 秒内点击目标游戏窗口；识别后恢复客户端，仍需明确确认。Esc 取消或超时也会恢复。
4. 主界面显示“已关联游戏窗口”，可以替换或解除。原文采集仍从“刷新进程 → 检查并连接 → 选择正文流”单独开始。

关联仅属于本次运行。确认前重新查询 HWND、窗口类、PID、进程创建时间、完整映像路径和架构；Bottle 和 Wine 设置参与隔离。独立状态查询每三秒内复用核验结果，窗口关闭或身份变化时解除关联。查询在后台执行，不阻塞原文快照轮询；绑定、替换和解除均不调用取文连接或断开。

后端在 `hook-client/src/window_binding.rs`；Tauri 命令为 `discover_windows`、`pick_window`、`bind_window`、`unbind_window`、`window_binding_status`。只读 helper 新增 `--windows`、`--pick-window 15`，原进程发现参数仍可用。实现采用 [EnumWindows](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumwindows)、[WindowFromPoint](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-windowfrompoint) 和 [GetWindowThreadProcessId](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid)，在所选 Bottle 内直接得到 Windows 身份。

### 0.1.2 历史证据

- [实机验证记录](../../diagnostics/window-binding-20261018/runtime-verification.json)：CrossOver 26.3、SR.exe x86；列表和实际点击返回同一窗口身份。正式 `.app` 内的列表选择、确认、解除、点选后隐藏/恢复、取消保留旧关联均已观察。
- [正式包关联截图](../../diagnostics/window-binding-20261018/10-client-ready.png)、[真实候选列表](../../diagnostics/window-binding-20261018/03-window-list.png)、[点击后的核对面板](../../diagnostics/window-binding-20261018/06-point-pick-result.png)。关联时取文保持未连接、0 条原文，没有自动注入。
- 后端 11 项单测、相关 clippy 和 Windows helper 交叉构建通过。前端另有 [8 组 IPC 契约检查](../../diagnostics/window-binding-20261018/frontend-contract/qa.json)与保留的历史截图；首屏图缺失状态见该组记录。该组使用测试替身，不能当作实机证据。
- 打包脚本在复制 `.app` 后执行本机 ad-hoc 签名并验证资源封装，再生成 ZIP；原项目及第三方许可继续随包保留。该签名不是 Developer ID 公证。

0.1.2当时没有用活动 HOOK 会话检验关联期间的连续取文，也没有关闭真实游戏验证失效分支；这些分支的隔离/身份逻辑有单元或前端检查。完整 GL20 / MC22 / 111 条产品用例状态不变。原生 macOS 游戏取字、新 Core 接入、窗口跟随 HUD 均不由此切片宣称完成。当前窗口选择 UI 要求 Wine 路径，尚未交付原生 Windows 前端能力判断。
