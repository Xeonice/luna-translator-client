# 同页平铺配置与会话

## ADDED Requirements

### Requirement: FSS-001

设计系统 MUST 将翻译服务配置的三类服务与五个配置职能组织为同一页内可达区块，保留对象相关字段和保存/检测/样句/选用语义。

#### Scenario: configuration-families-and-stages-are-flat-sections

- GIVEN 新完整候选及云端、本机和大模型的设计样本
- WHEN 用户管理服务并浏览配置页
- THEN 同页提供三类管理入口及连接、模型、用途、测试、已保存概览，操作不依赖互斥子页签，字段依据所选对象适用性显示，未保存草稿不进入日常选用。

### Requirement: FSS-002

设计系统 MUST 在当前会话同一页面展示当前状态、全部提取事件及Hook事件，保留各自数据边界与独立控制。

#### Scenario: current-session-information-is-visible-on-one-page

- GIVEN 已关联的设计会话及多种事件样本
- WHEN 用户浏览、筛选或分页一个数据区
- THEN 其他区块与常驻预览仍可达，独立筛选/计数/分页不互相污染，完整提取记录不被有界Hook排障日志替代，搜索和导出能力保留。

### Requirement: FSS-003

设计系统 MUST 将重复、高级及排障信息按需呈现，同时保留用户判断状态和恢复所需信息以及原有能力的真实入口。

#### Scenario: advanced-and-diagnostic-details-remain-reachable

- GIVEN 正常、配置不足、失败及完整身份信息的设计状态
- WHEN 用户查看主路径或打开高级/诊断详情
- THEN 主路径显示必要状态和恢复动作，完整参数、身份、资源、错误码、原文与复制入口仍可到达，未以移除能力或无操作说明替代交互。

### Requirement: FSS-004

设计系统 MUST 保留R72已保存服务资格、共享浅层选择器、明确作用范围、独立启停和历史归属，不因平铺重排改变服务绑定行为。

#### Scenario: existing-service-selection-and-history-semantics-are-preserved

- GIVEN 已保存可选服务、未配或已知失败服务、全局/游戏绑定与现有历史
- WHEN 用户保存配置、明确选用、恢复继承或请求重译当前句
- THEN 保存不自动选用，选用保持自动开关且仅影响后续句，禁选原因可见，主/对照去重，显式重译独立并保留旧句和服务版本，配置返回原入口与范围。

### Requirement: FSS-005

工程系统 MUST 为新候选执行可追溯的设计检查并保持批准源、旧候选及产品验收状态的独立性。

#### Scenario: fresh-design-evidence-cannot-promote-product-acceptance

- GIVEN 冻结R72、FL-01～10未来计划和新pending_review候选
- WHEN 新正反例、真实Playwright旅程及完整工程门禁运行
- THEN 新输出绑定实际命令、源码、断言和截图身份，原批准源/PNG/token、旧检查和历史不变，产品/平台/存储/正式包/四游戏保持原not_run或既有状态，不将守卫或浏览器结果继承到真实层。
