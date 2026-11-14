## ADDED Requirements

### Requirement: FP-03 游戏窗口发现使用一致坐标口径
系统 SHALL 在窗口查询helper自己的当前线程内以PMv2坐标口径完成枚举与点选，不修改游戏几何或游戏DPI设置。

#### Scenario: window-enumeration-and-picking-share-physical-coordinate-scope
- **WHEN** 查询窗口列表或使用点击选窗且窗口跨屏/负坐标/DPI缩放
- **THEN** 枚举callback和完整光标命中链处于同一当前线程scope；合法负坐标保留，反向/空矩形及身份不完整继续拒绝；原公开JSON与原verify/activate/title行为保持

#### Scenario: query-errors-never-skip-context-restoration
- **WHEN** API缺失、设置被拒绝、查询失败、恢复失败或查询异常退出
- **THEN** 缺能力或设置失败不执行查询；已设置必恢复保存的原context，恢复失败不得输出成功，异常用Drop兜底；错误明确传递，不回退默认坐标或放宽安全验证

### Requirement: FP-12 真实发现与预览回归独立验证
系统 SHALL 分别验证共享逻辑、双架构ABI、真实Wine查询和最终签名包用户可见行为。

#### Scenario: real-window-discovery-precedes-runtime-acceptance
- **WHEN** 本次查询修复进入验收
- **THEN** 保留原空结果和同身份ABA证据，记录新probe有效矩形/身份与最终包窗口列表、绑定及实际不同画面；Hook/词法/睡眠恢复按实测记录，不用旧包或编译成功替代产品通过
