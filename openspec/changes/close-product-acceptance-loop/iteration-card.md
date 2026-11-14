# 验证闭环优先 · Root迭代卡草案

- 用户原话：这块先补充验证闭环，然后再做改造；产品门禁直接算没做完。
- Change：close-product-acceptance-loop，M15工具与技术规范。Owner /root；离线浏览器测量Owner /root/settings_arch_review（本轮作者）；独立审阅另由非作者承担。
- 前置：a39e完整门禁自然结束；阶段报告完成且源身份复核；受管OpenSpec先于受管代码。当前仅ignored计划/原型，不改变产品或其准出状态。
- 需求：VAC-001实际渲染资格；VAC-002整页与client上下文；VAC-003版本/运行者/实际执行闭包；VAC-004单一未完成结论及禁止推进；VAC-005真实正负验证。复用现有FP-04/06/11/12与全部既有测试义务，不增产品功能。
- 允许候选路径：scripts/quality/product_completion.py、test_product_completion.py、render_evidence.py、test_render_evidence.py、current_design_react_{adapter,inspect,recompute}.py、test_current_design_react_adapter.py、current_design_alignment.py、verify.py；scripts/report/measure_render_evidence.mjs、verify_current_design_react.mjs；docs/development/{product-acceptance-loop.md,engineering-gates.md,current-phase.json,current-client-alignment-plan.md,iteration-baseline.json,baselines/FP1-20261105-R86.json}；docs/README.md；implementation-status JSON及三个派生导出仅更新实际工具源码锚；新OpenSpec自身六文件及delta spec。最终以受管verification.json精确路径为准。
- 禁止：apps/crates/native/resources产品根、原设计/参照/批准图/token/threshold/debt、游戏尺寸、用户应用/登录项/模型服务、提交推送。
- 完成条件：DOM/固定资产独立离线浏览器测量有真实正负原件；实际渲染/整页/Native/身份缺证据时不可比或未完成；aggregate与advance guard从既有registry及inspect/readiness衍生，不能凭局部passed或fixture/native字段升绿；当前真实产品结果必须仍incomplete/next_stage_allowed=false。
- 严格区分：守卫测试通过≠产品通过；旧a39e实际失败保留；本轮只修验收工具，UI改造后续单独迭代。

受管OpenSpec已先于受管实现创建。精确允许路径及命令以verification.json为准。
