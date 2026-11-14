## ADDED Requirements

### Requirement: FP-12 采集所有者独立于订阅者
系统 SHALL 由独立常驻进程持有窗口采集与授权对象；主窗、候选面板和封面仅订阅，慢消费者或界面失效不能更改采集生命周期。

#### Scenario: collector-process-isolates-subscriber-failure
- **WHEN** 主窗轮询失败、页签卸载、订阅暂停或消费者缓慢
- **THEN** 采集进程实例与当前目标代次保持，控制/采集不被无界队列反压，旧目标帧拒绝；A只通过双PID真实实证，不宣称B界面已集成

#### Scenario: collector-keeps-permission-identity-and-lifecycle-bounds
- **WHEN** worker启动、系统选窗、目标身份/授权变化、迟到回调、异常退出或应用正常退出
- **THEN** 最早分支不打开DB/Host/WebView，私有通道核代次/实例/目标；worker保留自身原filter与完整逐帧核验，不猜继承权限、不回收他人进程或反复弹picker

### Requirement: FP-03 常驻预览必须用真实签名进程验证
系统 SHALL 用同源同签名包验证独立采集；技术PoC不提升最终产品验收。

#### Scenario: signed-collector-proof-precedes-product-integration
- **WHEN** 独立进程准备接入现有正式界面
- **THEN** 先记录实际双PID、worker权限主体、系统单窗选择与多张真实变化帧、父轮询干扰，A通过后才登记B集成；保留已有失败与原视觉门禁
