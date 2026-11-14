# 收敛当前文档并建立最新设计对齐门禁

## 原因

R75 已将最新 guided 目标与客户端差距、70 来源附加测试闭包连接，但现有入口仍混合旧日期叙述、历史方案与当前义务。文档清理必须同时保留完整产品要求和历史身份；仅清理文字也不能证明产品符合最新设计。因此本轮将当前入口收敛，并新增独立的设计目标能力合同、工程守卫和只读就绪评估。

## 本轮变更

- 保持现有产品 ID 和稳定路径，重排现行说明；以 docs/README.md 和 current-documentation.json 形成可检查的唯一文档入口与职责分工，明确当前规范、待评审目标、历史来源和执行证据。
- 将 20 份 R75 旧文档原字节归档并登记原位置、字节数及 SHA-256；归档不删除产品义务，不迁移产品 ID 路径，不将旧叙述作为并行当前规范。
- 新增 current-design-alignment.json，将最新 guided 目标、需求、设计能力、当前源码/检查能力、必需真实层和对应前置关联；独立区分静态合同、工程检查、设计确认、真实产品运行与完整发布。
- 新增 documentation-contract、documentation-contract-tests、current-design-alignment-contract、current-design-alignment-tests、current-design-readiness-assessment，并接入统一 verify 和实际发布判断；保留 R75 current-product-readiness 与原检查、真实编译及五类负例。
- 本轮仅文档与工程工具，不修改产品、UI、DDL、技术栈、原始 PRD、License、已批准 source/PNG/token、旧机器计划或历史执行状态。

## 身份和边界

父基线为 FP1-20261030-R75，raw SHA-256 c385bb1f06ae38ba83943fc4bd219768ecc71d5699f956c92b3c2eeedc0ec3bf；完整 2102 源冻结树 SHA-256 6b7b7743d85d175e49dd8727906b411a8b565194f7d4edcaa438774810edf6ea，1228 pins。新 revision 为 R76。最新 guided 仍 pending_review，phase、active change、批准和产品状态保持；工程通过不能提升产品准出。不得提交或推送。
