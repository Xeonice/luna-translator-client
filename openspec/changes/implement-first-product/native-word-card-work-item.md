# 原生短词卡临时 surface

- 需求/组件：FP-04、FP-07；C06、C19；M12（语言与阅读 UI）、M13（平台窗口与窄 IPC）。Owner native_wordcard_impl。
- 依据：当前整稿短词卡锚定词语、独立于完整学习窗口；首版连续阅读与本地查词。材质 tokens、批准截图、4 个主 WindowRole 均不变。
- 缺陷：DOM popover 只能在 HUD webview 内显示，受 HUD 可用高度限制。改为独立、无装饰、非主动聚焦的原生附属窗口，可跨 HUD 边界。
- 允许路径：`crates/app-core/src/contracts/word_card.rs` 及 mod.rs 窄导出；`apps/desktop/src-tauri/src/platform/word_card.rs`、`ipc/word_card.rs`，bootstrap/main/platform/ipc 模块窄装配；word-card capability；`src/bridge/{application-port,tauri-port}.ts`；`src/features/language/{WordCard,WordCardSurface}.tsx` 及样式/index；`src/app/bootstrap.tsx`；`src/ui/reset.css`；`tests/support/{design-port,bootstrap}.tsx?`。不覆盖其他 Owner 的 Model 契约/生成文件或 verify-ui。
- 契约：UI 仅传 unitId/textRevision/tokenIndex 与 DOM client rect。Rust 读取不可变阅读单元校验词索引；单独冻结短词卡，不调用 freeze_learning。仅用户主动学习/语境解释才固定学习对象和调用 study。
- 平台：真实调用窗口内容原点、scale、monitor 工作区决定位置；macOS 统一到全局 logical points，Windows 保持物理坐标；上方不足向下，左右/上下 clamp。父窗移动、resize、scale、CloseRequested（既有隐藏路径）、销毁时关闭；新句替换 DOM 锚点保留最初 rect 与冻结对象；系统最小化/隐藏遵循原生 parent 附属关系；新句不替换冻结对象。各代临时 label 与 cardId 绑定，慢查询及迟到关闭不得覆盖新代。
- 权限：临时窗口只可读自身冻结对象、查词、关闭和明确发起学习；不得调用全量 snapshot、配置、注入或历史写命令。不安装键盘监控；Escape 仅在 HUD 或词卡收到键盘时关闭，保留可见关闭按钮。
- 验证：Rust 单元覆盖 1x/2x、负屏坐标、工作区边缘、上翻下、无效 anchor、代际拒绝；实际编译/前端 tsc；受控 browser transport 使用同一内容组件验证业务（不冒充原生跨窗）；正式包验证独立子窗越界、真实词典、游戏不抢焦点、长词条内部滚动、新句冻结、父窗移动关闭与 Escape 限定。正式包尚未执行的保持 not_run。
- 完成条件：窄 E2E 编译、独立复核后由 root 在正式包实机核验；不提交/推送，不提升完整产品验收状态。

## 锁定平台显示路径

本机锁定 Tauri 2.11.6 / tao 0.35.3：macOS `set_visible(true)` 进入 `makeKeyAndOrderFront`，所以隐藏创建后再 show 不能证明不抢焦点。实现改为创建时 `visible(true).focused(false)` 的 `orderFront` 路径，初始位置/大小提前设置，随后按各平台规范空间校正（macOS LogicalPosition/LogicalSize，Windows PhysicalPosition/PhysicalSize），再应用磨砂效果；不调用 show/set_focus。此源码链只证明选用的调用路径，真实游戏焦点与跨屏首帧仍须正式包实测，保持 not_run。

## 评审修正

- Tao macOS `inner_position`、monitor position、work_area 的 physical 值各乘自身屏幕 scale，异 DPI 屏幕的数值矩形会重叠。macOS 将调用窗口原点/大小与各 monitor 的屏幕/工作区按各自 scale 归一为同一全局 logical points，再选屏、clamp，最终只传 LogicalPosition/LogicalSize；Windows 仍用统一 physical 空间。新增真实重叠模型的选屏回归及同 scale/Windows 对照，不把同屏坐标测试冒充跨屏证据。
- 依现行设计 `learning.js` 的冻结词卡语义及既有 UI-07，纠正此前“DOM 锚点消失即关闭”的错误理解：新句替换锚点时保留最初 rect、unit 和选词，直到明确关闭、Owner 组件卸载或父窗移动/缩放/滚动。滚动 Owner 祖先在选择时捕获，不依赖旧节点仍在 DOM。
