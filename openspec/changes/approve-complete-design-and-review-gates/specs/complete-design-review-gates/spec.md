# 完整设计批准与强制审阅

## ADDED Requirements

### Requirement: CDA-001

工程系统 MUST 精确记录真实用户对当前61源完整设计组合的批准，并分离原57源历史批准与当前组合有效状态。

#### Scenario: explicit-current-composition-source-approval-is-effective

- GIVEN 用户明确批准全部当前设计、R79固定guided57+history4及新complete批准记录
- WHEN 当前权威解析校验用户原话、父身份、target/composition/routes和61源闭包
- THEN 当前组合有效approved_by_user，原61字节与pending标签、旧57批准及PNG/token/阈值保持，不授予独立参照或产品/原生/游戏/服务/发布通过。

### Requirement: CDA-002

工程系统 MUST 对已批准历史页给出准确设计映射，同时保留全部真实层执行与独立参照义务。

#### Scenario: history-mapping-closes-design-gaps-without-promoting-product

- GIVEN 794现行状态ID、history20状态及已批准R79历史设计
- WHEN history映射更新为17existing、3仍待映射、0missing
- THEN 794ID/真实required_layers/reference/execution及ADAPTERS保持，缺真实执行器不改成不适用，产品blocked不提升，真实设计与真实产品执行分别报告。

### Requirement: CDA-003

统一verify MUST 无条件对实际diff和最终完整源树要求独立非作者批准报告，不能仅凭metadata声明或检查ID省略绕过。

#### Scenario: actual-diff-needs-exact-independent-non-author-review

- GIVEN 实际45变更文件、最终after树和metadata固定报告路径/四作者身份
- WHEN verify读取schema1 independent-change-review/v1报告并实际校验身份、范围、hash与阻断意见
- THEN 只有非作者、完全一致author_ids、approved、独立标志true、无阻断且45files/wholeafter精确才通过；缺失/自审/错源/旧树/漏多重复路径/伪通过实际拒绝，报告SHA实时进回执而不写回源metadata。

### Requirement: CDA-004

客户端技术设计 MUST 与当前完整设计的历史范围、双分页和对象保护契约同步，且不虚报正式实现或验收。

#### Scenario: client-design-and-component-targets-match-approved-history

- GIVEN 已有HistoryScreen/Repository和当前61源设计
- WHEN 同步架构技术文档、CP-11及C23/C24/C36目标
- THEN 游戏/会话独立50条cursor、请求代次/迟到隔离、保留选择/滚动/词卡/草稿及显式当前句定位明确；Rust权威与既有端口保留，状态/包/历史/产品事实不改，不新增DDL或产品实现。

### Requirement: CDA-005

工程系统 MUST 通过闭合41source授权、四组非作者审阅和可恢复事务精确采用R80，只增加真实complete源版批准键。

#### Scenario: exact-r80-adoption-preserves-history-and-adds-only-complete-source-approval

- GIVEN R79原字节基线、2190源冻结图、1317pins与预授权41source
- WHEN 最终11/16/7/8组审、45文件packet/generator及wholeafter gate报告由非作者独审后由Root准备采用
- THEN 仅41source+4authority改变，全部旧pins路径和旧approvals保留，只新增complete_design_source_version；先fsync前像后写Main，成功核完整after、异常核完整before并留证；旧PNG/阈值/plan/DDL/phase/active/product及失败历史不提升。
