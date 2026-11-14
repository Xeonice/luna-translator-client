# 同一签名包 SR6 终验的显式 R65 rebase

本工作项 `verify-refactor-stability-r63` 已由 Root 在 R64 实际落入 Main；ID 保持 schema 2 已登记身份，不代表源码 revision。R64 前像实采为1943源，七文件工作项落地后构建源冻结为1950源／2e2652808333a4075f4ccbd2bcea36da97d53a6985bd555f8e44ac4c827c941f，记录见 final-stability-r64/work-item-prebuild-receipt.json。这些都是保留的历史身份，不能再声称本工作项仅在 ignored 准备目录或当前仅 R63／1933源。

R64 build-rc01 已实际尝试，在当前 language-worker 编译成功后，唯一 staged worker 从18b588...变为2f2a337...；原 preparation 源身份守卫拒绝漂移，catalog closure 也报告该派生资源不一致，Tauri build、ZIP、app、安装和 runtime 均未发生。Root 已保留新旧资源与失败原件并精确恢复旧18b与原1950／2e265树；旧冻结不能冒作之后新资源的身份。

本次方案仅在 `stage-current-language-worker-r65` 经精确独审、Root 真正采用并执行该 R65 的完整工程检查后，基于真实已审 R65 Main 显式 rebase 本工作项。准备时这些前提仍待完成，不预填 R65 数量/hash或通过状态。Root 必须先完整备份现有六文稿与旧1943 snapshot，再从包含已采用 new2f、R65资源WI和authority的真实 Main 调用 source_files 实采新 before；不得复制旧snapshot或把资源从源身份排除。

只修本工作项内四份状态/执行说明文稿，verification 的 ID、scope、需求/设计/模块映射、全部 checks 及 delta spec 原字节保持；新 snapshot 另由 Root 实采、精确独审后采用。rebase 后重新采完整 build map，使用全新 final-stability-r65 输出、新包、新nonce从头执行；R64失败log/source身份、R59/R62结果均保留且不继承。

原50步、全9义务、14links、29 Rust+21 component维护链接、五类负例、600/7200/15秒预算、原wrapper与全部oracle不改。generic50不自动证明全部R64 typed retention分支，实际附加场景证据须另列，未触发保持not_run。compact预览仍仅设计批准且暂不实施，视觉14状态failed保留；不新建expected PNG、不改token/阈值/原计划CSV，不据此提升产品或发布状态。

Root实际采用本rebase前，仅绑定已经真实发生的R65 adoption/fullverify与完整新before及其原始回执，修正文稿中的准备期状态；不要求尚不存在的最终build-map身份。文稿可预先引用稳定输出路径 `.build/stability-refactor-20261027/final-stability-r65/build-source-map.json` 和 `source-freeze-receipt.json`，不预填最终map/hash。四文稿与真实snapshot形成新exact packet经独审并采用五文件后，才从source_files采最终map；实际数量、tree/map SHA及独审冻结结果另存上述ignored外部回执。采用后不为填写最终map/hash回写这些源文稿，否则该源冻结失效，必须另开显式新轮。
