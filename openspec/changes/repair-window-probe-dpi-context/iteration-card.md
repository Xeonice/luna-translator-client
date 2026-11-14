# 迭代卡：DPI查询口径修复（编码前）

日期2026-10-26；FP-03/FP-12，目标恢复现行设计窗口选择与常驻预览入口。设计docs/design/first-product-v1/ansi-preview-recovery.html#native-pairing，不修改设计/token/批准图。

M02/HK Owner：仅crates/lunahook-host/examples/target_probe.rs（含同文件测试）。Root Owner：本OpenSpec、架构源码登记与R46精确评审材料、构建资源、包与GUI。Owner不是唯一协作者，不得覆盖他人修改。若确需其他源先报告，不扩大授权。

允许生成资源仅现有prepare_core_runtime维护入口写brokers/{x86,x64}/{target_probe.exe,lunahook-host.exe}及core-staging-receipt.json，Core锁/六Core PE/许可字节保持。允许Root精确维护catalog JSON/MD/两CSV、engineering baseline、iteration-baseline及R45归档。未经精确独审不采用新基线；不自动重算旧债务/批准图/原change快照。

依赖：已有LibraryLoader与Windows x86/x64 LLVM工具链、同源Core锁、可运行游戏与桌面、相同开发证书。约束：只改变helper自己的当前线程DPI context并恢复；不修改游戏宽度/位置/标题/游戏DPI/注册表/容器/系统休眠/TCC；本轮现有Hook会话保留至正常单次换包。Root独占运行实机probe、安装与GUI，worker不操作游戏。

完成条件：scope正负例真实执行、双架构编译和新旧probe同窗口对照、原严格身份/几何守卫保留、最终同签名包真实窗口列表与预览；Hook/词法/睡眠恢复分层记录。原发布/视觉/性能缺口保持，不以编译冒实际帧通过，不提交推送。
