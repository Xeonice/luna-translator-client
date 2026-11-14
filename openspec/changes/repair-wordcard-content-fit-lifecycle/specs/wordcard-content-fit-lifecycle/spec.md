## ADDED Requirements

### Requirement: FP-07 自动内容适高保持冻结原生词卡
系统 SHALL 在正文、新句、注音或译文引发的已确认程序性内容适高中，保留既有原生词卡的固定原句、选词、释义和最初锚点，不新增临时或固定二模式。

#### Scenario: automatic-content-fit-preserves-frozen-native-wordcard
- **WHEN** 点词打开原生词卡后，正文换句、ruby 或译文更新造成 HUD 实际内容适高
- **THEN** 原生词卡真实窗口仍存在且可见，card ID、固定 unit/revision/token/senses 与最初 anchor 保持；前端 window.resize 不自行关闭，也不把新当前句替换旧词卡

#### Scenario: exact-generation-and-window-epoch-isolate-late-events
- **WHEN** 自动适高事件出现连续 A/B/C、迟到、合并、重复，或旧词卡/Owner 已被新代替换
- **THEN** 仅确切 operation、card generation 与 window epoch 的事件能获得已确认分类；旧事件不能按 label 关闭新卡，有界状态不得扩大成永久尺寸白名单

### Requirement: FP-04 自动适高具有实际原生完成边界
系统 SHALL 以平台实际 apply/readback 与 epoch/operation CAS 确认自动内容适高，不把排队成功当作 native 几何完成，不在通知栈持锁执行原生调整或销毁。

#### Scenario: bounded-fit-settle-and-failure-reentry
- **WHEN** 适高遇到 no-op/no-event、rounding、应用/读回失败、partial、容量耗尽、重入或换窗
- **THEN** 无变化不遗留许可，内部锁在原生效果前释放；未知或不完整证明保守处理；macOS 复用 AppKit 真实 apply/readback，既有 fit 命令仅内部 async await oneshot 等待实际 settle 才返回真实 projection 或 error，公开名称/参数/DTO 不变，不受理即返回旧投影且不主线程阻塞；非 macOS 未证路径不冒称实测

### Requirement: FP-05 保留真实用户几何和显式关闭
系统 SHALL 保留用户几何变化及既有明确关闭行为，纯失焦不得关闭；用户和未知几何不得因自动适高许可而永久豁免。

#### Scenario: intentional-user-geometry-and-explicit-close-remain-effective
- **WHEN** 用户真实移动/live-resize/scale/reset/width/compact，或 Escape、外点、指定 Owner 滚动、卸载及精确原生 close 到达
- **THEN** 关闭对应代词卡，即使用户 live-resize 恰好命中旧预期尺寸；纯 blur 和词卡内部滚动不关闭，迟到 close 不影响新代，不替换或 swizzle Tao delegate

### Requirement: FP-08 学习对象和产品验收边界保持
系统 SHALL 继续以显式操作打开固定原句的独立学习窗，并以同一最终包真实原生窗口证据验收此修复，不改变模型/词法/存储公共契约。

#### Scenario: native-and-game-evidence-cannot-be-replaced-by-fixtures
- **WHEN** 验收本轮原生词卡修复或从既有词卡打开独立学习
- **THEN** 学习仍固定原对象；最终同包 MO 至少四段不同长度日文及异步结果要同时核实际 bounds/native Resized 和词卡窗口/冻结释义；私有事实出口只在既有 M12 window_manager.rs 内嵌 bounded try_send/off-main JSONL、单份 backup/行限额/loss 计数并与 Host 日志分离，绝无原文/释义/密钥且不能阻塞生命周期；浏览器/内部registry/编译或旧包通过不代替原生实机，未执行保持 not_run，旧视觉失败保留
