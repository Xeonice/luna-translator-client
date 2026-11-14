## ADDED Requirements

### Requirement: EGC-001
系统 SHALL 使实际页面清单与设计映射可核对且不删减旧功能和状态义务。

#### Scenario: surface-inventory-preserves-design-obligations
- GIVEN 当前实际8新/3旧入口与已批设计
- WHEN 对齐覆盖并执行注入和生命周期测试
- THEN 已知缺设计保持blocked，旧义务保留，未知入口/错误覆盖仍被拒绝，工具正确评估不等于产品通过

### Requirement: EGC-002
系统 SHALL 在逐项源审和非作者审核后精确采用当前基线及实现状态。

#### Scenario: review-current-source-and-adopt-exact-baseline
- GIVEN R90完整前像、当前源码及逐项有效证据
- WHEN 复审登记并准备采用
- THEN 原R90/未变pins/旧运行证据保留，新pins有审阅来源，缺或失效review实际拒绝，不自引用最终源码hash

### Requirement: EGC-003
系统 SHALL 登记并校验存储验收实际编译测试的完整源闭包。

#### Scenario: registered-storage-test-closure-is-complete
- GIVEN 真实Rust测试模块导入图
- WHEN 验证schema v1/v2 adapter
- THEN 已审6源列入，任一再漏项或越权源仍拒绝，不用登记成功代替存储运行验收

### Requirement: EGC-004
系统 SHALL 仅允许已独立审阅的本次工具增量继续消费原General S5视觉接受。

#### Scenario: scoped-visual-acceptance-survives-reviewed-tooling-only-increment
- GIVEN 原用户接受决定及不变的S5/原件
- WHEN 验证器只出现本次经过非作者审阅的QA/docs变化
- THEN 独立裁决可以验证接受，缺审/源码漂移/未知包/其他语义错误仍拒绝，不扩大原用户批准范围
