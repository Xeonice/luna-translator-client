# GitHub 门禁

## ADDED Requirements

### Requirement: ENG-GH-01 公开源码与资源可恢复
系统 SHALL 公开仓库保留许可与固定资源身份，排除本机现场，冷机恢复资源。

#### Scenario: 公开源码与资源可恢复
- **WHEN** 执行该GitHub门禁场景
- **THEN** 按真实执行/原件判定，不得用配置存在或旧结果表示通过

### Requirement: ENG-GH-02 实际Git基线与范围
系统 SHALL 使用真实存在base比较，首次导入审查，不接受全零base自报快照。

#### Scenario: 实际Git基线与范围
- **WHEN** 执行该GitHub门禁场景
- **THEN** 按真实执行/原件判定，不得用配置存在或旧结果表示通过

### Requirement: ENG-GH-03 部署根与真实Hook身份
系统 SHALL 允许固定的部署根归一化；源码、真实事件、匹配器/超时及副作用不能变化。

#### Scenario: 部署根与真实Hook身份
- **WHEN** 执行该GitHub门禁场景
- **THEN** 按真实执行/原件判定，不得用配置存在或旧结果表示通过

### Requirement: ENG-GH-04 冷机完整门禁
系统 SHALL 生成真实生产回执，消费源绑定L3与独审，运行真实编译及完整32组，缺证据拒绝。

#### Scenario: 冷机完整门禁
- **WHEN** 执行该GitHub门禁场景
- **THEN** 按真实执行/原件判定，不得用配置存在或旧结果表示通过

### Requirement: ENG-GH-05 必需检查真正阻断
系统 SHALL 正常PR通过，越界PR失败不可合并；读回main实际保护与Actions回执。

#### Scenario: 必需检查真正阻断
- **WHEN** 执行该GitHub门禁场景
- **THEN** 按真实执行/原件判定，不得用配置存在或旧结果表示通过

