## ADDED Requirements

### Requirement: VAC-001
系统 SHALL 实际渲染资格必须由原DOM与固定资产的独立浏览器复测决定，不能信相同偏好或producer标签。

#### Scenario: effective-render-state-is-independent
- **WHEN** 执行本轮对应验证
- **THEN** 检查原始证据并拒绝缺失或不一致，保持产品未完成直至所有适用实际义务闭合。

### Requirement: VAC-002
系统 SHALL 局部article、desktop viewport和真实client上下文必须分开；没有独审whole-client参照与有效执行不得完成整页。

#### Scenario: whole-client-context-is-not-a-target-crop
- **WHEN** 执行本轮对应验证
- **THEN** 检查原始证据并拒绝缺失或不一致，保持产品未完成直至所有适用实际义务闭合。

### Requirement: VAC-003
系统 SHALL 既有source/package/exe/PID-cohort/profile/instance证据只属于其具体受测对象，浏览器和旧包不能代替当前Native。

#### Scenario: runtime-and-artifact-subjects-stay-bound
- **WHEN** 执行本轮对应验证
- **THEN** 检查原始证据并拒绝缺失或不一致，保持产品未完成直至所有适用实际义务闭合。

### Requirement: VAC-004
系统 SHALL 任意必须项失败、缺失、blocked、not_run、不可比或身份不符都产生incomplete与next_stage_allowed=false。

#### Scenario: incomplete-product-cannot-advance
- **WHEN** 执行本轮对应验证
- **THEN** 检查原始证据并拒绝缺失或不一致，保持产品未完成直至所有适用实际义务闭合。

### Requirement: VAC-005
系统 SHALL 工具正负验证有真实原件，实际产品的未完成结果保留，不能把守卫通过当产品完成。

#### Scenario: actual-positive-and-negative-closure-evidence
- **WHEN** 执行本轮对应验证
- **THEN** 检查原始证据并拒绝缺失或不一致，保持产品未完成直至所有适用实际义务闭合。
