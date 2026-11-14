# 原生词卡自动适高生命周期设计

## 依据与目标

恢复 FP-07 点词固定对象及 FP-04 连续阅读；保留 FP-05 的显式控制和 FP-08 固定学习对象边界。依据当前 first-product-scope.md、ai-reading-prd.md、reading-window-session-technical-design.md 的新句保留词卡说明，以及 implement-first-product/native-word-card-work-item.md。独立架构审阅为 .build/wordcard-resize-diagnosis/architecture-review.json，SHA1c145426059f160af3c77d9835662a1ec1322481adb6ab0df92795ba3835077a。

真实设计状态：index.html#hud-window 持续到句/译文/ruby；#word-popover 固定本地词卡；#learning-window 是显式打开的独立学习，不随当前句替换。M12/C34 管几何与原生窗；M13/C19/C06 管现有内容呈现和关闭意图。不得将 M04 模型配置当成阅读模块。

## 平台权威与生命周期

WindowCoordinator 只处理平台几何，不导入 ReadingUnit 或复刻词法业务。WordCards 继续持有已有冻结对象。生命周期决定必须携带确切 card ID/generation、Owner 原生窗口实例 epoch 及用户 geometry generation，迟到旧事件不得按 label 销毁新窗或新卡。

内容适高预留的是确切一次 operation，而非 bool/超时许可。无变化不遗留许可；reserve 后释放内部锁，再执行原生调整并读取实际 bounds/scale，按 epoch/operation CAS 提交。Tauri set_size 的排队完成不等于 native apply 完成，macOS 使用既有 AppKit bridge 的主线程真实 apply/readback 来 settle。失败、部分应用、换窗、未知事件与耗尽有界容量都走明确保守分支，不静默遗留宽泛许可。

A/B/C 多次适高的迟到、合并、重复和乱序事件用有界顺序状态处理；不使用永久 expected-size 白名单、单个短 bool 或仅等待时间忽略。AppKit 真实 live-resize 开始/结束更新用户 geometry generation，保护用户拖动命中旧尺寸的情况；不替换或 swizzle Tao delegate。不在通知栈持锁调用 destroy/set_size，关闭操作先提取精确对象、释放锁，再执行原生效果。

已知程序性适高保留当时冻结卡及原始 anchor；用户移动、真实 live-resize、scale、重置位置、宽度/紧凑布局等明确用户操作仍按现有策略关闭对应代。未知 origin 保守关闭。非 macOS 没有实际 apply/readback 证明时保守处理，不提升跨平台实测。外部程序恰与受控尺寸重叠但 OS 不提供 origin 的限制必须列入实机结论，不能声称绝对可区分。

## 前端职责

WordCard.tsx 去除 window.resize 独立决定关闭的监听。Rust 是 owner 几何分类权威；UI 保留显式关闭、Escape、外点、指定 Owner 祖先滚动、Owner 卸载、迟到 open/closed 的代际隔离。纯失焦不关闭；词卡内部滚动不冒充 Owner 滚动。新句替换 DOM token、ruby 或译文更新都不能替换已有 frozen unit/revision/token/senses 或最初 anchor。显式打开学习仍沿原有路径，不新增临时/pin 模式。

## 范围、复用及授权边界

八生产文件与三 QA 既有文件的精确清单以 verification.json 为准。纯 Rust 策略测试嵌入原模块；受控 React 测试沿现有 producer 登记，旧 60 场景及既有断言保留。无公共 DTO/命令、DB/DDL、Core/ABI、语言 worker/资源、模型服务、框架、依赖或 approved 视觉变化。测试若不能在已授权三条 QA 路径表达，先报告具体路径及执行义务，不为过测试扩展架构。

后续 authority 仅允许 Root 独审后产生 R34 原字节归档、本 change review/新 revision、implementation-status.json 与其三个既有派生导出的真实 source/quote 同步。不得改历史审批、current artifact/runtime、批准图片/token、阈值或 legacy-debt；现在不生成 R35 哈希。

## 原生完成回执与私有有界事实出口

Root 已精确授权新增第八生产路径 apps/desktop/src-tauri/src/ipc/reading.rs。application_fit_hud_content 的公开名称、参数及返回 DTO 均不变；内部 async await oneshot，等待 RunLoop 实际 apply/readback/settle 完成后才 resolve 真实 projection 或 error。不得在受理排队时返回旧投影，不得主线程 blocking，不新增公共命令或模型/业务契约。

M12 私有异步 JSONL 内嵌既有 window_manager.rs，不增加目录或 module。出口通过有界 try_send 和 off-main writer 写入 app_log_dir，单份 backup、行限额与 loss 计数，记录真实 geometry/card 的 opaque ID、epoch、分类和关闭原因；绝无原文、释义或密钥，与 Host/Core 日志分离。队列满或写入错误不能阻塞主线程或伪造无丢失，关闭不能持 coordinator 锁等待 writer。仅日志可用不证明产品生命周期正确，原生窗口与最终游戏仍须独立核验。

## 验证与证据分层

1. 纯策略：连续适高 A/B/C、合并/重复/乱序/迟到、卡换代/窗换代、no-op/no-event、读回/应用失败/部分应用/rounding、容量、用户 live-resize 命中旧尺寸、move/scale/width/compact/reset、锁释放和 reentry；未知事件应关闭，不能为了保卡放行。
2. 真正 React WordCard：旧句→不同长度新句、ruby/translation 更新保持冻结卡；dispatch window.resize 不再自行关闭；原生精确 close、Escape/外点/Owner 滚动/卸载仍生效；内滚动/纯 blur 不关闭；旧 open/close 不影响新代。
3. 原生最终包：经真实 IPC 触发内容适高，实际 bounds 变化和 native Resized 必须可观察，并同时核真实 WebviewWindow 存在/可见、ID、冻结释义和 close reason。只检查 WordCards registry、静态截图、DesignPort 或编译不满足此项。
4. MO 同一最终包至少四段不同长度日文及异步 ruby/译文，再实际鼠标调整、拖动和可用 scale/display 操作；正文输入、焦点与释义冻结分别核对。未支持/未执行项保持 not_run 或 blocked，不外推 Never7/Sakura/Windows。

证据绑定当前源码/资源/签名包、window epoch、operation sequence、user generation、实际物理 bounds/scale、card generation/classification/close cause及截图；不记录秘密。registered product-native-ipc-tests 和 product-reading-component-tests 分别是代码/受控组件 producer，不冒充已有原生游戏 adapter。现 product-ui-capture-v4 的漂移失败继续保留，不能覆盖图或放宽阈值。release 所需真实 runtime adapter 不存在时仍阻断发布。
