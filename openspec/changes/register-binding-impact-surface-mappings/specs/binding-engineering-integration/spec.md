## ADDED Requirements

### Requirement: BIM-001 合法父末态与后继QA交接
工程工具 SHALL 按本变更设计中的实际源、执行原件及失败闭包核验；任何规范/登记/守卫通过均不授产品、Native、正式包或发布验收。

#### Scenario: handoff-requires-actual-adoption-and-reviewed-parent
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 H01 对应原件消费与真实正反例
- THEN 缺实际R93 adopted receipt、95-pin实际采用记录或父非作者seal审阅拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: projected-or-rolled-back-source-cannot-be-base
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 H02 对应原件消费与真实正反例
- THEN 仅有预测map、尚未完成或已回滚的事务，即使预测hash等于c655也不构成实际Base；必须有actual adopted receipt、非作者独审和同树原guard的真实原件
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: parent-oldguard-is-actual-and-same-tree
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 H03 对应原件消费与真实正反例
- THEN 父oldguard非实际原--check、exit非0或前后全图变化拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: parent-authority-and-failures-remain-immutable
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 H04 对应原件消费与真实正反例
- THEN 改父完整map、739产品map、旧policy/23plan/16gate归档或原签字/raw失败拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: different-change-id-cannot-skip-handoff
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 H05 对应原件消费与真实正反例
- THEN 别的change_id触发required=false不能代替successor交接；省略metadata检查仍强制handoff
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: snapshot-equals-parent-and-products-stay-zero-diff
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 H06 对应原件消费与真实正反例
- THEN QA编辑后重拍snapshot、snapshot与ParentSeal不等、任何产品字节/重命名变化拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: independent-scope-and-safe-paths-are-required
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 H07 对应原件消费与真实正反例
- THEN 同作者自审、漏作者、漏actualpath、stale review、symlink/alias逃逸拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: ci-requires-real-git-base
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 H08 对应原件消费与真实正反例
- THEN CI无真实Git base或以snapshot代替真实base拒绝；原P3差异不能被新QA标签吃掉
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false


#### Scenario: implemented-parent-and-amendment-review-chain
- GIVEN 同一R93实际父与4515已独审19项实现、新r2阶段的真实原件
- WHEN 执行 H09 对应实际消费及正反例
- THEN 合法4515已实施19项须绑定a0bc/b11/d71、真实checkpoint与旧R94失败回滚；缺完整父图/签字/raw、使用8909预测或rolled_back作Base、漏作者/自审、旧失败原件改变均拒绝
- AND 原30场景及完整义务保留，产品incomplete且next_stage_allowed=false

#### Scenario: active-r2-no-downgrade-and-phase-order
- GIVEN 同一R93实际父与4515已独审19项实现、新r2阶段的真实原件
- WHEN 执行 H10 对应实际消费及正反例
- THEN CLI与verify按actualdiff强制active r2精确review/auth；缺删错r2authority、metadata删amendment、退回r1、换ID/no-op或P1/P2提前改gate/catalog/未授权19路径均拒绝
- AND 原30场景及完整义务保留，产品incomplete且next_stage_allowed=false

### Requirement: BIM-002 真实生命周期literal登记与旧模块归属
工程工具 SHALL 按本变更设计中的实际源、执行原件及失败闭包核验；任何规范/登记/守卫通过均不授产品、Native、正式包或发布验收。

#### Scenario: lifecycle-registers-twelve-real-literal-paths
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 M01 对应原件消费与真实正反例
- THEN 删C17旧归属、错M10/M14命名、漏12literal规则/依赖或缺派生check拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

### Requirement: BIM-003 页面AST映射与累计状态义务
工程工具 SHALL 按本变更设计中的实际源、执行原件及失败闭包核验；任何规范/登记/守卫通过均不授产品、Native、正式包或发布验收。

#### Scenario: surface-lineage-preserves-old-state-identities
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 M02 对应原件消费与真实正反例
- THEN 删旧state/旧source descriptor/required layer，错新AST或只JSON声明executor拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: missing-design-reference-or-executor-remains-blocked
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 M03 对应原件消费与真实正反例
- THEN 新connecting/overlay无精确recipe/独立参照仍needs_mapping/blocked，不能自动通过
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

### Requirement: BIM-004 历史Store4产物身份与当前v5严格执行
工程工具 SHALL 按本变更设计中的实际源、执行原件及失败闭包核验；任何规范/登记/守卫通过均不授产品、Native、正式包或发布验收。

#### Scenario: historical-mutable-target-may-change-with-sealed-copy-intact
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 S01 对应原件消费与真实正反例
- THEN 原9suite的mutable target合法覆盖或消失，封存源/compile/list/raw/copy全真时应可消费
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: old-suite-source-compile-copy-list-and-assertions-are-exact
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 S02 对应原件消费与真实正反例
- THEN 任一copy hash、冻结源/Cargo.lock、Cargo artifact/package/test/src_path/list/原126断言/零skip链不实拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: all-nine-original-suites-and-original-raws-are-required
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 S03 对应原件消费与真实正反例
- THEN 少原9suite/换另批copy/错actual argv/符号链接/把copy写回target伪造历史拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: historical-rule-does-not-relax-current-v5-child-identity
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 S04 对应原件消费与真实正反例
- THEN 历史窄修不能豁免当前fresh Store5父子PID/源/binary/nonce/真实OS故障/SQLite闭包
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

### Requirement: BIM-005 请求检查的真实派发与防降级
工程工具 SHALL 按本变更设计中的实际源、执行原件及失败闭包核验；任何规范/登记/守卫通过均不授产品、Native、正式包或发布验收。

#### Scenario: explicit-qa-full-and-v5-checks-are-dispatched
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 D01 对应原件消费与真实正反例
- THEN QA-only明确请求原full BIND/v5但无实际派发/命令原件拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: assessment-and-v5-use-one-fresh-full-execution
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 D02 对应原件消费与真实正反例
- THEN assessment与strict v5共享同一fresh full执行；重复运行混nonce/source或借旧结果拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: actual-product-impact-keeps-original-full-check
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 D03 对应原件消费与真实正反例
- THEN 实际产品/存储路径即使只声明assessment，auto-required原full仍不可删除或降级
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: unknown-check-and-requested-phase-do-not-authorize
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 D04 对应原件消费与真实正反例
- THEN 未知check及任意requested phase-admission冒授权拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: declared-checks-require-actual-receipts
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 D05 对应原件消费与真实正反例
- THEN 没真实执行只填checks名称、零test/skip或receipt缺失拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: release-and-product-completion-remain-blocked
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 D06 对应原件消费与真实正反例
- THEN full blocked仍是full blocked；release/require-product-complete仍实际非零
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

### Requirement: BIM-006 fresh全BIND的独立readiness评估
工程工具 SHALL 按本变更设计中的实际源、执行原件及失败闭包核验；任何规范/登记/守卫通过均不授产品、Native、正式包或发布验收。

#### Scenario: semantic-layer-or-observer-errors-fail-assessment
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 A01 对应原件消费与真实正反例
- THEN 任何semantic failure、layer failed、EX failed、validation/infra/recompute错误令assessment失败
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: strict-v5-must-actually-pass
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 A02 对应原件消费与真实正反例
- THEN schema5不为strict passed、缺19边界/8路由/9timeline/4OS故障/原9suite任何一项失败
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: local-layer-or-producer-gaps-are-not-external-blocking
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 A03 对应原件消费与真实正反例
- THEN 缺domain/component/storage/guard或EX前6组生产者不得归入可接受Native缺层
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: only-absent-native-service-inputs-remain-blocked
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 A04 对应原件消费与真实正反例
- THEN 缺层仅从实际context空Native/service证据和固定registry推导；提供但不合法/零RGBA失败的证据不得当缺省
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: fresh-source-process-and-artifact-lineage-is-exact
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 A05 对应原件消费与真实正反例
- THEN source/nonce/PID/binary/实际请求/DB和源闭包混批、借旧raw或不fresh执行拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: all-original-journeys-and-obligations-remain
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 A06 对应原件消费与真实正反例
- THEN 删改18BJ、9EX、7finite+38原场景+2LIB、153/RW/901/RBC/4game任一义务拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

#### Scenario: assessment-never-forges-full-or-product-passed
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 A07 对应原件消费与真实正反例
- THEN 新增assessment不得发出binding-journey-execution=passed或product_complete/nextstage=true
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false

### Requirement: BIM-007 独立审阅后的精确下一基线采用
工程工具 SHALL 按本变更设计中的实际源、执行原件及失败闭包核验；任何规范/登记/守卫通过均不授产品、Native、正式包或发布验收。

#### Scenario: next-baseline-is-adopted-only-from-reviewed-pin-delta
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 P01 对应原件消费与真实正反例
- THEN 下一baseline覆盖R93、擅改未变pin/debt或无精确非作者pin-delta审阅拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false


#### Scenario: qa-only-catalogue-proof-and-protected-product-fields
- GIVEN 同一R93实际父与4515已独审19项实现、新r2阶段的真实原件
- WHEN 执行 P03 对应实际消费及正反例
- THEN 冻结实际r2 gate/doc后重新审M15 active current入口/v2六输出语义，不能stamp四SHA、沿旧十锚点上限或仅历史v1；单一current QA-only proof保R93 lineage，M13仅901→944 next_step计数，78产品字段或ComponentsCSV改变、升格产品/Native/包事实均拒绝
- AND 原30场景及完整义务保留，产品incomplete且next_stage_allowed=false

#### Scenario: atomic-six-output-and-unchanged-components-export
- GIVEN 同一R93实际父与4515已独审19项实现、新r2阶段的真实原件
- WHEN 执行 P04 对应实际消费及正反例
- THEN 新namespace/v2精确3catalog+3metadata同事务，input/after全图、six候选/typed pin/未变pin/新源审和当前QA-only proof均精确；旧c72/c98/v1/缺原件/少多输出/跨批拒绝，原未改Main postconditions任一失败真实整体回滚且保late-writer保护
- AND 原30场景及完整义务保留，产品incomplete且next_stage_allowed=false

### Requirement: BIM-008 原五类CLI负例与完整义务保留
工程工具 SHALL 按本变更设计中的实际源、执行原件及失败闭包核验；任何规范/登记/守卫通过均不授产品、Native、正式包或发布验收。

#### Scenario: five-cli-negative-categories-actually-execute
- GIVEN 已实际采用且独立审阅的R93父末态、其完整全源/产品图和本情形对应真实输入
- WHEN 执行 P02 对应原件消费与真实正反例
- THEN 原五类CLI负例必须真实positive0/violation1/rejected，缺任一执行原件拒绝
- AND 所有原失败保留，新run新nonce，产品仍incomplete且next_stage_allowed=false
