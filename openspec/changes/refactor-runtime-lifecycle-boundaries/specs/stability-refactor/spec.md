## ADDED Requirements

### Requirement: ENG-RF-001 稳定性阶段优先于剩余功能
系统 SHALL 将当前执行阶段指向稳定性重构及其相关文档，保留既有PRD范围但延期剩余产品内容。

#### Scenario: stability-precedes-feature-completion
- **WHEN** 进入本轮工作
- **THEN** 母change仅授权SR0资料；后续代码按窄子change执行，未满足稳定性退出条件前不混入模型参数、回退、配置导航或其他新增产品内容

### Requirement: ENG-RF-002 状态权威与安全边界明确
系统 SHALL 为Windows关联、Native采集、消费者传输、不可变末帧分别定义事实权威和失效条件，保留各信任边界必要核验。

#### Scenario: state-authority-and-safe-boundaries-remain-explicit
- **WHEN** 迁移状态或生命周期规则
- **THEN** 不以独立PID、去重或简化名义删除完整身份/许可/代际/序号校验，不合并不同seq命名空间，不让UI、封面或单订阅拥有producer生命周期

### Requirement: ENG-RF-003 小切片测试与同包稳定性验收
系统 SHALL 每片同步进行文档、表征测试、实现迁移和独审，最后以同一最终RC完成稳定性旅程。

#### Scenario: each-slice-has-real-regression-and-honest-evidence
- **WHEN** 关闭任一重构切片或进入剩余功能
- **THEN** 有实际可执行检查与适用真实证据；原断言/批准图/失败不被改绿，源码/浏览器/Native/游戏分层，缺前置为blocked而非passed；SR0规划检查不等于业务合同程序化或产品运行adapter已实现
