# 原生词卡内容适高生命周期修复工作卡

2026-10-22；编码前计划，基于已授权首版开发与持续实机回归，不新增产品模式。当前基线FP1-20261022-R34，R34源2e50943697d5390da3f45d135cc005c29ccabd50ddc0d976164d38a0367345c3；基线SHA53ad264157d9b2fbd53478a848505f8add9cadbbac6c904bfbfdab98a4a6941b。

## 已复现问题与要求

- 当前实际包为candidate12，源77ea4800...，Luna42741、MO98642/Win3100、SID1792611349484-42741-1。用户分离验证：言葉词卡打开→只切回MO不推进仍保留→不动HUD推进一句后词卡消失。CUA真实HUD新句/译文正常，截图视觉高度417→345；尚未记录OS Resized/关闭原因，不伪称唯一先触发路径。
- FP-04/05/07/08；现行整稿 learning.js 点词即固定原句，不存在临时/pin二模式；architecture reading-window-session-technical-design.md:319、新句保留既有词卡；implement-first-product/native-word-card-work-item.md:7～20明确新句替换DOM锚点不能关闭。
- 设计页面：当前first-product-v1/index.html#hud-window、#word-popover、#learning-window、#main-content；M12 desktop-platform / M13 product-ui，复用C19 WordCard/DictionaryEntryList与C06 Popover；不修改M04 model-config。
- 两条独立路径：WordCard.tsx window.resize主动close；native owner Resized→owner_changed→destroy。正文/注音/译文自动适高可触发两者。
- 参考独立架构审阅 .build/wordcard-resize-diagnosis/architecture-review.json（SHA1c145426059f160af3c77d9835662a1ec1322481adb6ab0df92795ba3835077a）。

## 范围、Owner与复用

- Root统筹和唯一GUI/安装；PL worker拥有8个生产文件：platform/window_manager.rs、platform/word_card.rs、ipc/word_card.rs、main.rs、platform/macos/mod.rs、platform/macos/bridge.m、ipc/reading.rs、features/language/WordCard.tsx（均以apps/desktop/src-tauri/src或apps/desktop/src为现有根）。统一处理前端重复关闭与原生分类，不并行拆同一生命周期。
- QA拥有verify-reviewed-reading.mjs、scripts/quality/reading_components.py、test_reading_components.py；如必须扩既有testsupport/design-port.ts或verify-ui.mjs，要先明确可执行义务，不覆写原50/60断言。PL纯Rust tests嵌现有生产模块，新增测试不拿fixture充实机。
- Governance只拥有本change及经Root逐项审阅的研发基线、实现状态导出和精确review；产品代码不由其修改。所有协作者不覆盖或回退他人改动。

## 技术策略及完成边界

- Rust统一几何原因分类，WindowCoordinator不导入ReadingUnit；WordCards保留冻结对象，失效只操作精确cardId/generation/window实例，不能晚到按label清新卡。
- reserve exact fit→释放锁调用原生set_size→实际bounds/scale读回→epoch/operation CAS确认；无变化不留许可，A/B/C迟到/合并/重复用有界顺序状态，未知/失败/部分应用/窗口换代保守清理。不能短bool/超时或永久尺寸白名单。
- 用既有AppKit bridge监听确切NSWindow的live-resize起止，记录用户geometry generation；不替换/swizzle Tao delegate，不在通知栈持锁调用destroy/set_size。用户拖动/resize/scale/reset/width/compact仍使当时词卡失效；仅失焦不关闭。
- 前端只移除重复resize权威，保留Esc/外点/指定滚动/卸载/迟到open防护。不增公开业务DTO/命令/数据库字段/框架，不改Core/语言资源/模型服务。
- 真实native测试须记录实际高度变化、native Resized、原生词卡可见/ID/冻结释义、关闭原因，不能只证明registry对象存在。最终同包MO至少4段不同长度文本和异步ruby/译文到来，词卡仍保留原句。
- QA同时覆盖真正用户鼠标resize/拖动关闭、未知/迟到/新卡/失败/no-op/no-event/容量/重入，以及前端不自行resize关卡。跨平台未实机不提升，外部程序并发同几何缺origin的可观察限制如实记录。
- 同签名新包、当前source与资源identity、实际编译/工程门禁和游戏分层证据缺一不得宣称全部通过。R34已实跑63检查62过，canonical视觉reference_source_drift仍失败；不得刷新批准图或放宽阈值。
- 原生operation自然断开active持久化P2另记，不混为当前词卡修复；Never7/Sakura仍待实机。无Git提交/推送。

## 编码前最终内部实现补充

- 第8生产路径为 `apps/desktop/src-tauri/src/ipc/reading.rs`，仅适高命令内部改async，公开名称/参数/DTO保持；用私有oneshot等待RunLoop真实apply、实际bounds读回和CAS settle后才返回projection或error，不能受理即回旧投影，不得main线程阻塞。拒绝投递/实例失效/无Resized时均完成明确终态。
- Root批准M12现有window_manager内私有有界异步trace helper（保持逻辑内聚；实际实现若过大再独审抽模块），原生回调只try_send typed事实，writer不在UI线程写盘；app_log_dir下current+一备份、行长/队列/总容量有界，loss/写失败可见。记录真实op/epoch/usergen/物理bounds/opaque cardId+gen/分类/关闭原因；不记录原文/释义/密钥，不伪装Core Host日志，无新publicIPC/DB。该出口为排查平台生命周期，不把缺日志视作通过。

## 2026-10-23 快照格式修复（未刷新来源）

原 R34 bare-map 快照 SHA192517f64dcf229526d651a6b3f39907538a00f477ced7deb4dc6dfb7c91a19e 原字节保留到 `source-snapshot.original-r34.json`。既有无Git source-scope要求 `files` 包装，Root将执行入口补为wrapper，其 `files` 与原map完整相等，未替换任何来源条目为当前值、未改gate_core或放宽范围。格式缺口被报告后先修正，R35据实登记两个文件；不视作产品通过。

## 2026-10-23 同路径编译检查修复

实际 clippy -D warnings 报出既有 ipc/reading.rs 的非绑定 JoinHandle。Root核对 `stop_preview_consumers` 同步执行且内部已spawn，并非未执行停止；只将隐式丢弃改为显式 `drop`，保持原detach行为，不新增异步调度或扩展预览产品行为。同一已授权 macos/mod.rs 两处既有 unsafe 只补真实安全前提注释；新词卡大Error和defer释放注释按实际错误修正。保留首轮失败，不禁用lint或伪称预览运行缺陷已修复。
