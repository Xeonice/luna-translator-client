# 会话目录次要操作的设计层级

## ADDED Requirements

### Requirement: HSW-001

历史设计候选 MUST 将“加载更早会话”呈现为轻量次要文字操作，保持响应式布局与可达性。

#### Scenario: session-paging-action-has-light-secondary-weight

- GIVEN 待审历史页当前常驻有底色/边框的会话目录按钮及用户要求收轻
- WHEN 仅修改其候选CSS并在深浅色、窄窗、长标签和200%布局查看
- THEN 默认态无常驻可见底色或边框，不挤压筛选与状态，仍可由鼠标和键盘到达，候选截图独立保留修改前后对照。

### Requirement: HSW-002

工程系统 MUST 保留该操作的全部现有交互和状态，不用修改测试预期或产品行为实现样式通过。

#### Scenario: style-only-adjustment-retains-existing-state-semantics

- GIVEN 现有会话50条独立分页、加载/失败重试/末尾禁用、focus-visible/hover及HP-01～12执行义务
- WHEN 会话操作变为文字层级并执行既有history-design实际检查与完整verify
- THEN 行为、键盘焦点、hover/disabled和spinner/文案继续可观察；index.html/history.js、PRD、旧测试/断言/阈值/计划与正式客户端不变，manifest仅更新CSS实际SHA。

### Requirement: HSW-003

工程系统 MUST 通过闭合授权、非作者独审和可恢复事务精确采用R79，保持R78全部批准与历史状态。

#### Scenario: exact-r79-adoption-preserves-review-and-product-boundaries

- GIVEN R78原字节父基线、2180源冻结图、1307pins和编辑前9source授权
- WHEN design2与governance7+helper8及manifest/generator独立packet审阅后由Root准备和采用
- THEN 仅9source与4authority变化，原pins路径与全部approvals保留，无verify/M15重绑或旧change修改；先持久前像再写Main、成功核完整after、异常核完整before；候选pending_review、原guided批准、57源/PNG/阈值、全页两缺口、旧计划和产品blocked不提升。
