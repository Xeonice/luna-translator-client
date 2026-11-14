# SR-6迭代卡

日期2026-10-27，父基线FP1-20261027-R59。用户授权：开始处理，直至整个重构计划完成。

追溯：FP-03, FP-04, FP-05, FP-07, FP-12；现行设计docs/design/first-product-v1/interaction-review.html#01-window-preview；模块M01, M02, M04, M07, M10, M12, M13, M14；组件C11, C13, C14, C18, C19, C34, C35, C36。Root执行真实Computer Use/正式包；QA独立程序复算；AP检查证据与结论；用户仅协助工具不可达的普通操作。。并行协作者不得覆盖或回退他人。

产品精确路径：

依赖：R58本轮实际61步60通过，唯原product-ui-capture-v4失败保留；产品代码已独审，最终本验证change完成输入/计划核对后冻结源码构建唯一签名重构RC。用户已确认完整紧凑设计源，并于本轮明确选择“继续原重构的实机回归，暂不实现新紧凑布局”；本RC不修改客户端布局。保留另一台Steam会话，出现冲突停提示。。复用既有端口与模型，保持公开行为和原断言。Root拥有本change、现状清单/导出、当前阶段/进展、技术说明及新基线审阅包。允许路径以verification为准，无整目录产品授权。

验收：先表征、迁移、独审，再实际运行完整注册检查、编译、边界/五负例及视觉检查。失败保留，不改批准SHA/PNG/token/阈值或测试预期迁就实现。最终同RC游戏/原生与模型条件单独验证。

此前R58验证计划显式由旧R55前像重设到当时已采用R58：原计划、审阅及完整前像单独保留；这不更改产品实现、旧视觉expected、技术锁或基线。实际OpenSpec严格CLI与当前technology合同均须在构建前执行。一个源码封印、固定开发证书、一个实际包与安装映像贯穿所有运行；实际CLI同时给--artifact和--runtime-inputs，存储与实机必须绑定同一ZIP。

本轮R59重新冻结：实际工具门禁58/58、exit0且source稳定1880/tree44135668bf65306983488e47dd4e79fa6eae02fe676085aa23a3245fe54858e2；详runtime-observer-repair-adoption/full-checks-r59/receipt.json。R58原v4失败与三次runtime超时/AX缺crop保留，不继承通过。前像完整source map及旧工作项字节保存在final-stability-r59/preimage；本轮仅重设本计划own4，不再改产品/工具/检查条件。重新固定证书构建并实机新跑，不能复用fd78旧包。
