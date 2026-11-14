## ADDED Requirements

### Requirement: FP-03 ANSI窗口可以明确选择预览对象
系统 SHALL 保留ANSI窗口改标题前的保护，并提供明确的macOS单窗口选择意图恢复预览；用户指定跨命名空间配对，选定后常驻更新，不宣称自动映射已经获得证明。

#### Scenario: ansi-window-preview-preserves-identity-and-title
- **WHEN** 当前有效Windows游戏窗口无法通过自动标题挑战完成预览身份匹配
- **THEN** 保留游戏原标题和几何，显示typed选择required终态与专用选择按钮；仅显式意图打开macOS15.2+单窗口系统picker，选择后保留原filter并前后复核两侧身份与误选过滤；取消不影响Hook，预览成功不作为取文接入前置

#### Scenario: preview-start-failure-has-one-typed-terminal-state
- **WHEN** candidate或session启动失败、选窗取消/不支持/超时/误选或IPC拒绝
- **THEN** Rust已知失败发布一个typed状态，Failed控制槽无成功帧/采集资源，普通刷新和轮询不暗中弹picker或无限启动；UI不残留starting或旧live，Failed slot可按既有resume/stop管理，权限恢复/目标变化/明确操作可重新评估

### Requirement: FP-12 原生预览受身份、容量和生命周期约束
系统 SHALL 在既有Rust权威/窄IPC/Core边界内维护有界可取消的预览；原生系统选择器的单一事务、授权filter、控制槽和采集代际具有可观察真实终态。

#### Scenario: capture-is-bounded-cancellable-and-rejects-stale-target
- **WHEN** 同目标并发选择、取消/90秒超时、旧callback迟到、同PE多活实例、错误或缺失身份/Bottle证据、目标退出/换代、权限变动或捕获预算/耗时超限
- **THEN** 不发布错目标帧，不把系统picker局部授权当全局TCC，精确释放旧filter/槽，不触碰游戏/容器；只有Resolved按generation参与采集和有效首帧封面，candidate生命周期不能停止session，UI旧invoke不能覆盖新consumer pull

### Requirement: FP-04 同包真实游戏画面和阅读必须分别验证
系统 SHALL 用同源、同签名的新包验证真实ANSI目标画面及既有Hook/阅读隔离，不能把受控组件、纯黑图、API成功或旧包结果当实机通过。

#### Scenario: same-package-real-game-frame-and-reading-regression
- **WHEN** 本轮采集修复进入验收
- **THEN** 记录最终包/资源/签名身份、系统选窗真实回调及连续不同实际画面；离屏/最小化等按实际结果表达并正确保留最后成功帧，标题/几何保持；独立回归原文/分词注音/词卡，不修改批准图或继承旧candidate通过，未执行保持not_run
