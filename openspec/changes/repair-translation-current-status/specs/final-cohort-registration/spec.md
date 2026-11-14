## ADDED Requirements

### Requirement: TCS-GOV-001 Exact current migration carrier
Current schema2 carrier pointers MUST bind the reviewed whole database implementation only after independent proof that migration SQL, DDL, versions, field relationships and original boundary obligations remain unchanged.

#### Scenario: reviewed-non-ddl-carrier-keeps-all-hash-rejections
- **GIVEN** R63已审database.rs新增只读projection事务与真实fail_task委派，迁移SQL和schema保持
- **WHEN** 独立审阅并核对schema2两chain及review顶层当前来源
- **THEN** 仅三个当前source hash可精确更新；破坏任一指针或SQL身份仍拒绝，历史v1/nestedreview与原失败不变

### Requirement: TCS-GOV-002 Complete final cohort registration
The schema2 lifecycle registry MUST select all original nine obligations, fourteen invariant links, SR6 checks and maintained cases for the newly registered final ID, while retaining every prior registration and rejecting missing checks.

#### Scenario: new-r63-final-retains-full-original-obligations
- **GIVEN** 已独审schema2合同仅追加新R63终验ID及精确projection依赖行
- **WHEN** 真实require_pin读取并对原primary、R62和R63运行空diff选择
- **THEN** 完整义务相同，缺native/component/runtime均拒绝；非法重复未知或未pin登记仍拒绝，不改旧WI、预算、oracle、设计PNG或debt

### Requirement: TCS-GOV-003 Truthful catalog and reviewed generation
The current implementation catalog and authority packet MUST describe exact integrated source changes and reviewed public contract deltas without replacing retained historical evidence or claiming product acceptance.

#### Scenario: actual-main-drift-and-public-rpc-delta-are-recorded
- **GIVEN** Main已有有效历史artifact与R63实际源码差分，新readonly RPC和DTO增量已审
- **WHEN** 独立核对逐条source quote与明确UTC日期并生成待审packet
- **THEN** 只更新实际源和13类明确授权派生路径，contract unchanged为false且附明细；不批量刷新历史/验收状态，生成和Root采用分离


治理生成器显式source_evidence_additions仅向已有module/component/capability row追加；配置每项必须有真实product闭包内safe path、准确line/quote/hash与独审理由，重复/未知row/越界/错误身份全部拒绝。新增引用及理由进入exact-delta和manifest供独审；原112引用修复与4显式remap分开保留，不自动搜全库或删原引用。
