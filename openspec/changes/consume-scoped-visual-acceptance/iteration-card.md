# 迭代卡

| 字段 | 内容 |
|---|---|
| Change | consume-scoped-visual-acceptance |
| 用户要求 | 接入自动化门禁；接受当前General S5微小渲染差异 |
| 需求/设计 | FP-11、General V3与用户已确认的27固定组合；不重设计 |
| 模块/组件 | M15工程验证、C23/C32对应只读验证；不改生产组件 |
| Owner | Root：规范/证据准入；visual_acceptance_gate：解析与接线；visual_acceptance_tests：独立负例；非作者终审 |
| 允许路径 | verification.json精确15路径；实现前像见.build/general-visual-acceptance-20261109/admission.json |
| 依赖 | 既有S5包、独立参照、真实Root/React/Native原件和用户决定，缺任一即拒绝 |
| 验证 | 原件hash、同包签名、实际逐图/语义复核；篡改、越界、未来包、错误类型、旧回执重标签负例；Root对应注册执行 |
| 完成 | 自动消费当前定域用户接受，原失败保留；未知/改变输入仍拒绝；局部工程结论不授产品或发布通过 |
