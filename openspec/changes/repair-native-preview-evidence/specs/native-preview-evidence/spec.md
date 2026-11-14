## ADDED Requirements

### Requirement: OBS-001 Native numeric frames
The native observer MUST decode real AX CGFloat frame values through the same finite, shape-checked production geometry path used for preview cropping.

#### Scenario: native-cgfloat-frame-survives-the-production-type-path
- **GIVEN** 实际AX产生原生数值frame，且原Any容器不能靠Double数组强转稳定读取
- **WHEN** 对真实生产Swift纯路径执行数值解码与裁剪换算
- **THEN** 合法原生frame保留精确坐标，非法类型／形状／非有限值明确拒绝，并有旧red→新green证据

### Requirement: OBS-002 Shadowless screenshot geometry
The native observer MUST map actual AX frames to pixels of the same shadowless window screenshot using observed window bounds and actual image dimensions.

#### Scenario: shadowless-window-screenshot-matches-observed-bounds
- **GIVEN** 当前指定窗口逻辑bounds及AXImage包含负原点、Retina或不同x/y比例
- **WHEN** 取得不含阴影的本次窗口PNG并计算裁剪
- **THEN** 裁剪来自正确窗口／实际帧且不越界，不用固定缩放或整窗OCR代替预览图

### Requirement: OBS-003 Explicit observation failures
The native observer MUST preserve bounded stage-specific failure diagnostics without fabricating successful frames, crops, OCR, or capture times.

#### Scenario: each-native-observation-failure-retains-a-real-diagnostic
- **GIVEN** 帧解码、几何、截图、图像解码、OCR、裁剪或写入出现失败
- **WHEN** 只读取证器处理该失败
- **THEN** 保留真实阶段／原因及已有字段，缺图不变成功，不改权限／几何／原runtime oracle或时间预算

### Requirement: OBS-004 Independent source and run cohorts
The acceptance workflow MUST preserve old failed evidence and require a newly frozen, certificate-signed RC and independent runtime cohort after observer source changes.

#### Scenario: old-failed-runs-do-not-become-new-source-acceptance
- **GIVEN** 旧run只有局部OCR匹配而预览裁剪缺失，observer或测试将修改source树
- **WHEN** 采用修后精确源码并准备正式回归
- **THEN** 保留旧red/超时及源码身份，不改旧包外部JSON冒充新seal；guard通过不提升四游戏／SleepWake／产品视觉通过
