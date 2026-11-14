## ADDED Requirements

### Requirement: VUA-001
系统 SHALL 自动消费精确用户接受的既有视觉结果，并保留自动像素结果及其他验收身份。

#### Scenario: exact-user-accepted-retained-visual-result
- GIVEN 用户明确接受的General S5、完整独立原件和代码登记决定
- WHEN 工程门禁重新验证该既有结果
- THEN 生成独立用户接受裁决，旧0RGBA失败保持原字节；不再为该像素差异启动重拍或修UI
- AND 真实逐图/语义/身份复核及当前工具源码身份记录，不声称新执行了产品捕获

### Requirement: VUA-002
系统 SHALL 拒绝接受范围之外、证据变化或非像素错误的自动豁免。

#### Scenario: refuse-scope-integrity-and-semantic-bypass
- GIVEN 任一决定/原件/包/源码/案例/几何/可见语义或检查结果被改变
- WHEN 工程门禁消费用户接受决定
- THEN 实际非零拒绝；新包不能继承本次接受，其他全局门禁和产品完成判断不提升
