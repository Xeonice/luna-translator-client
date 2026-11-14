# 阅读优先与用户必要配置

## ADDED Requirements

### Requirement: GSR-001

设计系统 MUST 将当前原句、译文与必要阅读动作作为主阅读区域，预览和辅助状态保持清楚但不压过阅读内容。

#### Scenario: current-reading-is-the-primary-content

- GIVEN 当前阅读、等待、失败及无当前句的设计状态
- WHEN 用户打开当前会话并继续阅读
- THEN 当前句与译文的层级和空间清楚，必要控制及恢复入口可达，预览为辅助且保持目标身份，历史日志不占据默认阅读主区域。

### Requirement: GSR-002

设计系统 MUST 将历史、原始提取和Hook诊断后置且默认折叠，同时保留完整数据和各区独立状态。

#### Scenario: supporting-history-and-diagnostics-are-collapsed-after-reading

- GIVEN 会话三数据集、独立筛选/分页/跟随及常驻预览
- WHEN 用户展开、收起、检索或分页后方历史与日志
- THEN 折叠只改变显示，不卸载订阅或重置其他区，不裁剪选流前/未选流记录，完整搜索/导出与缺口警告可达，同一预览持续存在。

### Requirement: GSR-003

设计系统 MUST 从明确供应商/引擎及具体接口路线引导配置，并由精确引用既有目录的呈现合同说明适用信息。

#### Scenario: a-concrete-service-and-route-drive-configuration

- GIVEN 既有13协议、21预设、4离线候选和同品牌多接口资源
- WHEN 用户添加或编辑一种服务
- THEN 呈现合同精确绑定已有资源身份，品牌与路线不混淆，不按名称推断协议或自动切换接口，不编造默认地址、模型、探测结果或兼容能力。

### Requirement: GSR-004

设计系统 MUST 优先显示用户实际需要的必要字段和可理解的缺项原因，并保留适用高级能力与独立学习用途。

#### Scenario: required-fields-guide-users-without-dropping-capabilities

- GIVEN 传统HTTP、LLM及本机引擎和完整/缺失必要字段状态
- WHEN 用户按当前服务完成配置或展开高级项
- THEN 主表单只要求契约适用字段，可信默认和可选项分别表达；未知必要条件不可由隐藏而放行，模板/参数/运行策略/密钥池和精读/查词独立用途仍可达，不虚造模型ID、URL或Key。

### Requirement: GSR-005

设计系统 MUST 保持草稿、测试、保存、选用、启停及历史归属的既有独立语义，完成引导不隐式改变正在使用的服务。

#### Scenario: guided-configuration-preserves-selection-and-version-semantics

- GIVEN 当前绑定、已保存不可变版本及当前游戏/全局范围
- WHEN 用户编辑、检查、测试、保存、返回选用或明确重译当前句
- THEN 配置动作不自动选用/启用/发送游戏文本，已保存资格与禁用原因、HTTP未测试区分、本机资源门禁、范围/继承、后续句和显式重译/旧历史保持原合同。

### Requirement: GSR-006

工程系统 MUST 对新候选执行有当前源码身份的独立设计检查，并保持旧候选、批准源和真实产品状态的独立性。

#### Scenario: fresh-guided-design-evidence-remains-design-only

- GIVEN 冻结R73和GD-01～12未执行产品计划
- WHEN 新正反例、真实Playwright旅程及完整新旧工程检查运行
- THEN 命令、断言、错误、报告与截图绑定当前新候选，旧PA/OT/SS/FL与批准源/PNG/token及历史不被重写，新候选待评审，设计检查不提升产品/系统/真实服务/存储/正式包/游戏或发布状态。
