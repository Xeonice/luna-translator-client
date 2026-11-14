# 服务图存储合同：schema4

本目录是**当前R4设计的独立字段与结构Oracle，作者27项守卫及2556组SQL探针已通过，等待本Oracle的非作者审阅**。SQL设计获准开始生产实现，不等于迁移、Rust生产者、真实资源、正式包或产品通过。[作者登记](storage-review.json)明确分开这些状态。本目录不读取生产004来生成任何预期。

冻结设计：[004_service_graph.design.sql](004_service_graph.design.sql)，SHA256 `09eab3339ed56f535f72cc95d6d12cab069c73d6cd47a07197dd69f561351d55`。结构由原独立profile设计、v2设计、冻结003设计，再执行本设计推导；词典库完全保持schema1。全量为profile **81表734列**、词典32列，总766字段；旧431＋29＋24=484字段义务保留，本片新增282字段、28张表，涉及35张表。

```text
database/v4/
├── 004_service_graph.design.sql   原冻结SQL；本作者不修改
├── schema-contract.json           全量字段/STRICT/索引/列顺序/FK/触发器/视图token
├── field-dictionary.json          282字段、逐列CHECK/FK、准确producer目标和待实测状态
├── field-dictionary.csv           同一字典，逐项精确复算
├── boundary-fixtures.json         明示synthetic，每字段非NULL正向控制及合法联合变体
├── field-boundaries.json          逐字段、nullable组合、89逐FK及语义探针库存
├── storage-acceptance.json        累积旧义务、新迁移路线及实际运行marker准入
├── storage-review.json            作者自检/独审/生产迁移/产品分开
└── README.md
scripts/quality/
├── storage_schema_v4.py           独立schema/字典/只读候选比较
├── storage_boundaries_v4.py       字段、单FK隔离、完整设计语义执行
├── test_storage_schema_v4.py
├── test_storage_boundaries_v4.py
└── test_storage_acceptance_v4.py
```

## 字段和关系的职责

| 领域 | 真实字段及职责 | producer目标 |
|---|---|---|
| 服务图 | codec3、用户/激活revision角色、用途、HTTP options/origin、离线config、服务默认/override、主项与对照顺序 | `queries/service_graph.rs` |
| 精确选择 | 来源版本、source/effective真实FK、稳定control身份、选择计数、只动applied而不动draft | `queries/service_selection.rs` |
| Google授权 | 固定公开client、owned grant、授权epoch与refresh材料游标、SA/OAuth真实租约来源 | `queries/google_authorization.rs` |
| 本机资源 | runtime/ABI、环境/安装、本机授权、文件或Apple观察、直接语言对、明确route、完整闭包seal | `queries/offline_services.rs` |
| 实际执行 | task初始目标、显式fallback、attempt实际目标/租约、cache origin attempt | `queries/tasks.rs`及以上授权/资源模块 |

所有目标路径以`crates/app-infra/src/storage/`为前缀。字典中的`pending_runtime_verification`是义务状态，不能因文件出现或代码编译自动变绿。完整关系见字典：35表89个FK逐一登记；69个新增/改变触发器、1个执行权威视图另有实际Rust义务ID，不以结构token存在代替实际路径测试。

原字段唯一物理可空性放宽是`binding_engines.model_id`，由新HTTP/Offline XOR约束替代原单HTTP必填；不能虚造空模型。另明确扩展旧codec、auth与preset认证域，旧非法值仍在原v1–3 fixture执行。旧opaque ID不追加ASCII160限制，不重命名、不重算原seal。R2明确：新逻辑服务去重只适用于实际codec3且对应binding语义3；原codec1/2及codec3内LegacyV2的合法启用／禁用slots保持。TypedV3的禁用项、派生alias和主／对照仍不得重复，混合语义在各binding分别判断。原R1设计/作者回执及真实Rust旧slot失败保留，新SQL经独审和Root显式采用后才覆盖本目录；不将R1重新标成通过。

R4仅为旧版配置里没有Model的Connection建立内部来源证明；执行目标仍只有HTTP／Offline，不新增公开选择资格或虚造Model。来源必须是直接、封存、未GC、seal/hash一致的旧codec1/2，保留原稳定停止闸；自己引用、codec3证明链、伪造来源及带Model的连接均拒绝。有效版本在证明后补Model也不得封存。HTTP多Model共享连接必须汇聚到同一来源／控制身份，connection-only与HTTP证明不能抢占同一有效连接。GC先标有效子版本、再清除子证明，最后才允许删除旧父Connection。SQL只核验关系和元数据，private Rust仍须核对旧Connection全字段，不把FK视为授权。

FK编号有固定完整身份：R3原87条的`id/table/foreign_key/prior_relationship`逐项不变，子列序、父列序、父表、ON DELETE／UPDATE和MATCH均绑定；activation原0～7保留，新增直接Connection FK仅追加8／9。禁止按新SQLite排序重排；原737项Rust义务只增加这2项及`config_gc_marks.config_gc_unreferenced`触发器，共740项。真实FK回执必须携带从实际候选独立读取的完整`relationship`，保持全部ID和正计数仍不能掩盖关系重指。

## 独立检查怎样分层

1. **完整形状**：校验冻结设计hash和旧Oracle字节，再比较全部列类型/NULL/default/PK/STRICT、索引、复合FK列序及删除策略、触发器和视图token。候选仅用只读immutable连接，拒绝符号链接、非空WAL/SHM、损坏或FK违反；前后字节必须不变。设计克隆只能当守卫fixture。
2. **字段投影**：使用独立冻结SQL的精确CREATE TABLE，去触发器、关闭FK，避免不可变UPDATE或无关父键先报错，掩盖字段CHECK。每个新字段都有非NULL正向、错误物理类型、NULL，加NUL、UTF8大小、JSON、u32与物理i64边界。NULL单字段和成组mask分别执行，不能把“列可空”误读为“半个引用合法”。
3. **逐FK隔离**：每个真实FK独立保留child的CHECK、精确列序/删除行为，只投影目标parent键；执行匹配正向、无parent、每个parent列的wrong-owner、删除策略。其他FK不会抢先拒绝。它证明该关系的SQL行为，不能代替完整真实父对象的producer或运行授权。
4. **完整设计语义**：正常启用FK与触发器，验证refresh材料归属、grant CAS/初始状态、稳定control别名、源保留/事务回滚、TypedV3禁止旧策略影子、实际安装环境/引擎/runtime、显式pivot同runtime、资源/route seal及GC、13协议payload carrier边界。异常原件保留。
5. **真实Rust与外层adapter**：同一个实际compile/run保留旧431＋29＋24义务，新282字段逐个`RUST-MAP:`和`V4-FIELD:`，以及89 FK、69触发器、视图和明确业务marker。每字段必须真实非零`valid/type/null`子断言，89个FK回执必须带从真实候选独立读取的完整`relationship`。run/nonce、源码闭包、真实host/旧reader、快照/备份、进程退出及实际Rust执行由Root注册的外层验证；单填marker、Python设计SQL、旧回执都不能满足此层。

### SQL允许不等于客户端允许

预期结果只由冻结独立设计决定，不能从待验候选学习“通过”。部分JSON内层类型、某些无界opaque ID及业务引用观察属于typed Rust或实际authority层，SQL只做carrier。`field-boundaries.json`对SQL实际accept和必须另做的Rust检查分别登记；不能把“json_valid”或“有一行reference”称为合法执行。尤其：

- HTTP options按固定`protocol`tag，版本只在可信schema列；origin按固定kind及真实preset键，禁止任意header/token URL/脚本。R3允许Custom和Preset可选`site`文本；有值时不得是null／布尔／数字／对象／数组或含NUL；缺省省略仍保持旧字节，未知key继续拒绝。这是非秘密来源标识，不授服务或用户授权。Azure的resourceType及DeepL的plan无默认：codec允许缺失以保存未完成草稿；资格与发送前必须要求显式选择，不能暗选第一个。JSON carrier或草稿保存成功不代表已配置、可选或可运行。
- TypedV3的旧context/schedule必须为`{}`，新override唯一；LegacyV2保原字节与原语义。历史模式只有已批准的`original / translation / both`。
- SA使用真实pool/member/credential/lifecycle；用户OAuth用真实owned grant/epoch/minted材料，不能互造grant。SQL不存access token、私钥或refresh正文。
- 离线不造Connection/Model/APIKey，未安装、未验证runtime或未授权观察不变成可选。pivot必须显式同意且同runtime，不能自动下载或转云。

## 作者自检与真实准出

```sh
python3 scripts/quality/storage_schema_v4.py
python3 scripts/quality/storage_boundaries_v4.py
PYTHONPATH=scripts/quality python3 -m unittest test_storage_schema_v4 test_storage_boundaries_v4 test_storage_acceptance_v4
```

R3原22项守卫、2257组SQL、737项Rust库存与完整原件已先封存，旧回执不改标签当R4通过。R4原2094条字段／NULL／旧域探针的完整控制与预期不变；原87条FK完整payload与ID相等；76组原语义保留。另增256组connection-only的八字段NULL组合、41组完整设计语义、2组直接FK，并增加5项实际守卫。首轮27项及2556组已执行，但主动突变审计发现后加Model的负例可被旧template-purpose规则先拒；现已补合法purpose以隔离新seal分支，并新增精确移除该分支的反证。该定向守卫及修正后的完整27项、2556组均已实跑通过，14个输入在最终执行期间稳定；首轮绿色不作为最终依据。本轮首次ordinal红例证明“所有旧ID仍在但6项换义”；源码已以原87条完整payload固定hash与新2项显式追加修正。所有原始失败、修复与新回执均保留在[本轮交接](../../../../.build/current-client-execution-20261105/schema4-oracle-r4/handoff.json)，不会伪造mapper或运行marker。Root精确采用的R4 SQL已由另一非作者准入，本Oracle及真实Rust740项须另审／另跑。这些命令只验证本独立合同及其负例；不授产品验收。真实路线另要求0/1/2/3→4、重开4、真正Store3/Store2拒4且原库字节不变；迁移前只读预检及单文件备份、旧行/seal/head保真、同host失败回滚、外部kill原子性、只读/busy/物理ENOSPC、GC/CAS/字段及authority。001–003原SQL/ledger/hash和旧codec golden必须保留，不能刷新旧违规hash绕过。

正式服务账号/应用OAuth资料、Apple语言资源、Argos完整安装、合法J北京/金山DLL仍是各自真实运行的外部前置。设计和作者自检不替代它们，也不替代整页视觉、正式签名包与四游戏回归。产品`incomplete`、`next_stage_allowed=false`。
