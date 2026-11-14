# 实施与验证任务

以下均是计划；源码、受控测试、原生实机和正式发布状态分别报告。

- [ ] 核对 Root 工作卡、R34 原始 source-snapshot、FP-04/05/07/08、真实 #hud-window/#word-popover/#learning-window 与 M12/M13、C06/C19/C34 映射，完成独立架构审阅及静态 OpenSpec 校验。
- [ ] PL 在八个允许生产文件实现 Rust 统一适高分类、精确 card generation/window epoch、释放锁后 native apply/readback/CAS；无变化/失败/容量/未知路径保守且有界。
- [ ] PL 复用 AppKit bridge 真实 live-resize 起止和主线程实际 apply/readback；不替换 Tao delegate，不持锁 destroy/set_size，非 macOS 未证路径不宣通过。
- [ ] PL 在 ipc/reading.rs 仅将既有 fit 命令内部改为 async oneshot 等待原生 apply/readback/settle；公开名称/参数/DTO 不变，不主线程 blocking，不返回受理时旧投影。
- [ ] PL 将私有 bounded try_send/off-main JSONL 内嵌 window_manager.rs，单份 backup/行限额/loss 计数，仅真实 opaque geometry/card 事实，与 Host 日志分离，验证饱和/写入失败不阻塞。
- [ ] PL 移除 WordCard.tsx 重复 window.resize 关闭，保留显式关闭/Escape/外点/Owner 滚动/卸载/迟到隔离；不改冻结内容或词典/模型。
- [ ] PL 在既有模块补 A/B/C迟到/合并/重复、卡/窗换代、no-op/no-event、失败/partial/rounding、容量、用户几何命中旧尺寸及重入测试并保留原断言。
- [ ] QA 在三条已授权维护路径登记真实 WordCard/原生close transport 场景，保留原60场景和断言，核 ruby/translation 到来、内部滚动/blur、明确关闭与代际隔离；需要新路径时先报 Root。
- [ ] 执行实际 TypeScript/Rust 编译、原生模块及组件 producer、守卫反例；独立审阅生产实现与证据，不能用守卫自测替代产品验收。
- [ ] 最终源码冻结后才准备精确 authority 候选：R34字节归档、新 review/baseline、受影响 status/evidence 与既有派生导出；Root 独审采用，不更改已批准图片/token/审批/历史结果或存储指针。
- [ ] 构建同源同资源同签名最终包，记录真实 IPC→native apply/bounds/Resized→实际词卡 WebviewWindow 的可见性/ID/冻结内容及关闭原因。
- [ ] 同包 MO 至少四段不同长度日文及异步 ruby/译文验证词卡保留；实际用户 resize/move/可用 scale/width/compact/reset 另测关闭行为，并确认游戏正常操作。
- [ ] 保存旧 candidate12 失败、R34 canonical视觉漂移与所有未执行/阻塞项；按适用范围继续四游戏回归，不测 Ever17，不提交/推送，不宣称完整发布通过。
