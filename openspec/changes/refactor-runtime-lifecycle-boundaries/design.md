# 稳定性重构的阶段设计

这是SR0计划与技术契约，不是已完成重构。详见docs/architecture/runtime-lifecycle-refactor.md与runtime-lifecycle-contracts.json；阶段及测试责任见docs/development/stability-refactor-plan.md/JSON、refactor-impact-matrix.json。

四轴分工而非全局万能状态：bindingTrust由M02窗口关联决定，captureTruth由M12 worker/Native事实决定，consumerTransport由每WebView独立订阅管理，lastFrame保持实际已核验图像的不可变身份与采集时刻。保留各信任边界必要核验，尤其Windows完整身份、Native授权filter、source/epoch、lease/generation、序号和许可撤销。worker frame seq与service projection seq不合并。

首片仅提取WindowPreviewService已有的纯状态政策到拟新增私有platform/window_preview/state.rs；公开DTO/IPC、worker/Native、UI/Hook保持。测试先表征再移动，发现不符既定行为时单列缺陷，不修改预期。之后分别收敛私有TS订阅、会话预览/独立封面consumer、Hook facade内部关联/proof/journal、原生几何事件原因。几何若已满足单权威不为拆文件而改；无性能证据不拆SQLite或盲调并发。

本change只有文档精确白名单。后续路径在计划中均为提议，不在此change获得写权限；需子change先登记模块/Owner/精确路径/独审/真实检查。禁止趁重构补参数、回退、OCR或改设计风格。原设计与技术增量审批分开。

SR6同一最终签名RC关闭此次承诺的交叉影响场景后，才启动剩余产品内容；这仍不等于完整首版发布。当前视觉来源冲突是验收前置，单独审阅处理而非顺手改hash或删检查；外部条件可阻塞实机，但不阻塞独立纯规则开发，也不得据此关闭阶段。
