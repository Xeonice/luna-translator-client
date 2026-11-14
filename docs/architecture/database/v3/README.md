# HTTP 服务身份：schema 3

本目录是首个 HTTP 存储切片的独立设计合同。生产schema3切片已完成真实适配器同轮执行及非作者复核；结果仅绑定下述源码与运行记录，不代表当前整个客户端或后续schema4通过。只重建 `connections`、`credential_refs`，新增配置编码版本及不可变预设来源；不引入离线 EngineTarget、资源表或 OAuth grant。首版 13 协议、21 预设和 4 个本机非模型引擎的总目标没有缩减。

冻结的 [设计 SQL](003_http_service_identity.design.sql) SHA256 为 `813145f4923f28860ad2035abf5df8f5b9dbf68753b45510a2b794f80d3c43c2`。其窄HTTP方向已获独立DDL审查，生产迁移、字段和关系已由真实适配器执行并获非作者复核；实际账号和正式包验收仍分别记录。合同作者与最终源码审查者必须分开。

```text
database/
├── profile.sql、config.sql           原始独立设计；431 字段义务保留
├── v2/                              原有29字段及真实会话义务保留
└── v3/
    ├── 003_http_service_identity.design.sql  已审设计源
    ├── schema-contract.json                 双库完整484字段与所有结构
    ├── field-dictionary.json、.csv           24字段增量及生产者目标
    ├── boundary-fixtures.json               明示的非产品边界输入
    ├── field-boundaries.json                可重建校对的SQL探针义务
    ├── storage-acceptance.json              旧义务与本片新增准出要求
    └── storage-review.json                  审阅状态，不能自签产品
```

完整预期从原独立设计、已审 schema 2 oracle 和本目录冻结 SQL 推导，**不读取新生产 migration 来生成预期**。词典 schema 1 完全不变。旧 460 字段物理属性保留，新增 24 字段，总计 484；`connections.protocol/auth_mode`、`credential_refs.auth_kind` 三个旧枚举的合法域扩展单独登记，不宣称旧 CHECK 全部不变。

| 对象 | 新字段 | 责任及验证 |
|---|---|---|
| config_revisions | codec_version | SQL 1=原编码，2=HTTP 新编码；不可原地修改配置。默认1仅标识旧行，不能改写旧 seal。 |
| connections | binding schema、protocol contract、profile ID/revision、preset ID/revision | 精确 tuple、全空或完整的 nullable 组、来源复合 FK；无版本绑定的草稿不自动成为可执行连接。 |
| credential_refs | envelope_schema | NULL 只解释旧 raw APIKey；1 按可信 auth_kind 严格解码 typed Vault 封套；不可凭秘密内容猜格式。 |
| provider_preset_versions | 16字段 | builtin/user 来源不可变；关键身份独立列与复合 FK。来源不是另一份运行权威，不跟随 latest。 |

逐列类型、NULL、默认值和主键位置见[字段字典](field-dictionary.json)；全部 CHECK、索引、复合 FK 的列顺序/删除策略和触发器见[完整结构合同](schema-contract.json)。字段字典的生产者路径是本片明确目标，`pending_runtime_verification`是冻结字段合同形成时的标签；实际执行结论由独立回执解析，不修改预期来迁就运行。路径存在或编译成功不能自动提升它。

SQL 对目录 ID 与 SHA256 显式拒绝 NUL；只用 `length/GLOB` 不够，因为 SQLite 在部分文本操作中遇 NUL 会截断。正版本限定 u32 范围，STRICT 仍须分别测试错误类型、整数小数、NULL 和物理 i64/API u32 边界。Endpoint 在 SQL 中允许未完成草稿；HTTP(S)、userinfo、代理及导入字段白名单仍须 Rust 校验，不能把字段层通过理解成网络授权。

`codec_version` 不等于数据库 schema 版本。LegacyV1 保留原字段声明/Vec 顺序和字节含义；原库没有存储部分 Vec 的排序，不能对排序后的 load 结果重新 hash，再指责旧 seal 损坏。新 codec2 的 DTO/规范排序与实际保存重开必须单独验证：无序顶层实体按 ID，有序 engines/terms 保持 position。不得通过重生成旧 golden、修改旧 seal 或把未知 codec 回退 raw 来迁就实现。

## 分层检查与准出

1. `storage_schema_v3.py` 校对冻结设计 hash、原 v1/v2 合同及义务字节、完整 v3 形状、24字段 JSON/CSV 和复合关系。候选库只读比较，拒绝非空 WAL/SHM、符号链接、损坏与外键违反。
2. `storage_boundaries_v3.py` 在精确 CREATE TABLE 投影上执行字段 valid/type/NULL、NUL、版本、JSON、nullable 组和旧枚举扩展探针；整数另外区分 SQLite 驱动的物理 i64 越界与合同 u32 越界。字段投影移除触发器、关闭 FK，防不可变 UPDATE 或无关父键失败掩盖字段错误；另在完整独立设计库验证来源 FK、封存和不可变触发器。201项字段/组合加12项关系、共213项设计SQL/驱动探针是这一层的库存，不是生产者或迁移通过数。
3. 真实 Rust 存储适配器必须同一 run 保留原 22 项、431字段与 v2 的29字段/会话义务，再提供本片59个 delta marker：8项语义、24项 `RUST-MAP:`、24项 `V3-FIELD:` 和3项 `V3-LEGACY-ENUM:`。每个字段 marker 必须含真实非零 `valid/type/null` 子断言，不能借别字段或全局摘要顶替。
4. 编译/执行命令、源码闭包、nonce、run ID、数据库快照、备份及正式包身份由真实适配器外层核实。`validate_delta_receipt` 只检查语义库存，不认证执行来源。设计克隆、守卫 fixture、旧回执、只填 hash 或 descriptor 均不能授予产品通过。

三个旧枚举的原 probe ID 在 [准出登记](storage-acceptance.json) 中保留：原非法值仍在精确 v1/v2 fixture 上执行；v3 对已批准新值使用 typed 上下文验证，并证明未知 binding/kind/schema 在 Vault/网络前拒绝。不能删旧负例、修改原预期或让新 Store 假称版本2。

```sh
# 独立设计层；结果始终不授予产品验收
python3 scripts/quality/storage_schema_v3.py
python3 scripts/quality/storage_boundaries_v3.py
PYTHONPATH=scripts/quality python3 -m unittest test_storage_schema_v3 test_storage_boundaries_v3 test_storage_acceptance_v3
```

真实适配器入口为`storage_product_adapter.py --schema 3`。本切片最新有效同轮记录是`.build/current-client-execution-20261105/cp03/adapter-v3-runtime-02.json`，源码闭包`ebd97ccedfd7e86fb6f0f2fe54347174e73d95a9aac43980819c25df53f3b820`：66项库测试、210项集成测试、17组件、59个delta marker及484字段，均在真实编译/执行与同轮nonce下完成。补充进程审计验证实际命令、exit、时间和源码，拒绝只填写receipt。

非作者复核见`.build/current-client-execution-20261105/review/storage-gateway-1793885204256181000/review.json`：另跑52项Rust、58项守卫及19个恶意输出HTTP场景，复核46份封闭快照、原业务列/旧seal和真实旧reader拒schema3。首轮失败、非法输出及备份修复前证据原样保留。以上只覆盖此精确分片；之后源码变化必须新跑，不能继承为全客户端、schema4或产品通过。

## 迁移与认证的必需现场

升级前持独占租约，只读核验真实源版本、原迁移 hash、profile、完整 schema、FK 与 integrity；所有已有待升级库先生成独立核验的 SQLite 备份。专用迁移连接在事务外关闭 FK，在 IMMEDIATE 内 create/copy/drop/rename 与重建依赖，完整核对旧列/seal/head 后提交，再启用 FK 并读回1。失败不得发布 runtime；恢复使用核验备份，不改版本号假降级。

真实路线包含 fresh0→3、1/2→3、3重开、旧客户端拒绝3、迁移失败回滚、崩溃后的完整2或3、备份还原、readonly/busy/真实 ENOSPC，以及未来版本/hash/结构/FK/profile 损坏的只读拒绝。必须使用实际 Rust/bundled SQLite、原 writer 完整 fixture 和新 save→reopen 校验；内存设计库不证明这些路线。

生产 task/test/discovery 共用精确协议/profile/auth admission，先验证权威和可信 SQL metadata 再读取 Vault；异种活跃池成员明确拒绝，不能从所有 active 凭据中随机挑选。材料 kind/schema 错配、撤销、停用与迟到结果均须重新核验。Google service-account/refresh 材料可表达不等于授权层已经实现；本切片不得用 raw Key 冒充 access token，或把秘密塞进 networkMetadata。

当前正式包、实际翻译账号、四游戏和产品准出仍由各自新运行记录决定。本目录没有执行它们，也不提升 `next_stage_allowed`。
