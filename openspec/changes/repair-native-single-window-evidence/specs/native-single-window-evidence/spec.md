## ADDED Requirements

### Requirement: CAP-001 Single-window request
The read-only observer MUST request the selected window without shadow or attached-window family while preserving the original bounded capture timeout.

#### Scenario: single-window-request-excludes-attached-family-and-shadow
- **GIVEN** 当前指定CGWindowID，原截图可能包含附属窗口union
- **WHEN** 生产截图命令生成并执行
- **THEN** 实际argv包含-a/-o，仍限原指定窗口及原5秒；没有扩大采集或修改游戏几何

### Requirement: CAP-002 Actual canvas validation
The observer MUST validate actual decoded CGImage dimensions before OCR and preserve JSON-safe diagnostics on every rejected canvas.

#### Scenario: actual-image-canvas-rejected-before-ocr-when-inconsistent
- **GIVEN** 实际像素宽高与selected CG bounds可能失配、非有限或溢出
- **WHEN** 读取生产CGImage并运行比例与有限值验证
- **THEN** 失配保留PNG与明确reason且OCR调用为0；合法倍率不硬编码；尺寸一致不声称原点或所有权已证明

### Requirement: CAP-003 Original evidence contracts
The guard suite MUST preserve all original test methods, native cases and runtime sentence acceptance rules while executing the production Swift checks.

#### Scenario: original-native-cases-and-oracle-remain-unchanged
- **GIVEN** 原48 guard与16 native IDs、oracle/阈值/预算已冻结
- **WHEN** 补充generated CGImage生产路径检查
- **THEN** 原方法与断言保留、原16顺序保留，纯测试不冒充真实采屏；不放宽OCR或刷新批准图

### Requirement: CAP-004 Fresh runtime cohort
The workflow MUST retain old failed receipts and require a freshly sealed same-certificate package and independent runtime cohort for actual acceptance.

#### Scenario: new-source-requires-fresh-runtime-cohort
- **GIVEN** R59失败记录与新observer源码身份不同
- **WHEN** 采用修复并准备实机回归
- **THEN** 旧run不恢复或拼接，parent/child独立范围及同包四游戏和真实睡醒另验，未执行不填通过

