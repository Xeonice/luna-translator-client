# HUD后台状态刷新稳高

## ADDED Requirements

### Requirement: HJ-001

系统 MUST 满足HJ-001行为，且保留已有阅读合同。

#### Scenario: unchanged-content-query-refresh-does-not-pulse-hud-height

- GIVEN 真实生产React和当前对象投影
- WHEN 后台query/任务/正文状态变化
- THEN 同原文同译文下无关commit触发 held query，不新增自然内容高度或fit请求；query读取保持active且反馈可访问。

### Requirement: HJ-002

系统 MUST 满足HJ-002行为，且保留已有阅读合同。

#### Scenario: visible-errors-and-default-notices-remain-observable

- GIVEN 真实生产React和当前对象投影
- WHEN 后台query/任务/正文状态变化
- THEN 默认主窗状态Notice仍可见；HUD实际query错误/unavailable及重试、真实任务错误/流式译文/取消保持可观察。

### Requirement: HJ-003

系统 MUST 满足HJ-003行为，且保留已有阅读合同。

#### Scenario: natural-content-fit-and-query-owner-contracts-remain

- GIVEN 真实生产React和当前对象投影
- WHEN 后台query/任务/正文状态变化
- THEN 长短正文自然适高、同内容去重、owner late、single-flight与commit floor保持；不固定整个HUD，不改变原生几何。
