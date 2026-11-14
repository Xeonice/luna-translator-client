# 整套guided源版批准与全页覆盖

## ADDED Requirements

### Requirement: GDA-001

工程系统 MUST 根据本轮明确用户授权，使用独立批准记录使当前整套reviewed guided源版本的有效用户确认生效。

#### Scenario: explicit-whole-source-user-approval-is-effective

- GIVEN 用户明确确认当前整套guided审核通过及R76冻结源版
- WHEN 批准记录和共享只读权威解析器校验固定target、用户语句、范围、记录身份、57源和固定闭包
- THEN 权威文档与实际门禁读取approved_by_user，不再无条件声称无用户源版批准；重复许可不是前置，独立参照与产品执行不据此通过。

### Requirement: GDA-002

工程系统 MUST 保留已批准57源的原字节、捕获时pending标签与历史批准资产，禁止自动刷新来源或批准图迁就新状态。

#### Scenario: approval-does-not-rewrite-captured-sources-or-png-policy

- GIVEN 原manifest/catalog/presentation快照、固定57源闭包及20261019批准记录
- WHEN 记录新的有效用户源版确认
- THEN 原57源/PNG/token/source-version/零差异阈值与旧批准原身份保持，当前批准引用外置；篡改记录、scope、目标或源hash即拒绝，JSON与adapter不能自授参照或产品批准。

### Requirement: APG-001

工程系统 MUST 用独立可检查清单闭合全部当前设计页面/scenario及真实产品AST surface，而非只覆盖旧阅读与设置状态。

#### Scenario: design-pages-and-actual-product-surfaces-are-closed

- GIVEN 当前设计源与实际产品入口/导航/组件AST
- WHEN 校验全页覆盖合同
- THEN 每项映射需求、状态、M/C、Owner与适用层，退役入口明确保留当前差异；遗漏、重复、未知surface或伪造统计拒绝，不以临时固定总数代替独立完整性检查，不改旧70/576或CSV身份。

### Requirement: APG-002

工程系统 MUST 区分缺设计、缺执行recipe及有依据的不适用状态，保留全部适用真实验证义务。

#### Scenario: execution-recipe-gaps-never-negate-approved-design

- GIVEN 当前源版已获用户批准且部分页面/状态没有真实执行recipe
- WHEN 分类适用状态、前置和全页readiness
- THEN 缺recipe记为执行能力缺口，不误报已有设计未经用户批准；不适用必须有范围依据，unknown/缺adapter/待实现不能改为不适用逃避必测，真实设计缺口与产品未对齐分别可见。

### Requirement: APG-003

工程系统 MUST 在实际开发与release路径检查适用全页真实层，同时保持既有产品覆盖及批准/执行边界。

#### Scenario: all-page-engineering-checks-do-not-promote-product-acceptance

- GIVEN 正确批准记录、全页合同及仍缺独立参照/生产adapter/同一最终包的事实
- WHEN 统一verify及只读readiness实际运行
- THEN 工程合同与负例独立执行，原153/RW/存储/当前576及Windows4累计；缺参照、真实React/native或证据保持blocked/not_run，旧v4/HTML/fixture和自报passed不能替代当前全页产品执行。

### Requirement: GOV-001

工程系统 MUST 通过闭合授权、独立最终文件审阅和可恢复事务采用精确R77差异，只新增本轮真实用户源版批准元数据。

#### Scenario: exact-r77-adoption-preserves-r76-with-one-explicit-approval-key

- GIVEN R76原字节父基线、2142源冻结map、1269pins与预先闭合48source
- WHEN 独立docs24/tooling15/governance8及packet审阅后由Root准备和采用R77
- THEN 仅48源和4authority变化；旧approvals键值全部保留，新增guided_design_source_version精确绑定target/approved_by_user/record/closure；其余metadata、phase、activechange、产品和历史状态保持，M15仅唯一hash与确定性CSV，先持久回滚前像再写Main并验证真实receipt，失败记录恢复完整性。
