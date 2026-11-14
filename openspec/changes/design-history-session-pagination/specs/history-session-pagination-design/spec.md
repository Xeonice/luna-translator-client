# 历史页会话与分页设计

## ADDED Requirements

### Requirement: HDP-001

设计候选 MUST 以稳定游戏/会话身份表达历史范围，保留来源与独立手动输入归属，且支持用户选择范围后查看对应历史。

#### Scenario: game-and-session-scopes-retain-real-ownership

- GIVEN 独立待评审候选的游戏、手动输入、会话和记录fixture
- WHEN 用户切换游戏范围或明确选择会话
- THEN 会话与记录按record → session → game关系展示；新范围重置对应首批和请求代次，不把旧范围、显示标题或列表下标当归属。

### Requirement: HDP-002

设计候选 MUST 提供相互独立的会话和记录分页，每批最多50项，并表达初载、追加等待、失败重试、空态和末尾。

#### Scenario: independent-paging-keeps-data-through-recoverable-states

- GIVEN 会话或记录总数超出首批及可注入的独立加载状态
- WHEN 用户加载更早、切换范围、遇到错误并重试
- THEN 同一冻结窗口正确追加、替换仅替换对应范围，旧内容在等待/失败时保留且旧范围清楚标注，空态和末尾不伪装为仍有数据；两类筛选/分页/滚动互不污染。

### Requirement: HDP-003

设计候选 MUST 保护固定句选择、词卡和人工修正草稿，并拒绝旧范围或旧请求的迟到成功与错误结果。

#### Scenario: selection-editing-and-late-results-are-isolated

- GIVEN 选中的旧句、固定词卡、正在编辑或保存失败的草稿及进行中的分页请求
- WHEN 加载更多、收到迟到结果、显式定位当前句或取消/重试修正
- THEN 选择和对象身份不随页变更，编辑期间禁用会切换范围的操作，失败保留草稿，取消恢复原译文；仅当前请求可影响所属窗口，旧响应或旧错误不能覆盖当前内容。

### Requirement: HDP-004

工程系统 MUST 实际执行候选HTML设计检查与Playwright交互，并严格区分设计演示、用户源版批准、独立参照与产品验收。

#### Scenario: real-design-execution-does-not-approve-candidate-or-product

- GIVEN history-session-paging-20261031仍为pending_review且guided已批准57源保持
- WHEN 合同、篡改负例与真实Playwright设计执行运行
- THEN 源身份、实际命令/断言/截图与当前回执相互绑定；旧PNG/token/阈值、当前guided目标、两项known_missing及产品blocked不变；计划not_run不被运行回执覆盖，HTML/工程通过不授予新源版、参照或生产/原生/游戏/发布通过。

### Requirement: HDP-005

工程系统 MUST 通过闭合授权、非作者最终审阅和可恢复事务精确采用R78，保留R77全部批准与历史身份。

#### Scenario: exact-r78-adoption-preserves-all-r77-approval-and-runtime-states

- GIVEN R77原字节父基线、2160源冻结map、1287pins及预先闭合23source
- WHEN 设计4/docs6/tools4/governance8与manifest/generator独立packet审阅后由Root准备采用
- THEN 仅23source与4authority变化，所有原pin路径及全部approvals精确保留；M15只有唯一verify.pySHA与确定性CSV重绑；phase/active/旧计划/产品/历史失败不变，先持久前像再写Main，成功核对完整after、异常核对完整before并留恢复回执。
