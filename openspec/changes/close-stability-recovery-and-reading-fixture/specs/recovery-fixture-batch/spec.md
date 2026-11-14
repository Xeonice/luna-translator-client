# Recovery and reading fixture batch

## ADDED Requirements

### Requirement: SRB-001

系统 MUST 满足组合工作项的精确身份及检查边界。

#### Scenario: only-exact-reviewed-source-preimages-enter-the-batch

- GIVEN 实际采用的 R63 Main 前像及两个冻结 leaf
- WHEN Root 准备组合候选并审阅其影响
- THEN 组合只能消费两个冻结 manifest 的七份源；先核 Main R63 前像，再复制 exact after 字节。两 leaf 的十五份 WI、source-snapshot 与 freeze 留在外部冻结证据，不重写其内容；当前 guard 禁止跨 change allowed_paths，因此本组合只有自有 WI 进入候选源码。

### Requirement: SRB-002

系统 MUST 满足组合工作项的精确身份及检查边界。

#### Scenario: bounded-native-recovery-keeps-identity-and-budget-authority

- GIVEN 实际采用的 R63 Main 前像及两个冻结 leaf
- WHEN Root 准备组合候选并审阅其影响
- THEN Native 实现沿用已审六源：区分 source 点单位与输出像素，typed refusal 本身不授权保留，只有同 attempt 完整 post proof 与消费时 TCC/epoch 校验才可保留；元数据变化触发有界恢复。private collector 3→4，旧3拒绝；Core ABI1、Hook JSONL2 与公共 DTO 保持。

### Requirement: SRB-003

系统 MUST 满足组合工作项的精确身份及检查边界。

#### Scenario: reading-capture-consumes-independent-projection-without-replacing-oracles

- GIVEN 实际采用的 R63 Main 前像及两个冻结 leaf
- WHEN Root 准备组合候选并审阅其影响
- THEN QA capture 从独立完整输入构造明确 projection，并等待实际查询与可见状态；不从生产输出反推 oracle，不修改批准 PNG/token、像素阈值、原生布局或任何产品运行结果。原 top225→0 是 leaf 隔离证据，组合后的当前浏览器运行须重新执行。

### Requirement: SRB-004

系统 MUST 满足组合工作项的精确身份及检查边界。

#### Scenario: lifecycle-checks-and-final-r63-remain-complete-and-separate

- GIVEN 实际采用的 R63 Main 前像及两个冻结 leaf
- WHEN Root 准备组合候选并审阅其影响
- THEN 组合明确声明全部 SR6 工程、Hook、infra、reading、target、visual、native checks；实际 diff selector 不得漏检查。现有 schema2 注册 verify-refactor-stability-r63 负责另行同包 final cohort；九义务、十四 links、五十步骤与预算原值保持。真实 SCK、四游戏、新同源包和最终验收均为 not_run。
