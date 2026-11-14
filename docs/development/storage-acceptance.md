# 存储模块可执行验收契约

首版实现已包含生产schema1/index1迁移、真实Rust/SQLite边界探针和部分生产字段往返。PRE-14仅支持新库、已知身份同版本重开与配对恢复，以及拒绝未知/无版本/损坏来源；有限工程政策见[独立审阅与采纳](../../openspec/changes/implement-first-product/pre14-engineering-review.json)。这不批准新增字段设计或全产品发布。现已注册生产适配器 `rust-storage-complete-v1`，同次执行原生产测试、431字段映射、全部边界、22项模块验收及十条路线。实现已接线不表示验收通过；必须取得当前源码和当前正式候选包的完整执行回执。128条用例、153个`case@variant`计划仍保持原身份与状态。

## 唯一来源和自动一致性

| 对象 | 唯一来源 | 程序保证 |
|---|---|---|
| 表、字段与约束设计 | `docs/architecture/database/{profile,config,dictionary}.sql` | SQLite实际执行设计DDL并提取字段、主键、默认值、复合外键、索引、CHECK、触发器 |
| 版本冻结的期望结构 | `docs/architecture/database/schema-contract.json` | `storage_schema.py --check`逐项对照DDL；验收时只读对照真实候选库，不能自动刷新合同 |
| 字段边界与声明CHECK的探针 | `docs/architecture/database/field-boundaries.json` | `storage_boundaries.py --check`重新推导并实际运行探针，缺字段、删探针、改预期或未解决项均拒绝 |
| 模块验收子项、Owner和用例映射 | [storage-acceptance.json](storage-acceptance.json) | `storage_acceptance.py --check`检查全部22项、现有用例组合与前置；动态引用全部字段与探针身份 |
| 生产编译/运行命令 | `scripts/quality/storage_acceptance.py`的`STORAGE_ADAPTERS` | 只允许实现迭代在代码中注册的明确Rust目标，禁止JSON或回执自行提供命令 |
| 每次实际验收结果 | `.build/storage-acceptance/<run_id>/execution.json`及原始日志、DB快照 | 新建不可复用run目录，绑定本次源码、包、引擎、逐断言和候选库hash；按MA-16独立归档 |

字段身份从DDL与边界登记精确比对，全部探针ID和每字段`RUST-MAP:<database.table.field>`由程序展开。数据库验收JSON不复制数百个字段或上万个探针，避免两份清单漂移。边界探针数量随经评审的契约变化，以当次实际回执为准；不能把数量当作通过证据。

字段设计检查使用隔离投影，有意关闭外键和触发器以定位单字段及CHECK边界。Python绑定层的超出i64拒绝、NaN转NULL等行为会如实标注，不能证明Rust产品行为。生产Rust目标必须编译实际输入/DTO/存储类型映射，逐字段验证可空性、范围、无损转换，以及REAL的NaN/Infinity有限值策略、u64规范十进制、JSON结构与UTF8位置；同时在完整生产schema下另测关系、授权和业务事务。`INTEGER PRIMARY KEY`对NULL自动分配rowid的SQLite行为不能代替产品API的必填身份校验。

## 验收覆盖和职责

22个稳定子检查ID为`STORAGE-01`～`STORAGE-22`，标题、Owner、准确`case@variant`和证据要求保存在JSON中。它们细化既有用例，不新增产品功能或改变153组合。

| 覆盖 | 既有验收入口 | 设施与维护归属 |
|---|---|---|
| 新建双库、字段/Rust类型、完整关系 | TP-01、TP-12、TP-17、TP-27、CR-08-CONFIG、CR-12-RESOURCE | TE-04/08；AP/JA；MA-02/05 |
| 事务崩溃、WAL、备份、busy、只读、磁盘满 | TP-01/02/03 | TE-04；AP/PL；MA-05 |
| Keychain新增/撤销、取消与提交竞争 | TP-04/05/14/24、MC-12/13 | TE-06；PL/AI/AP；MA-05/10 |
| 缓存、人工修正、历史分页、保留与受控GC | TP-16/25、CR-08-HISTORY、CR-12-PERSIST、MC-20 | TE-04/06；AP；MA-02/05 |
| 配置封存、三态参数、幂等、档隔离 | TP-06/07/08/09/10/11、MC-01/08/11/12/21 | TE-04/06；AP；MA-02/05 |
| 真实词典只读、资源及跨库版本 | CR-12-RESOURCE、TP-19 | TE-08；JA/PL；MA-05 |
| 支持升级、拒绝降级、配对恢复 | TP-02/30、MC-16、CR-09-UPGRADE | PRE-14、TE-10/12；AP/PL/QA；MA-05/10/16 |

生产代码与测试应维护于已批准的`crates/app-infra/src/storage/{database,migrations,queries}/`、`crates/app-infra/tests/`和对应领域模块；首版实施已在这些路径提供实际生产实现与专项测试；完整适配器使用 `--lib --test storage_product_acceptance`，聚合复用原生产测试与专用support；逐字段实际值与来源由typed API写入并独立读取核对。门禁单测维护于`scripts/quality/test_storage_acceptance.py`。其临时名为`cargo`的进程和小SQLite库仅验证守卫确实执行两条命令、核验快照并拒绝伪证；正式注册表不包含它们，不能作为产品验收。

## 开发与发布阻断

`storage_requirement_gaps(root, metadata, changed_paths, profile)`为统一verify提供判定：

- 开发态`kind=tooling`可进行设计、契约与守卫校验。
- `kind=product`修改受管存储、字典、凭据、持久化领域、语言资源或相关测试路径，或声明涉及M01/M03/M04/M08/M10/M11/M16，必须提供已注册`product_storage_checks`。
- 发布态无条件要求真实存储adapter和已冻结PRE-14矩阵；只填写检查名称、计划勾选、旧PoC、设计DDL重建或hash回执均不能替代。
- 注册名称通过后，统一门禁仍须实际调用`run_product_storage`，不能把“名称存在”当作“已经运行”。真实runner缺失、编译/运行失败、无断言、ignored/skip、回执过时、字段/结构漂移、迁移路线缺失或版本链不符，均不放行。

发布还保留完整设计待审、历史债务、实际正式包153组合与其他真实adapter的独立要求。此存储验收不能单独签发首版发布批准，也不会把原计划CSV写成通过。

## 支持迁移矩阵

当前支持矩阵仅冻结profile/schema1与dictionary/index1，不提供旧PoC或v2迁移。路线和实际结果分别维护，未知/无版本/损坏库原件不变。后续支持范围变更仍须独立评审以下内容：

1. `approval_ref`：仓库内批准对象，明确受支持版本、失败恢复、旧PoC迁移选择和可恢复样本。
2. `fresh_versions`：profile与dictionary的新建正整数版本。
3. `migration_chain`：profile逐版本的`version/previous_version/source_path/source_sha256`，以及按真实生产顺序记录的`sql_sources[{path,sha256}]`与SQL迁移账本`migration_sha256`。Rust入口身份和SQL账本身份分别核验；链从0开始，末版本等于fresh profile版本。schema1账本按两份生产SQL的实际顺序和单个换行拼接计算，不能拿Rust文件hash代替。
4. `dictionary_versions`：以索引格式版本为键，固定`resource_version/manifest_sha256`。
5. 每条`routes`：稳定ID、已注册的`kind`（本轮reopen/compatibility_rejection/paired_recovery；未来upgrade/downgrade_rejection须另行评审）、数据库、明确`source_state`、来源/目标版本、冻结schema引用与期望accepted/rejected/error。目标版本为正整数；无版本或损坏来源的`from_version/source_schema`为null，不编造已支持旧结构；未知版本保留其实际版本号并拒绝。已知来源和目标schema绑定现存冻结合同，保留历史版本，不覆写当前合同冒充旧版。

两条`fresh_profile/fresh_dictionary`路线由程序固定补入，并与注册的新建版本绑定。每条实际路线另展开`MIGRATION-ROUTE:<id>`断言，只有回执ID列表不算覆盖。

每条路线必须产生独立的before/after文件；配对恢复另提供独立backup。禁止不同路线复用同一份fresh文件。程序核验每份文件的hash、只读完整性和实际结构，并读取数据版本：profile的`schema_migrations`实际末版本、previous链与迁移源hash；dictionary的`dictionary_meta.index_schema_version/resource_version/manifest_sha256`。accepted after按目标版本及target schema核对；rejected after按来源版本及source schema核对，同时before/after字节hash不变，target schema仍绑定被拒绝尝试的目标版本。paired recovery的after还必须与已校验backup逐字节相同。不能用“声明版本”代替实际库中的账本。

## 真实adapter和运行回执接口

实现迭代必须在代码中注册`adapter_version`、固定`sqlite_version/sqlite_source_id`、真实`migration_sources/test_sources`、`rust_test_target`及两条argv命令。两条命令均固定为同一`app-infra --lib --test storage_product_acceptance`目标并带`--locked`；编译命令带`--no-run`，执行命令带`-- --nocapture --test-threads=1`。封闭argv拒绝位置过滤、`--skip`、遗漏目标及任何其他参数。不得引用设计SQL作为生产迁移入口，也不得使用`--list/--ignored`替代执行。编译阶段不能提前生成运行回执。

执行器创建独立run目录和`required-checks.json`，通过`FP_STORAGE_RUN_ID/OUTPUT/RECEIPT/SOURCE_SHA256/ARTIFACT_SHA256/STARTED_AT_MS/INVENTORY/INVENTORY_SHA256`环境变量传给已注册目标。大量字段探针通过文件传递，避免环境变量长度限制。runner必须：

- 实际执行全部`STORAGE-*`、动态边界、每字段Rust映射和`MIGRATION-ROUTE:*`子检查；stdout逐项输出唯一`STORAGE_ASSERTION <id> <正整数断言数> PASS`。Rust总结必须非零passed、零failed、零ignored；禁止跳过。
- 将`product_storage_runtime_receipt`写入本轮`receipt.json`，含本次run_id、源码/包hash、adapter和SQLite版本/source_id、开始/结束毫秒时间、全部checks（id/status/assertions/skipped=0）、完整migration_routes及两个databases。
- migration_routes逐项提供database、source/target schema路径与hash、outcome、before/after/backup的相对路径和hash；实际文件均在本轮adapter目录内。两个databases固定profile/dictionary，并精确绑定对应fresh路线的after文件、hash、`open_mode=read_only_snapshot`及`origin=registered_production_migrations`。

程序捕获真实编译/执行日志并自行计算hash，不接受runner自行编造日志路径作为执行证明。候选库必须关闭并形成一致快照，无活跃WAL/SHM；比较前后hash相同。路径逃逸、符号链接、旧run、错误引擎、包/源码运行中变化、修改要求清单、缺路线、错误schema或错误迁移账本都失败。编译目标和迁移来源需在实现评审中核对，`origin`字符串本身不是生产真实性证明。

## 命令和当前结果含义

```bash
python3 scripts/quality/storage_acceptance.py --check
python3 -m unittest discover -s scripts/quality -p test_storage_acceptance.py -v
python3 scripts/quality/storage_acceptance.py --runtime --adapter <已注册目标> --artifact <最终包> --output .build/storage-acceptance
```

`--check`与独立`--runtime`均强制执行冻结schema对照和全字段边界校验；不能跳过总verify后绕过前置。runtime使用 `--adapter rust-storage-complete-v1`；缺当前包、来源不符、任一实际断言或独立库检查失败均非零拒绝。专项局部回执不等于完整同次验收。真实设施交付后每次运行另建证据，不回写计划定义；不能以放宽规则、删断言或登记假adapter解除阻断。

## 执行预算、真实包身份与失败清理

全量内置词典构建与逐行核验使用真实归档，开发实测约379秒；编译预算600秒、完整运行预算1800秒，均为代码登记的有界预算，不缩减条目或断言。命令在独立进程组执行；超时先TERM再KILL本组，保留已输出日志、阶段失败和最终execution.json，不能留下测试后代继续运行或将超时记成blocked。测试创建的有限磁盘映像只清理本run的确切挂载。

`package_identity.py`在执行前实际读取ZIP中的EXE、Info.plist、Core与语言锁，并调用系统codesign检查应用资源签名。源码身份写入签名保护的Resources/luna-source-identity.json；ZIP外层identity含最终签名EXE hash，外部build-manifest绑定ZIP与两份身份。只改外部JSON不能把旧包改名为当前源。该机制证明本机开发构建来源，不代替开发者证书、签名公证或发布审批。

缓存矩阵同时验证等价输入命中与版本/配置/语言资源/模型/凭据/上下文变化不误命中；显式重译跳过读缓存而幂等操作重放不重复请求。资源声明比较完整不可变身份，包含同session重入；错误版本、manifest、来源或索引实际拒绝并保留历史引用。全部结果仍写本次run，不回写原计划CSV。
