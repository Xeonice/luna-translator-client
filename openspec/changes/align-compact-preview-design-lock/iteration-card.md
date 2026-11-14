# SR-DESIGN-LOCK迭代卡

日期2026-10-27，父基线FP1-20261027-R56。用户授权：开始处理，直至整个重构计划完成。

追溯：FP-03, FP-05, FP-12；现行设计docs/design/first-product-v1/interaction-review.html#01-window-preview；模块M12；组件C35。Root拥有technology-lock单文件和本change/authority；AP独立审阅；并行report/context仅写ignored证据，不覆盖他人。并行协作者不得覆盖或回退他人。

产品精确路径：
- openspec/technology-lock.json

依赖：R56已正式采用并完成当前scope的61步59通过，原product-ui-capture-v4与5旧design锁拒绝均保留，当前源1857全程稳定；当前actual design34/1686/98及两轮原旅程17/50独立证据已通过，不自动冒充产品验收。复用既有端口与模型，保持公开行为和原断言。Root拥有本change、现状清单/导出、当前阶段/进展、技术说明及新基线审阅包。允许路径以verification为准，无整目录产品授权。

验收：先表征、迁移、独审，再实际运行完整注册检查、编译、边界/五负例及视觉检查。失败保留，不改批准SHA/PNG/token/阈值或测试预期迁就实现。最终同RC游戏/原生与模型条件单独验证。

针对R56前置审阅遗漏，本片源码与精确候选包独审均必须额外实际执行 `python3 scripts/quality/technology_contract.py --json`，不能以technology-contract-tests的fixture通过代替当前工程合同。全门禁中的technology-and-design-contract仍为真实检查；本片只登记5个已批准设计源的SHA叶值，其余技术锁字段逐值保留。原R56的59/61及两项失败回执单独保存，不回写为通过。

R57真实完整运行中，OpenSpec严格语法正确拒绝了本片中文规则缺少MUST/SHALL的格式错误。本次R58仅补同义规范关键词、允许精确父R57归档及本记录；产品与5SHA技术锁内容不再改变。源码与精确packet审阅必须额外实际执行固定OpenSpec CLI的 `validate --all --strict --no-interactive`，不能以自定义spec_contract校验替代。原R57失败回执保留，不回写旧批准或旧结果。
