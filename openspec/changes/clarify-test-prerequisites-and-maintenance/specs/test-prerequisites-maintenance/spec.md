## ADDED Requirements

### Requirement: TOP-001 Scoped prerequisite inventory
测试供给登记 SHALL 明确用户/研发责任、交付物、就绪现状及受影响的case@variant，且不以历史存在代替当前复验。

#### Scenario: wrong-or-broad-prerequisite
- **WHEN** 删除准备项、引用未知组合、让双屏或多Key要求阻断无关测试
- **THEN** 静态登记检查返回非零，纯规则/合成协议不要求真实Core或词典已准备

### Requirement: TOP-002 Maintained executable source ownership
每条用例 SHALL 映射到符合层和Owner的测试源码区域，区分已有工程守卫、计划产品harness、授权fixtures及生成报告。

#### Scenario: report-substitutes-test-source
- **WHEN** 将测试实现位置指向报告、临时产物或独立Core内部测试以替代客户端消费测试
- **THEN** 维护登记检查拒绝，未知Owner/层级/漏项也拒绝

### Requirement: TOP-003 Readiness and secret boundaries
供给登记 SHALL 保持待确认/准备/评审状态，禁止预标产品通过或在登记/报告中保存原始秘密。

#### Scenario: premature-ready-or-secret
- **WHEN** 记录宣称已就绪/产品passed，或夹带秘密字段与明显秘密值
- **THEN** 检查拒绝且不回显秘密值；不启动产品或真实付费调用

### Requirement: TOP-004 Deterministic operations exports
前置、维护和153组合依赖表 SHALL 从同一JSON登记生成并默认拒绝导出漂移；HTML按组合展示前置和源码维护区域。

#### Scenario: operations-export-drift
- **WHEN** 导出表被改动或漏掉组合
- **THEN** 统一静态检查返回非零；报告另用Playwright核对实际显示/筛选/下载

### Requirement: TOP-005 Preserve planning-only product scope
本轮 SHALL 只补测试准备和长期维护规范及报告/检查设施，保留产品/设计批准与128定义153组合not_run状态。

#### Scenario: unauthorized-product-implementation
- **WHEN** 供给规划变更写入产品模块或擅自提升既有批准
- **THEN** 源范围或基线检查失败；编译和工具自测不能充作产品实测
