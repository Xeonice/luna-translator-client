## ADDED Requirements

### Requirement: FP-12 独立采集与消费者生命周期分离
系统 SHALL 在独立常驻worker持有窗口授权、核验、采集；其他部分只订阅状态和最新帧，控制仅来自明确应用意图。

#### Scenario: independent-producer-survives-ui-and-hook-consumers
- **WHEN** 切页、最后订阅退出、临时IPC拒绝、慢消费者、翻译/SQL/Hook结束
- **THEN** 不重建或停止采集；实例/原filter/身份保持，真正目标或许可失效仍拒旧帧

#### Scenario: exact-listener-cancellation-and-context-isolation
- **WHEN** 同lease多订阅、旧取消/迟到回复、worker/目标代次变化
- **THEN** 精确取消自身，single-flight有界，旧代次帧不能进入新目标，新页面能读同代次最新有效帧

### Requirement: FP-03 统一正式窗口控制与身份
系统 SHALL 通过正式界面与worker完成选择、取消、重试、关联移交与明确释放；保持公开Tauri/Core JSON合同和完整身份核验。

#### Scenario: unified-window-selection-transfer-and-retry
- **WHEN** 候选选择、系统picker取消/超时/迟到或确认关联
- **THEN** 同App可明确重试且不重启游戏/Main；candidate到session保留原filter并fence新帧代次，不将旧帧冒充新session或封面

#### Scenario: signed-real-frames-precede-product-acceptance
- **WHEN** 准备宣称本窗口修复已完成
- **THEN** 必须有最终同源签名正式包、真实独立worker系统选窗/不同帧及消费者隔离实证；A超时零帧、视觉漂移和其他未验证项继续保留，不用源码/预检/受控组件代替

### Requirement: FP-04 正确呈现暂态与真实终态
系统 SHALL 分离可信绑定、读健康度和最后帧；暂态拒绝不误清绑定，真实终态与换目标拒绝旧帧。

#### Scenario: transient-read-preserves-binding-without-false-live
- **WHEN** 已有可信同context绑定后收到typed暂态拒绝，或收到真实unbound/许可失效/身份变化
- **THEN** 前者保留订阅和原帧时间且不伪造实时；后者按原终态拒帧/撤销旧动作；UI读不再启动Windows周期核验


#### Scenario: unresolved-target-establishes-on-first-validated-frame
- **WHEN** 既有同lease/generation/consumer的初始projection尚无Native target hash，采集器交付projection与帧身份一致的实际首帧
- **THEN** 消费者接受一次目标身份解析并显示首帧、后续新帧持续更新；已知目标变化、错lease/consumer/generation、旧序号仍拒绝；迟到null元数据与新订阅不得回退已解析身份或重建producer

#### Scenario: failed-first-frame-exposes-bounded-stage-evidence
- **WHEN** 系统选窗、原生截图、校验或传输未交付首帧
- **THEN** 同一source/epoch/request的有界固定诊断通过既有状态链可见，区分当前阶段/系统错误码/真实尺寸及无首帧与末帧过期；不得泄露内容、路径、标题、token或将诊断/预检当帧成功


#### Scenario: selected-window-recovers-after-transient-wake-capture
- **WHEN** 原已核验共享单窗口在休眠恢复期间出现屏外可见性、编码或同窗口尺寸暂态失败，随后系统重新提供同一有效窗口画面
- **THEN** 原始filter路径仍执行单窗SCK采集并在既有有界调度中恢复有效新帧，不重启Main/worker/Hook或自动改选窗口；最终错误与原Native阶段可区分，权限/身份失效和预算失败仍拒绝，失败/元数据不得刷新末帧时间
