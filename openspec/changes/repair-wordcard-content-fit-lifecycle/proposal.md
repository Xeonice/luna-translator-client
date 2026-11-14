# 修复原生词卡在正文自动适高时意外关闭

## Why

candidate12 实机已分离焦点复现：打开 MO 的言葉词卡，仅切回游戏不推进仍保留；不触碰 HUD 推进一句后词卡消失。新正文与译文正常，截图中 HUD 高度变化。源码存在 WordCard 的 window.resize 关闭及原生 owner Resized 无差别关闭两条路径；尚未记录实际 OS 事件和关闭原因，不能断言哪一条先触发。

现行 FP-04/05/07/08、当前整稿的点词固定原句、技术设计与既有 native-word-card 工作项已要求新句不能替换冻结词卡。本变更恢复既有语义，不新增临时/固定模式。

## What Changes

- Rust 平台层统一识别本应用自动内容适高与用户/未知几何变化；以确切 card generation、window epoch 和 operation 身份保护词卡，保持原句、选词、释义及最初锚点。
- 自动适高采用有界 reserve → 释放锁 → 原生实际 apply/readback → CAS settle；不能把 Tauri set_size 排队成功当作原生调整完成。macOS 复用现有 AppKit bridge 在主线程实际应用并回读，同时观察真实 live-resize 边界；不替换或 swizzle Tao delegate。
- application_fit_hud_content 保持公开名称/参数/返回 DTO，只在内部 async await oneshot，等待 RunLoop 实际 apply/readback/settle 后才返回真实 projection 或 error；不得受理即返回旧投影，也不得主线程阻塞。window_manager.rs 内嵌私有有界异步几何/词卡 JSONL，try_send、off-main writer、单份 backup、行限额与 loss 计数；仅 opaque 几何/卡身份，不记录原文/释义/密钥，独立于 Host 日志。
- UI 移除 window.resize 的重复关闭权威；保留 Escape、外点、指定 Owner 滚动、卸载与迟到 open/closed 隔离。用户移动/缩放、scale、重置、宽度及紧凑布局等明确几何变化仍关闭对应代词卡；纯失焦不关闭。
- 复用现有原生测试与真实 React 组件 producer，补失败、迟到、合并、重复、容量、无变化与锁重入边界；最终包验证实际原生词卡窗口而非仅注册记录。

## Scope and Impact

唯一生产允许范围为工作卡的八个既有文件（原七源加 apps/desktop/src-tauri/src/ipc/reading.rs）；QA 只修改既有 verify-reviewed-reading.mjs、reading_components.py 与 test_reading_components.py。不改公共 DTO、数据库、Core、模型、语言资源、依赖、材质 token 或已批准图片。design-port.ts / verify-ui.mjs 没有预先授权；若维护测试确需扩展，先给出具体义务及精确路径由 Root 决定。

主责为 M12 desktop-platform、M13 product-ui，复用 C19 WordCard/DictionaryEntryList、C06 Popover 与 C34 原生多窗；C16/C18 是阅读内容和用户几何 intent 的既有输入，不改其实现。M03 阅读对象、M07/M08 语言/词典保持输入权威；M04 是 model-config，不属于此修复。

当前设计真实锚点为 index.html#hud-window、#word-popover、#learning-window；不存在 #reading-overlay。这是不改变外观的生命周期修复，不能据此重拍批准图。

## Verification Boundary

编码起点为 Root 从 R34 已采用完整源码 map 复制的 source-snapshot.json，SHA192517f64dcf229526d651a6b3f39907538a00f477ced7deb4dc6dfb7c91a19e，保持原字节。R34 fullverify 的 63 项中 62 通过、canonical UI reference_source_drift 一项失败保持原事实；本变更的计划、编译、单元、受控组件、原生和游戏证据独立记录，不继承 candidate12 的局部结果。

最终需要同源同资源同签名包，在 MO 至少四段不同长度日文及异步 ruby/译文中核对真实 native Resized、实际 word-card WebviewWindow 存在/可见、card ID 与冻结释义，再验证真实用户调整关闭行为。未执行为 not_run。跨平台未实测及外部程序并发同几何缺少 origin 的可观察限制须如实说明。R35 指纹/实现状态/导出更新只在最终源码和独立审阅后提出精确候选，Root 决定采用；不提前刷新 R34 或提升上线状态。
