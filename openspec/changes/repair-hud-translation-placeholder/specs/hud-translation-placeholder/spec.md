# HUD真实翻译投影占位

## ADDED Requirements

### Requirement: HTP-001

系统 MUST 满足HTP-001，保持Rust状态权威和现有边界。

#### Scenario: hud-task-progress-replaces-generic-empty-placeholder

- GIVEN 真实生产HUD、同一阅读对象和Rust投影
- WHEN query和翻译状态变化
- THEN 真实queued/running/partial显示对应进度且不并列通用尚无译文；completed显示真实译文。

### Requirement: HTP-002

系统 MUST 满足HTP-002，保持Rust状态权威和现有边界。

#### Scenario: hud-empty-and-recovery-state-remain-truthful

- GIVEN 真实生产HUD、同一阅读对象和Rust投影
- WHEN query和翻译状态变化
- THEN 失败/取消/unordered/unavailable/query-error可观察可恢复；无任务只述已知事实，不能推断配置原因。

### Requirement: HTP-003

系统 MUST 满足HTP-003，保持Rust状态权威和现有边界。

#### Scenario: single-query-owner-and-layout-stable-feedback-are-preserved

- GIVEN 真实生产HUD、同一阅读对象和Rust投影
- WHEN query和翻译状态变化
- THEN 唯一query owner、late identity隔离、R66稳高与自然长短fit保留；默认Main/history/study行为保持。
