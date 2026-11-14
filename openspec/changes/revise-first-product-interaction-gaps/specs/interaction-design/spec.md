# 首版六项交互设计修订

## ADDED Requirements

### Requirement: DREV-001 运行窗口预览
运行窗口预览 SHALL 显示窗口身份、帧来源及更新时间；权限拒绝、目标退出、旧帧和回放可区分。

#### Scenario: preview-provenance
- **WHEN** 评审者从现行整稿的实际入口操作该场景
- **THEN** 候选设计明确表达上述行为及未实施边界，并由独立设计检查留证

### Requirement: DREV-002 HUD
HUD SHALL 有可操作拖区、位置与屏幕边界规则及可配置快捷键；浏览器演示不声称全局注册。

#### Scenario: movable-hud
- **WHEN** 评审者从现行整稿的实际入口操作该场景
- **THEN** 候选设计明确表达上述行为及未实施边界，并由独立设计检查留证

### Requirement: DREV-003 产品Select
产品Select SHALL 展示统一自定义触发器与展开菜单，包含键盘、焦点、禁用和长文本状态。相邻选项的高亮区域须有清楚间隔、不相接或重叠；鼠标和键盘共用唯一活动项，已选勾记独立，禁用项不高亮。

#### Scenario: custom-select
- **WHEN** 评审者从现行整稿的实际入口操作该场景
- **THEN** 候选设计明确表达上述行为及未实施边界，并由独立设计检查留证

### Requirement: DREV-004 当前会话
当前会话 SHALL 能访问本会话完整提取文本和Hook事件；分页不截断总量，原始来源与最终正文区分。

#### Scenario: full-session-data
- **WHEN** 评审者从现行整稿的实际入口操作该场景
- **THEN** 候选设计明确表达上述行为及未实施边界，并由独立设计检查留证

### Requirement: DREV-005 HUD
HUD SHALL 表达半透明材质与减少透明度回退；分词、注音独立可见，词法失败有明确降级。

#### Scenario: visible-ruby-and-material
- **WHEN** 评审者从现行整稿的实际入口操作该场景
- **THEN** 候选设计明确表达上述行为及未实施边界，并由独立设计检查留证

### Requirement: DREV-006 交付
交付 SHALL 逐条说明既有门禁覆盖和漏检原因，保留原始失败/通过回执与新设计未评审状态。

#### Scenario: coverage-gap-explanation
- **WHEN** 评审者从现行整稿的实际入口操作该场景
- **THEN** 候选设计明确表达上述行为及未实施边界，并由独立设计检查留证

