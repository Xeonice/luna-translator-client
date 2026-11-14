# R61自然退出操作终态修复迭代卡

授权：用户要求完成既定整个稳定性重构；本片为R59真实失败的窄存储修复，保持原回归与新紧凑布局暂不实施的边界。

父基线FP1-20261027-R60，SHAd1b1d55f80e7254220c2c1e5109ff53db8045824d88c44588509d83d418c509b。本片前像必须在R60 Root精确采用后采集；本次准备的源码map为1890项，tree 3ddb8489697ff8ee1bdda3a5b8eddcbff74bc73cfca7cab0fe54504cc361639c。两份产品源原字节、父基线、旧debt与verify-refactor-stability-rc全部字节另存ignored preimage；不声称保存全部源字节。

追溯：NOP-001～005→FP-03/05/09/12→当前阅读/断开/诊断设计→M02/M03/M10/M14/M15，C15/C23/C33/C36。Owner：durable_operation_stop负责独立候选及真实分层测试，Root负责OpenSpec/集成/authority/实际验证；AP负责持久化语义，HK负责ACK边界，QA负责同包证据，独立regression_fix_code_review已审阅精确attempt02两个hash。并行协作者不独占代码库、不回退他人修改。

允许产品路径仅crates/app-infra/src/storage/database.rs与crates/app-infra/tests/application_runtime.rs；其余仅本工作项及verification列出的精确authority路径。禁止修改旧R59验证工作项、legacy-debt、测试oracle/600秒预算、PRD/批准PNG/token、Core/协议、DB schema或真实profile库。

前置：R60采用回执与当前源码精确一致；candidate before仍匹配Main；独立review02 SHA f4ea0b83b1615c466b38d858b6bccfa491af0af6537bfa696128566c1566055d；正式source-snapshot取本轮真实前像而非复用R59隔离map。

验证：规范/映射/范围、编译及完整app-infra regression；实际rust-storage-complete-v2；DB字段/关系/边界守卫；原v4视觉执行保留结果；npm run verify -- --change repair-natural-exit-operation-terminal。精确authority packet必须另独审后Root采用。隔离四新用例1/1、全套41/41、两旧代码预期红例只证明受控层，不提升Main/正式包/游戏通过。

完成条件：适用源码/存储门禁实跑；新同源签名包、新nonce、同包四游戏自然退出全部operations终态/Host/ACK/租约实测。R59原失败不可更改。尚未执行not_run，外部条件缺失blocked；无需重启/缩放/改尺寸来准备文档。

## R60完整门禁反馈后的精确工具范围补充

R60本轮真实门禁62/66失败；storage-acceptance-tests与storage-acceptance-v2-tests均证明现有test_sources未登记R60已编译的preview_identity.rs，原visual v4/lifecycle失败另保留。本片新增允许工具路径scripts/quality/storage_acceptance.py，只在v1 base列表追加该实际输入一次，v2按现有继承方式同时获得；不改Cargo命令、source闭包算法、断言、schema或adapter政策。两产品路径仍只有database.rs和application_runtime.rs；工具不是第三个产品文件。候选before 12c5cd828b883aecb105f1b96b4aee825caaa2ac563d9231f4478b3b16948f29 → after 182078a78d740d582d9a5a8f39e4637a5e14e8c3fd35b5efdc7f1042226c28a6，须独立审阅后Root集成。

隔离根已实际调用storage_requirement_gaps：R60+旧登记拒绝编译闭包，R60+候选登记无缺口。R61两个产品补丁叠加后仍被旧迁移来源全文件hash两项与schema2独审hash一项阻断；此补充没有授权自动刷新这些引用。必须另记录精确影响、独立审阅迁移SQL未改和窄代码补丁后，补齐相应authority范围；否则完整存储仍blocked。旧R60采用记录不改。

## 非DDL源码迁移carrier的两份精确文档范围

Root已要求按现有R32先例独立核对迁移carrier（docs/development/iterations/FP1-20261021-runtime-followup/plan.md:61）。允许额外文档仅docs/architecture/database/v2/storage-acceptance.json与docs/architecture/database/v2/storage-review.json；前者只能在既有migration_chain[0]与[1]的source_sha256将已独审的database.rs完整身份与当前源码对应，后者仅同步production_migration_source_sha256。必须先由独立context_state核对SQL/DDL和窄非DDL补丁，保留其回执、精确3字段diff和旧字节，再由Root精确采用；不是删除检查或未审查刷新hash。

docs/development/storage-acceptance.json的历史v1 registry与其prior_registry_sha256均不改；SQL、schema、迁移route、旧fixture及所有阈值不改。原R60 snapshot已包含这两文档旧hash，原字节在追加preimage/original保存，初始前像receipt保持原样。独审与采用未执行时仍blocked。
