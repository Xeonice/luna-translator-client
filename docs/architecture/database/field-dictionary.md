# 首版数据库字段字典

由 `scripts/quality/database_design.py --write` 从设计 DDL 与 SQLite PRAGMA 确定性生成。仅为架构设计，不是已安装迁移或产品实测。

共 2 类数据库、53 张表、431 个字段。`profile` 每配置档独立可写；`dictionary` 为随包只读辞书。SQLite 隐式主键/唯一索引也在目录内。

## profile

### binding_engines

用途的有序模型分配；翻译可多引擎而学习用途仅单引擎。Owner：AP；范围：FP-06, FP-08, FP-09；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 所属不可变整体配置版本 |
| `binding_id` | TEXT | 否 | 2 | — | 同版本用途绑定ID |
| `engine_id` | TEXT | 否 | 3 | — | 用途内跨版本稳定的引擎项ID |
| `purpose` | TEXT | 否 | — | — | 冗余用途并以复合外键保证与父用途一致 |
| `model_id` | TEXT | 否 | — | — | 同版本模型配置ID |
| `position` | INTEGER | 否 | — | — | 结果显示和调度顺序，按用途唯一 |
| `enabled` | INTEGER | 否 | — | 1 | 该分配项启用意图；一个翻译引擎失败不关闭其他项 |
| `template_id` | TEXT | 是 | — | — | 引擎级用途模板；为空继续继承 |

外键：

- `(revision_id, template_id, purpose) → templates(revision_id, template_id, purpose)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(revision_id, model_id) → models(revision_id, model_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(revision_id, binding_id, purpose) → purpose_bindings(revision_id, binding_id, purpose)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `binding_engines_by_model (revision_id, model_id)`；origin=c。

```sql
CREATE INDEX binding_engines_by_model ON binding_engines(revision_id, model_id);
```
- `binding_engines_one_study_engine (revision_id, binding_id)`；UNIQUE; 部分索引; origin=c。

```sql
CREATE UNIQUE INDEX binding_engines_one_study_engine ON binding_engines(revision_id, binding_id) WHERE purpose <> 'translation';
```
- `sqlite_autoindex_binding_engines_1 (revision_id, binding_id, engine_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_binding_engines_2 (revision_id, binding_id, position)`；UNIQUE; origin=u。
- `sqlite_autoindex_binding_engines_3 (revision_id, binding_id, engine_id, purpose)`；UNIQUE; origin=u。

触发器：`binding_engines_no_update`、`binding_engines_sealed_delete`、`binding_engines_sealed_insert`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE binding_engines (
    revision_id TEXT NOT NULL, -- 所属不可变整体配置版本
    binding_id TEXT NOT NULL, -- 同版本用途绑定ID
    engine_id TEXT NOT NULL CHECK (length(engine_id) > 0), -- 用途内跨版本稳定的引擎项ID
    purpose TEXT NOT NULL CHECK (purpose IN ('translation','grammar','context_word')), -- 冗余用途并以复合外键保证与父用途一致
    model_id TEXT NOT NULL, -- 同版本模型配置ID
    position INTEGER NOT NULL CHECK (position >= 0), -- 结果显示和调度顺序，按用途唯一
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)), -- 该分配项启用意图；一个翻译引擎失败不关闭其他项
    template_id TEXT, -- 引擎级用途模板；为空继续继承
    PRIMARY KEY (revision_id, binding_id, engine_id),
    UNIQUE (revision_id, binding_id, position),
    UNIQUE (revision_id, binding_id, engine_id, purpose),
    FOREIGN KEY (revision_id, binding_id, purpose) REFERENCES purpose_bindings(revision_id, binding_id, purpose) ON DELETE RESTRICT,
    FOREIGN KEY (revision_id, model_id) REFERENCES models(revision_id, model_id) ON DELETE RESTRICT,
    FOREIGN KEY (revision_id, template_id, purpose) REFERENCES templates(revision_id, template_id, purpose) ON DELETE RESTRICT
) STRICT;
```

### binding_parameters

用途层参数三态覆盖与来源追溯。Owner：AP；范围：FP-06；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 所属不可变整体配置版本 |
| `binding_id` | TEXT | 否 | 2 | — | 同版本全局或游戏用途绑定ID |
| `parameter_key` | TEXT | 否 | 3 | — | 适配器认可的参数名 |
| `mode` | TEXT | 否 | — | — | 继承上一层、显式值或阻断继承并省略 |
| `value_json` | TEXT | 是 | — | — | set保存实际JSON值；不是用SQL NULL表示发送null |

外键：

- `(revision_id, binding_id) → purpose_bindings(revision_id, binding_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_binding_parameters_1 (revision_id, binding_id, parameter_key)`；UNIQUE; origin=pk。

触发器：`binding_parameters_no_update`、`binding_parameters_sealed_delete`、`binding_parameters_sealed_insert`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE binding_parameters (
    revision_id TEXT NOT NULL, -- 所属不可变整体配置版本
    binding_id TEXT NOT NULL, -- 同版本全局或游戏用途绑定ID
    parameter_key TEXT NOT NULL CHECK (length(parameter_key) > 0), -- 适配器认可的参数名
    mode TEXT NOT NULL CHECK (mode IN ('inherit','set','omit')), -- 继承上一层、显式值或阻断继承并省略
    value_json TEXT CHECK (instr(value_json,char(0))=0 AND (value_json IS NULL OR json_valid(value_json))), -- set保存实际JSON值；不是用SQL NULL表示发送null
    PRIMARY KEY (revision_id, binding_id, parameter_key),
    FOREIGN KEY (revision_id, binding_id) REFERENCES purpose_bindings(revision_id, binding_id) ON DELETE RESTRICT,
    CHECK ((mode = 'set' AND value_json IS NOT NULL) OR (mode IN ('inherit','omit') AND value_json IS NULL))
) STRICT;
```

### cache_entries

按实际请求/资源语义命中的派生缓存，人工修正不作为机器缓存写回。Owner：AI；范围：FP-06, FP-07, FP-08, FP-09；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `cache_key` | TEXT | 否 | 1 | — | 规范缓存键 |
| `purpose` | TEXT | 否 | — | — | 用途分区 |
| `request_semantics_sha256` | TEXT | 否 | — | — | 输入/版本/用途实际语义 |
| `resource_semantics_sha256` | TEXT | 否 | — | — | 模型/模板/术语/词法/辞书等相关语义 |
| `result_json` | TEXT | 否 | — | — | 已校验结果与来源元数据，无秘密 |
| `result_sha256` | TEXT | 否 | — | — | 结果校验摘要 |
| `byte_size` | INTEGER | 否 | — | — | 有界容量统计 |
| `created_at_ms` | INTEGER | 否 | — | — | 生成时间 |
| `expires_at_ms` | INTEGER | 否 | — | — | 失效时间 |
| `last_accessed_at_ms` | INTEGER | 否 | — | — | 最近命中时间 |

外键：

无。

索引：

- `ix_cache_expiry (expires_at_ms, cache_key)`；origin=c。

```sql
CREATE INDEX ix_cache_expiry ON cache_entries(expires_at_ms,cache_key);
```
- `ix_cache_lru (last_accessed_at_ms, cache_key)`；origin=c。

```sql
CREATE INDEX ix_cache_lru ON cache_entries(last_accessed_at_ms,cache_key);
```
- `sqlite_autoindex_cache_entries_1 (cache_key)`；UNIQUE; origin=pk。

触发器：无。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE cache_entries (
    cache_key TEXT NOT NULL CHECK(instr(cache_key,char(0))=0 AND (cache_key IS NULL OR (length(cache_key)=64 AND cache_key NOT GLOB '*[^0-9a-f]*'))), -- 规范缓存键
    purpose TEXT NOT NULL CHECK(purpose IN ('translation','grammar','context_word','lexical','dictionary')), -- 用途分区
    request_semantics_sha256 TEXT NOT NULL CHECK(instr(request_semantics_sha256,char(0))=0 AND (request_semantics_sha256 IS NULL OR (length(request_semantics_sha256)=64 AND request_semantics_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 输入/版本/用途实际语义
    resource_semantics_sha256 TEXT NOT NULL CHECK(instr(resource_semantics_sha256,char(0))=0 AND (resource_semantics_sha256 IS NULL OR (length(resource_semantics_sha256)=64 AND resource_semantics_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 模型/模板/术语/词法/辞书等相关语义
    result_json TEXT NOT NULL CHECK(instr(result_json,char(0))=0 AND (json_valid(result_json) AND json_type(result_json)='object')), -- 已校验结果与来源元数据，无秘密
    result_sha256 TEXT NOT NULL CHECK(instr(result_sha256,char(0))=0 AND (result_sha256 IS NULL OR (length(result_sha256)=64 AND result_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 结果校验摘要
    byte_size INTEGER NOT NULL CHECK(byte_size>=0), -- 有界容量统计
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms IS NULL OR created_at_ms>=0), -- 生成时间
    expires_at_ms INTEGER NOT NULL CHECK(expires_at_ms IS NULL OR expires_at_ms>=0), -- 失效时间
    last_accessed_at_ms INTEGER NOT NULL CHECK(last_accessed_at_ms IS NULL OR last_accessed_at_ms>=0), -- 最近命中时间
    PRIMARY KEY(cache_key),
    CHECK(expires_at_ms>=created_at_ms)
) STRICT;
```

### capture_sources

完整候选文本流身份，不按TextOutW名字/固定行号选流。Owner：HK；范围：FP-03, FP-04, FP-12；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `source_id` | TEXT | 否 | 1 | — | 本会话内流ID |
| `session_id` | TEXT | 否 | — | — | 所属Hook会话 |
| `host_instance_id` | TEXT | 否 | — | — | Host实例代次 |
| `hook_address_u64` | TEXT | 否 | — | — | Hook地址规范十进制uint64 |
| `context_u64` | TEXT | 否 | — | — | context全宽值 |
| `subcontext_u64` | TEXT | 否 | — | — | subcontext全宽值 |
| `hook_code` | TEXT | 否 | — | — | 完整Hook code快照 |
| `hook_name` | TEXT | 否 | — | — | 展示名称，非唯一身份 |
| `encoding` | TEXT | 否 | — | — | 实际编码/代码页声明 |
| `stream_identity_sha256` | TEXT | 否 | — | — | 完整ThreadParam/Host身份规范摘要 |
| `identity_extensions_json` | TEXT | 否 | — | — | 有版本的附加身份字段，不能替代上述核心字段 |
| `discovered_at_ms` | INTEGER | 否 | — | — | 首次出现时间 |

外键：

- `(session_id) → sessions(session_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_capture_sources_1 (source_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_capture_sources_2 (source_id, session_id)`；UNIQUE; origin=u。
- `sqlite_autoindex_capture_sources_3 (session_id, stream_identity_sha256)`；UNIQUE; origin=u。

触发器：`capture_source_requires_hook`、`immutable_capture_sources`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE capture_sources (
    source_id TEXT NOT NULL PRIMARY KEY, -- 本会话内流ID
    session_id TEXT NOT NULL, -- 所属Hook会话
    host_instance_id TEXT NOT NULL, -- Host实例代次
    hook_address_u64 TEXT NOT NULL CHECK(instr(hook_address_u64,char(0))=0 AND (length(hook_address_u64) BETWEEN 1 AND 20 AND hook_address_u64 NOT GLOB '*[^0-9]*' AND (length(hook_address_u64)=1 OR substr(hook_address_u64,1,1)<>'0') AND (length(hook_address_u64)<20 OR hook_address_u64<='18446744073709551615'))), -- Hook地址规范十进制uint64
    context_u64 TEXT NOT NULL CHECK(instr(context_u64,char(0))=0 AND (length(context_u64) BETWEEN 1 AND 20 AND context_u64 NOT GLOB '*[^0-9]*' AND (length(context_u64)=1 OR substr(context_u64,1,1)<>'0') AND (length(context_u64)<20 OR context_u64<='18446744073709551615'))), -- context全宽值
    subcontext_u64 TEXT NOT NULL CHECK(instr(subcontext_u64,char(0))=0 AND (length(subcontext_u64) BETWEEN 1 AND 20 AND subcontext_u64 NOT GLOB '*[^0-9]*' AND (length(subcontext_u64)=1 OR substr(subcontext_u64,1,1)<>'0') AND (length(subcontext_u64)<20 OR subcontext_u64<='18446744073709551615'))), -- subcontext全宽值
    hook_code TEXT NOT NULL, -- 完整Hook code快照
    hook_name TEXT NOT NULL, -- 展示名称，非唯一身份
    encoding TEXT NOT NULL, -- 实际编码/代码页声明
    stream_identity_sha256 TEXT NOT NULL CHECK(instr(stream_identity_sha256,char(0))=0 AND (stream_identity_sha256 IS NULL OR (length(stream_identity_sha256)=64 AND stream_identity_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 完整ThreadParam/Host身份规范摘要
    identity_extensions_json TEXT NOT NULL CHECK(instr(identity_extensions_json,char(0))=0 AND (json_valid(identity_extensions_json) AND json_type(identity_extensions_json)='object')), -- 有版本的附加身份字段，不能替代上述核心字段
    discovered_at_ms INTEGER NOT NULL CHECK(discovered_at_ms IS NULL OR discovered_at_ms>=0), -- 首次出现时间
    FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE RESTRICT,
    UNIQUE(source_id,session_id),
    UNIQUE(session_id,stream_identity_sha256)
) STRICT;
```

### config_gc_marks

不再被使用版本的受控清理墓碑；保留头和封存。Owner：AP；范围：FP-09, FP-11；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 已验证无外部引用的封存版本 |
| `operation_id` | TEXT | 否 | — | — | 与清理同事务提交的操作回执 |
| `requested_at_ms` | INTEGER | 否 | — | — | 授权保留策略执行时间 |

外键：

- `(operation_id) → operation_receipts(operation_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(revision_id) → config_revision_seals(revision_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_config_gc_marks_1 (revision_id)`；UNIQUE; origin=pk。

触发器：`config_gc_marks_no_delete`、`config_gc_marks_no_update`、`config_gc_unreferenced`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE config_gc_marks (
    revision_id TEXT NOT NULL PRIMARY KEY REFERENCES config_revision_seals(revision_id) ON DELETE RESTRICT, -- 已验证无外部引用的封存版本
    operation_id TEXT NOT NULL REFERENCES operation_receipts(operation_id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED, -- 与清理同事务提交的操作回执
    requested_at_ms INTEGER NOT NULL CHECK(requested_at_ms>=0) -- 授权保留策略执行时间
) STRICT;
```

### config_heads

草稿与已应用版本的独立可CAS更新指针。Owner：AP；范围：FP-06, FP-08, FP-11；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `head_kind` | TEXT | 否 | 1 | — | 指针种类；保存草稿不移动applied |
| `revision_id` | TEXT | 否 | — | — | 当前指向的整体配置版本 |
| `head_revision` | INTEGER | 否 | — | — | 乐观并发版本；更新须带expected_revision并原子递增 |
| `updated_at_ms` | INTEGER | 否 | — | — | 指针最后更新时间，UTC毫秒 |

外键：

- `(revision_id) → config_revisions(revision_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_config_heads_1 (head_kind)`；UNIQUE; origin=pk。

触发器：`config_heads_requires_seal_insert`、`config_heads_requires_seal_update`、`config_heads_revision_monotonic`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE config_heads (
    head_kind TEXT NOT NULL PRIMARY KEY CHECK (head_kind IN ('draft','applied')), -- 指针种类；保存草稿不移动applied
    revision_id TEXT NOT NULL REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 当前指向的整体配置版本
    head_revision INTEGER NOT NULL CHECK (head_revision >= 0), -- 乐观并发版本；更新须带expected_revision并原子递增
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= 0) -- 指针最后更新时间，UTC毫秒
) STRICT;
```

### config_revision_seals

完整配置装配后的不可撤销封存凭证。Owner：AP；范围：FP-06, FP-08, FP-11；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 被封存的整体配置版本 |
| `content_hash` | TEXT | 否 | — | — | 与头记录一致；实际规范化内容摘要由Rust计算 |
| `sealed_at_ms` | INTEGER | 否 | — | — | 同保存事务的封存时间 |

外键：

- `(revision_id) → config_revisions(revision_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_config_revision_seals_1 (revision_id)`；UNIQUE; origin=pk。

触发器：`config_revision_seals_no_delete`、`config_revision_seals_no_update`、`config_seal_identity`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE config_revision_seals (
    revision_id TEXT NOT NULL PRIMARY KEY REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 被封存的整体配置版本
    content_hash TEXT NOT NULL CHECK(instr(content_hash,char(0))=0 AND (length(content_hash)=64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 与头记录一致；实际规范化内容摘要由Rust计算
    sealed_at_ms INTEGER NOT NULL CHECK(sealed_at_ms>=0) -- 同保存事务的封存时间
) STRICT;
```

### config_revisions

配置档内不可变整体配置快照与本地校验摘要。Owner：AP；范围：FP-06, FP-08, FP-11；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 不可变整体配置版本ID，与请求语义指纹分开 |
| `parent_revision_id` | TEXT | 是 | — | — | 来源版本；首个版本允许为空 |
| `content_hash` | TEXT | 否 | — | — | 规范化非秘密配置内容的SHA256 |
| `created_at_ms` | INTEGER | 否 | — | — | 保存时间，UTC毫秒 |
| `save_reason` | TEXT | 否 | — | — | 形成独立快照的原因 |
| `local_validation_json` | TEXT | 否 | — | '{}' | 本地缺项与字段错误摘要；不是网络测试通过标记 |

外键：

- `(parent_revision_id) → config_revisions(revision_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_config_revisions_1 (revision_id)`；UNIQUE; origin=pk。

触发器：`config_revisions_no_update`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE config_revisions (
    revision_id TEXT NOT NULL PRIMARY KEY CHECK (length(revision_id) > 0), -- 不可变整体配置版本ID，与请求语义指纹分开
    parent_revision_id TEXT REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 来源版本；首个版本允许为空
    content_hash TEXT NOT NULL CHECK (instr(content_hash,char(0))=0 AND (length(content_hash) = 64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 规范化非秘密配置内容的SHA256
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0), -- 保存时间，UTC毫秒
    save_reason TEXT NOT NULL CHECK (save_reason IN ('create','edit','copy','import','restore','migrate')), -- 形成独立快照的原因
    local_validation_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(local_validation_json,char(0))=0 AND (json_valid(local_validation_json) AND json_type(local_validation_json) = 'object')), -- 本地缺项与字段错误摘要；不是网络测试通过标记
    CHECK (parent_revision_id IS NULL OR parent_revision_id <> revision_id)
) STRICT;
```

### connection_runtime_states

连接即时停用的独立安全闸，恢复必须明确执行，不随保存草稿自动开启。Owner：AP；范围：FP-06, FP-08, FP-12；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `connection_id` | TEXT | 否 | 1 | — | 稳定连接ID，跨配置版本安全闸 |
| `config_revision_id` | TEXT | 否 | — | — | 最近关联的已应用连接快照，仅供来源追溯 |
| `is_suspended` | INTEGER | 否 | — | — | 即时禁止新请求并取消在途，不能等待下一次应用 |
| `control_revision` | INTEGER | 否 | — | — | 安全闸CAS版本，显式恢复也递增 |
| `reason_code` | TEXT | 是 | — | — | 脱敏停用/恢复原因 |
| `updated_at_ms` | INTEGER | 否 | — | — | 最后明确控制时间UTC毫秒 |

外键：

- `(config_revision_id, connection_id) → connections(revision_id, connection_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_connection_runtime_states_1 (connection_id)`；UNIQUE; origin=pk。

触发器：`connection_control_cas`、`connection_runtime_states_requires_seal_insert`、`connection_runtime_states_requires_seal_update`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE connection_runtime_states (
    connection_id TEXT NOT NULL PRIMARY KEY, -- 稳定连接ID，跨配置版本安全闸
    config_revision_id TEXT NOT NULL, -- 最近关联的已应用连接快照，仅供来源追溯
    is_suspended INTEGER NOT NULL CHECK(is_suspended IN (0,1)), -- 即时禁止新请求并取消在途，不能等待下一次应用
    control_revision INTEGER NOT NULL CHECK(control_revision>0), -- 安全闸CAS版本，显式恢复也递增
    reason_code TEXT, -- 脱敏停用/恢复原因
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms>=0), -- 最后明确控制时间UTC毫秒
    FOREIGN KEY(config_revision_id,connection_id) REFERENCES connections(revision_id,connection_id) ON DELETE RESTRICT
) STRICT;
```

### connections

某整体配置版本内的连接、路由和非秘密认证引用。Owner：AP；范围：FP-06, FP-08, FP-11；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 所属不可变整体配置版本 |
| `connection_id` | TEXT | 否 | 2 | — | 跨版本稳定的连接ID |
| `display_name` | TEXT | 否 | — | — | 用户显示名；不参与请求语义身份 |
| `protocol` | TEXT | 否 | — | — | 协议适配目标；unsupported可保存草稿但不可应用为运行用途 |
| `deployment` | TEXT | 否 | — | — | 部署位置意图；实际地址与路由仍须应用校验 |
| `auth_mode` | TEXT | 否 | — | — | 认证方式；无认证只允许适配器认可的实际服务 |
| `endpoint` | TEXT | 是 | — | — | 非秘密服务地址；未完成草稿允许为空，禁止URL内嵌凭据 |
| `credential_pool_id` | TEXT | 是 | — | — | 显式选择的不可变凭据池ID；草稿允许未选择 |
| `credential_pool_revision` | INTEGER | 是 | — | — | 精确凭据池版本，不随池新建版本自动变化 |
| `proxy_mode` | TEXT | 否 | — | 'inherit' | 网络代理继承、显式覆盖或禁用 |
| `network_metadata_json` | TEXT | 否 | — | '{}' | 安全网络配置白名单，例如无秘密代理地址、超时和region |
| `availability_intent` | INTEGER | 否 | — | 0 | 用户连接可用意图；运行安全停用和恢复须另经应用控制 |

外键：

- `(credential_pool_id, credential_pool_revision) → credential_pool_revisions(pool_id, pool_revision)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(revision_id) → config_revisions(revision_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `connections_by_credential_pool (credential_pool_id, credential_pool_revision)`；origin=c。

```sql
CREATE INDEX connections_by_credential_pool ON connections(credential_pool_id, credential_pool_revision);
```
- `sqlite_autoindex_connections_1 (revision_id, connection_id)`；UNIQUE; origin=pk。

触发器：`connections_no_update`、`connections_pool_not_purged`、`connections_sealed_delete`、`connections_sealed_insert`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE connections (
    revision_id TEXT NOT NULL REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 所属不可变整体配置版本
    connection_id TEXT NOT NULL CHECK (length(connection_id) > 0), -- 跨版本稳定的连接ID
    display_name TEXT NOT NULL, -- 用户显示名；不参与请求语义身份
    protocol TEXT NOT NULL CHECK (protocol IN ('openai_compatible','anthropic','azure_translator','unsupported')), -- 协议适配目标；unsupported可保存草稿但不可应用为运行用途
    deployment TEXT NOT NULL CHECK (deployment IN ('local','remote','cloud')), -- 部署位置意图；实际地址与路由仍须应用校验
    auth_mode TEXT NOT NULL CHECK (auth_mode IN ('none','api_key')), -- 认证方式；无认证只允许适配器认可的实际服务
    endpoint TEXT, -- 非秘密服务地址；未完成草稿允许为空，禁止URL内嵌凭据
    credential_pool_id TEXT, -- 显式选择的不可变凭据池ID；草稿允许未选择
    credential_pool_revision INTEGER CHECK (credential_pool_revision IS NULL OR credential_pool_revision > 0), -- 精确凭据池版本，不随池新建版本自动变化
    proxy_mode TEXT NOT NULL DEFAULT 'inherit' CHECK (proxy_mode IN ('inherit','override','disabled')), -- 网络代理继承、显式覆盖或禁用
    network_metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(network_metadata_json,char(0))=0 AND (json_valid(network_metadata_json) AND json_type(network_metadata_json) = 'object')), -- 安全网络配置白名单，例如无秘密代理地址、超时和region
    availability_intent INTEGER NOT NULL DEFAULT 0 CHECK (availability_intent IN (0,1)), -- 用户连接可用意图；运行安全停用和恢复须另经应用控制
    PRIMARY KEY (revision_id, connection_id),
    FOREIGN KEY (credential_pool_id, credential_pool_revision) REFERENCES credential_pool_revisions(pool_id, pool_revision) ON DELETE RESTRICT,
    CHECK ((credential_pool_id IS NULL) = (credential_pool_revision IS NULL))
) STRICT;
```

### corrections

人工修正独立版本，机器新结果不能覆盖。Owner：AP；范围：FP-09；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `correction_id` | TEXT | 否 | 1 | — | 修正ID |
| `translation_id` | TEXT | 否 | — | — | 被修正译文 |
| `correction_revision` | INTEGER | 否 | — | — | 同译文人工版本 |
| `corrected_text` | TEXT | 否 | — | — | 用户保存文本，失败保留UI编辑 |
| `base_revision` | INTEGER | 否 | — | — | CAS前置版本 |
| `created_at_ms` | INTEGER | 否 | — | — | 保存时间 |

外键：

- `(translation_id) → translations(translation_id)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_corrections_1 (correction_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_corrections_2 (translation_id, correction_revision)`；UNIQUE; origin=u。

触发器：`immutable_corrections`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE corrections (
    correction_id TEXT NOT NULL PRIMARY KEY, -- 修正ID
    translation_id TEXT NOT NULL, -- 被修正译文
    correction_revision INTEGER NOT NULL CHECK(correction_revision>0), -- 同译文人工版本
    corrected_text TEXT NOT NULL, -- 用户保存文本，失败保留UI编辑
    base_revision INTEGER NOT NULL CHECK(base_revision>=0), -- CAS前置版本
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms IS NULL OR created_at_ms>=0), -- 保存时间
    FOREIGN KEY(translation_id) REFERENCES translations(translation_id) ON DELETE CASCADE,
    UNIQUE(translation_id,correction_revision)
) STRICT;
```

### credential_pool_gc_marks

不再被配置或测试引用的池版本清理墓碑。Owner：PL；范围：FP-09, FP-11；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `pool_id` | TEXT | 否 | 1 | — | 清理池ID |
| `pool_revision` | INTEGER | 否 | 2 | — | 清理池版本 |
| `operation_id` | TEXT | 否 | — | — | 与清理同事务回执 |
| `requested_at_ms` | INTEGER | 否 | — | — | 清理时间 |

外键：

- `(pool_id, pool_revision) → credential_pool_seals(pool_id, pool_revision)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(operation_id) → operation_receipts(operation_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_credential_pool_gc_marks_1 (pool_id, pool_revision)`；UNIQUE; origin=pk。

触发器：`credential_pool_gc_marks_no_delete`、`credential_pool_gc_marks_no_update`、`pool_gc_unreferenced`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE credential_pool_gc_marks (
    pool_id TEXT NOT NULL, -- 清理池ID
    pool_revision INTEGER NOT NULL, -- 清理池版本
    operation_id TEXT NOT NULL REFERENCES operation_receipts(operation_id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED, -- 与清理同事务回执
    requested_at_ms INTEGER NOT NULL CHECK(requested_at_ms>=0), -- 清理时间
    PRIMARY KEY(pool_id,pool_revision),
    FOREIGN KEY(pool_id,pool_revision) REFERENCES credential_pool_seals(pool_id,pool_revision) ON DELETE RESTRICT
) STRICT;
```

### credential_pool_members

某池版本中的精确凭据版本分配。Owner：PL；范围：FP-06, FP-08, FP-11；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `pool_id` | TEXT | 否 | 1 | — | 所属凭据池ID |
| `pool_revision` | INTEGER | 否 | 2 | — | 所属不可变池版本 |
| `member_id` | TEXT | 否 | 3 | — | 池成员稳定ID |
| `credential_id` | TEXT | 否 | — | — | 精确秘密版本；系统库不存在时不得授权请求 |
| `position` | INTEGER | 否 | — | — | 成员稳定顺序 |
| `enabled` | INTEGER | 否 | — | 1 | 该池版本的成员使用意图，不覆盖秘密撤销 |
| `weight` | INTEGER | 否 | — | 1 | 显式加权策略权重；运行错误权重不得原地改快照 |

外键：

- `(pool_id, pool_revision) → credential_pool_revisions(pool_id, pool_revision)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(credential_id) → credential_refs(credential_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `credential_pool_members_by_credential (credential_id, pool_id, pool_revision)`；origin=c。

```sql
CREATE INDEX credential_pool_members_by_credential ON credential_pool_members(credential_id, pool_id, pool_revision);
```
- `sqlite_autoindex_credential_pool_members_1 (pool_id, pool_revision, member_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_credential_pool_members_2 (pool_id, pool_revision, position)`；UNIQUE; origin=u。
- `sqlite_autoindex_credential_pool_members_3 (pool_id, pool_revision, credential_id)`；UNIQUE; origin=u。
- `sqlite_autoindex_credential_pool_members_4 (pool_id, pool_revision, member_id, credential_id)`；UNIQUE; origin=u。

触发器：`credential_pool_members_no_update`、`pool_members_sealed_delete`、`pool_members_sealed_insert`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE credential_pool_members (
    pool_id TEXT NOT NULL, -- 所属凭据池ID
    pool_revision INTEGER NOT NULL, -- 所属不可变池版本
    member_id TEXT NOT NULL CHECK (length(member_id) > 0), -- 池成员稳定ID
    credential_id TEXT NOT NULL REFERENCES credential_refs(credential_id) ON DELETE RESTRICT, -- 精确秘密版本；系统库不存在时不得授权请求
    position INTEGER NOT NULL CHECK (position >= 0), -- 成员稳定顺序
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)), -- 该池版本的成员使用意图，不覆盖秘密撤销
    weight INTEGER NOT NULL DEFAULT 1 CHECK (weight > 0), -- 显式加权策略权重；运行错误权重不得原地改快照
    PRIMARY KEY (pool_id, pool_revision, member_id),
    UNIQUE (pool_id, pool_revision, position),
    UNIQUE (pool_id, pool_revision, credential_id),
    UNIQUE (pool_id, pool_revision, member_id, credential_id),
    FOREIGN KEY (pool_id, pool_revision) REFERENCES credential_pool_revisions(pool_id, pool_revision) ON DELETE RESTRICT
) STRICT;
```

### credential_pool_revisions

显式凭据池的不可变成员策略版本。Owner：PL；范围：FP-06, FP-08, FP-11；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `pool_id` | TEXT | 否 | 1 | — | 跨池版本稳定的凭据池ID |
| `pool_revision` | INTEGER | 否 | 2 | — | 池版本；成员/策略变更生成新版本 |
| `policy` | TEXT | 否 | — | — | 显式单选或用户授权轮询策略；不授权换供应商 |
| `policy_metadata_json` | TEXT | 否 | — | '{}' | 有界退避、错误权重与次数等非秘密策略 |
| `created_at_ms` | INTEGER | 否 | — | — | 池版本保存时间，UTC毫秒 |
| `content_hash` | TEXT | 否 | — | — | 池成员与策略的语义SHA256 |

外键：

无。

索引：

- `sqlite_autoindex_credential_pool_revisions_1 (pool_id, pool_revision)`；UNIQUE; origin=pk。

触发器：`credential_pool_revisions_no_update`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE credential_pool_revisions (
    pool_id TEXT NOT NULL CHECK (length(pool_id) > 0), -- 跨池版本稳定的凭据池ID
    pool_revision INTEGER NOT NULL CHECK (pool_revision > 0), -- 池版本；成员/策略变更生成新版本
    policy TEXT NOT NULL CHECK (policy IN ('selected','round_robin','weighted')), -- 显式单选或用户授权轮询策略；不授权换供应商
    policy_metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(policy_metadata_json,char(0))=0 AND (json_valid(policy_metadata_json) AND json_type(policy_metadata_json) = 'object')), -- 有界退避、错误权重与次数等非秘密策略
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0), -- 池版本保存时间，UTC毫秒
    content_hash TEXT NOT NULL CHECK (instr(content_hash,char(0))=0 AND (length(content_hash) = 64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 池成员与策略的语义SHA256
    PRIMARY KEY (pool_id, pool_revision)
) STRICT;
```

### credential_pool_seals

凭据池成员和策略装配完成的不可撤销封存凭证。Owner：PL；范围：FP-06, FP-08, FP-11；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `pool_id` | TEXT | 否 | 1 | — | 被封存池ID |
| `pool_revision` | INTEGER | 否 | 2 | — | 被封存版本 |
| `content_hash` | TEXT | 否 | — | — | 与池版本头相同的规范语义摘要 |
| `sealed_at_ms` | INTEGER | 否 | — | — | 装配完成封存时间 |

外键：

- `(pool_id, pool_revision) → credential_pool_revisions(pool_id, pool_revision)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_credential_pool_seals_1 (pool_id, pool_revision)`；UNIQUE; origin=pk。

触发器：`credential_pool_seals_no_delete`、`credential_pool_seals_no_update`、`pool_seal_identity`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE credential_pool_seals (
    pool_id TEXT NOT NULL, -- 被封存池ID
    pool_revision INTEGER NOT NULL, -- 被封存版本
    content_hash TEXT NOT NULL CHECK(instr(content_hash,char(0))=0 AND (length(content_hash)=64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 与池版本头相同的规范语义摘要
    sealed_at_ms INTEGER NOT NULL CHECK(sealed_at_ms>=0), -- 装配完成封存时间
    PRIMARY KEY(pool_id,pool_revision),
    FOREIGN KEY(pool_id,pool_revision) REFERENCES credential_pool_revisions(pool_id,pool_revision) ON DELETE RESTRICT
) STRICT;
```

### credential_refs

系统安全库不可变秘密身份及可恢复生命周期元数据。Owner：PL；范围：FP-06, FP-08, FP-11, FP-12；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `credential_id` | TEXT | 否 | 1 | — | 单个不可变秘密版本ID；替换秘密必须生成新ID |
| `vault_ref` | TEXT | 否 | — | — | 系统安全库定位引用，不是Key值或可导出认证材料 |
| `masked_label` | TEXT | 否 | — | — | 脱敏Key身份标签，不回显完整秘密 |
| `auth_kind` | TEXT | 否 | — | — | 安全库保存的认证材料种类 |
| `state` | TEXT | 否 | — | — | pending不可授权；撤销或待清理均禁止发送 |
| `created_at_ms` | INTEGER | 否 | — | — | pending意图写入时间，UTC毫秒 |
| `activated_at_ms` | INTEGER | 是 | — | — | 系统秘密成功建立且本地承认可引用的时间 |
| `revoked_at_ms` | INTEGER | 是 | — | — | 撤销先持久化，系统库删除失败也不得清除该时间 |
| `cleanup_requested_at_ms` | INTEGER | 是 | — | — | 待回收或系统删除的请求时间 |
| `cleaned_at_ms` | INTEGER | 是 | — | — | 系统秘密已清理时间；元数据仍可支持旧历史来源 |
| `lifecycle_revision` | INTEGER | 否 | — | 0 | 生命周期CAS版本；秘密身份和vault_ref不随更新变化 |

外键：

无。

索引：

- `credential_refs_cleanup (state, cleanup_requested_at_ms)`；origin=c。

```sql
CREATE INDEX credential_refs_cleanup ON credential_refs(state, cleanup_requested_at_ms);
```
- `sqlite_autoindex_credential_refs_1 (credential_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_credential_refs_2 (vault_ref)`；UNIQUE; origin=u。

触发器：`credential_refs_identity_immutable`、`credential_refs_lifecycle_monotonic`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE credential_refs (
    credential_id TEXT NOT NULL PRIMARY KEY CHECK (length(credential_id) > 0), -- 单个不可变秘密版本ID；替换秘密必须生成新ID
    vault_ref TEXT NOT NULL UNIQUE CHECK (length(vault_ref) > 0), -- 系统安全库定位引用，不是Key值或可导出认证材料
    masked_label TEXT NOT NULL, -- 脱敏Key身份标签，不回显完整秘密
    auth_kind TEXT NOT NULL CHECK (auth_kind IN ('api_key')), -- 安全库保存的认证材料种类
    state TEXT NOT NULL CHECK (state IN ('pending','active','revoked','cleanup_pending')), -- pending不可授权；撤销或待清理均禁止发送
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0), -- pending意图写入时间，UTC毫秒
    activated_at_ms INTEGER CHECK (activated_at_ms IS NULL OR activated_at_ms >= created_at_ms), -- 系统秘密成功建立且本地承认可引用的时间
    revoked_at_ms INTEGER CHECK (revoked_at_ms IS NULL OR revoked_at_ms >= created_at_ms), -- 撤销先持久化，系统库删除失败也不得清除该时间
    cleanup_requested_at_ms INTEGER CHECK (cleanup_requested_at_ms IS NULL OR cleanup_requested_at_ms >= created_at_ms), -- 待回收或系统删除的请求时间
    cleaned_at_ms INTEGER CHECK (cleaned_at_ms IS NULL OR cleaned_at_ms >= created_at_ms), -- 系统秘密已清理时间；元数据仍可支持旧历史来源
    lifecycle_revision INTEGER NOT NULL DEFAULT 0 CHECK (lifecycle_revision >= 0), -- 生命周期CAS版本；秘密身份和vault_ref不随更新变化
    CHECK (state <> 'active' OR (activated_at_ms IS NOT NULL AND revoked_at_ms IS NULL AND cleaned_at_ms IS NULL)),
    CHECK (state NOT IN ('revoked','cleanup_pending') OR revoked_at_ms IS NOT NULL),
    CHECK (cleaned_at_ms IS NULL OR state = 'cleanup_pending')
) STRICT;
```

### engine_parameters

有序引擎项的模型特定参数三态覆盖。Owner：AP；范围：FP-06；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 所属不可变整体配置版本 |
| `binding_id` | TEXT | 否 | 2 | — | 同版本用途绑定ID |
| `engine_id` | TEXT | 否 | 3 | — | 同版本用途内的引擎项ID |
| `parameter_key` | TEXT | 否 | 4 | — | 适配器认可的参数名 |
| `mode` | TEXT | 否 | — | — | 继承、显式设置或省略发送 |
| `value_json` | TEXT | 是 | — | — | set保存实际JSON值，其余模式SQL NULL |

外键：

- `(revision_id, binding_id, engine_id) → binding_engines(revision_id, binding_id, engine_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_engine_parameters_1 (revision_id, binding_id, engine_id, parameter_key)`；UNIQUE; origin=pk。

触发器：`engine_parameters_no_update`、`engine_parameters_sealed_delete`、`engine_parameters_sealed_insert`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE engine_parameters (
    revision_id TEXT NOT NULL, -- 所属不可变整体配置版本
    binding_id TEXT NOT NULL, -- 同版本用途绑定ID
    engine_id TEXT NOT NULL, -- 同版本用途内的引擎项ID
    parameter_key TEXT NOT NULL CHECK (length(parameter_key) > 0), -- 适配器认可的参数名
    mode TEXT NOT NULL CHECK (mode IN ('inherit','set','omit')), -- 继承、显式设置或省略发送
    value_json TEXT CHECK (instr(value_json,char(0))=0 AND (value_json IS NULL OR json_valid(value_json))), -- set保存实际JSON值，其余模式SQL NULL
    PRIMARY KEY (revision_id, binding_id, engine_id, parameter_key),
    FOREIGN KEY (revision_id, binding_id, engine_id) REFERENCES binding_engines(revision_id, binding_id, engine_id) ON DELETE RESTRICT,
    CHECK ((mode = 'set' AND value_json IS NOT NULL) OR (mode IN ('inherit','omit') AND value_json IS NULL))
) STRICT;
```

### game_targets

用户已核对的可编辑启动/目标配置；会话另存当时身份。Owner：AP；范围：FP-02, FP-03；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `target_id` | TEXT | 否 | 1 | — | 稳定目标配置ID |
| `game_id` | TEXT | 否 | — | — | 所属游戏，一条当前目标配置 |
| `environment_kind` | TEXT | 否 | — | — | 首版只实际接入CrossOver；未解析可入草稿 |
| `environment_key` | TEXT | 否 | — | — | 真实Bottle稳定引用；未解析用独立待核验键 |
| `bottle_locator` | TEXT | 是 | — | — | 本机Bottle定位，受隐私导出策略控制 |
| `launch_path` | TEXT | 是 | — | — | 启动入口，可与实际EXE不同 |
| `canonical_exe_path` | TEXT | 是 | — | — | 核验后的实际Windows完整路径 |
| `target_path_key` | TEXT | 是 | — | — | Windows大小写/分隔等规范化比较键，由Rust解析生成 |
| `working_directory` | TEXT | 是 | — | — | 工作目录 |
| `arguments_json` | TEXT | 否 | — | '[]' | 结构化启动参数；应用校验禁止秘密参数 |
| `architecture` | TEXT | 是 | — | — | 核验位数，未知可缺省 |
| `verification_state` | TEXT | 否 | — | — | 目标核验状态，不代表已注入或收到文本 |
| `row_revision` | INTEGER | 否 | — | — | 跨窗编辑CAS版本 |
| `updated_at_ms` | INTEGER | 否 | — | — | 更新时间 |
| `archived_at_ms` | INTEGER | 是 | — | — | 移出标记，供活跃目标去重 |

外键：

- `(game_id) → games(game_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_game_targets_1 (target_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_game_targets_2 (game_id)`；UNIQUE; origin=u。
- `uq_game_target_environment_path (environment_key, target_path_key)`；UNIQUE; 部分索引; origin=c。

```sql
CREATE UNIQUE INDEX uq_game_target_environment_path ON game_targets(environment_key,target_path_key) WHERE archived_at_ms IS NULL AND target_path_key IS NOT NULL;
```

触发器：无。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE game_targets (
    target_id TEXT NOT NULL PRIMARY KEY, -- 稳定目标配置ID
    game_id TEXT NOT NULL UNIQUE, -- 所属游戏，一条当前目标配置
    environment_kind TEXT NOT NULL CHECK(environment_kind IN ('crossover','unresolved')), -- 首版只实际接入CrossOver；未解析可入草稿
    environment_key TEXT NOT NULL, -- 真实Bottle稳定引用；未解析用独立待核验键
    bottle_locator TEXT, -- 本机Bottle定位，受隐私导出策略控制
    launch_path TEXT, -- 启动入口，可与实际EXE不同
    canonical_exe_path TEXT, -- 核验后的实际Windows完整路径
    target_path_key TEXT, -- Windows大小写/分隔等规范化比较键，由Rust解析生成
    working_directory TEXT, -- 工作目录
    arguments_json TEXT NOT NULL DEFAULT '[]' CHECK(instr(arguments_json,char(0))=0 AND (json_valid(arguments_json) AND json_type(arguments_json)='array')), -- 结构化启动参数；应用校验禁止秘密参数
    architecture TEXT CHECK(architecture IS NULL OR architecture IN ('x86','x64')), -- 核验位数，未知可缺省
    verification_state TEXT NOT NULL CHECK(verification_state IN ('unresolved','verified','stale','invalid')), -- 目标核验状态，不代表已注入或收到文本
    row_revision INTEGER NOT NULL CHECK(row_revision>0), -- 跨窗编辑CAS版本
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms IS NULL OR updated_at_ms>=0), -- 更新时间
    archived_at_ms INTEGER CHECK(archived_at_ms IS NULL OR archived_at_ms>=0), -- 移出标记，供活跃目标去重
    FOREIGN KEY(game_id) REFERENCES games(game_id) ON DELETE RESTRICT
) STRICT;
```

### games

稳定游戏库记录，移出使用归档且不删除游戏文件。Owner：AP；范围：FP-01, FP-02, FP-09；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `game_id` | TEXT | 否 | 1 | — | 本档稳定游戏ID |
| `display_name` | TEXT | 否 | — | — | 用户显示名，不作为目标身份 |
| `original_title` | TEXT | 是 | — | — | 原名，可缺省 |
| `source_language` | TEXT | 是 | — | — | 用户声明语言，未知不伪造日语 |
| `cover_asset_ref` | TEXT | 是 | — | — | 本机封面资源引用，不存游戏资产二进制 |
| `row_revision` | INTEGER | 否 | — | — | 乐观并发基值 |
| `created_at_ms` | INTEGER | 否 | — | — | 入库时间 |
| `updated_at_ms` | INTEGER | 否 | — | — | 最后编辑时间 |
| `archived_at_ms` | INTEGER | 是 | — | — | 移出库时间；历史保留 |

外键：

无。

索引：

- `ix_games_active (archived_at_ms, updated_at_ms, game_id)`；origin=c。

```sql
CREATE INDEX ix_games_active ON games(archived_at_ms, updated_at_ms DESC, game_id);
```
- `sqlite_autoindex_games_1 (game_id)`；UNIQUE; origin=pk。

触发器：无。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE games (
    game_id TEXT NOT NULL PRIMARY KEY, -- 本档稳定游戏ID
    display_name TEXT NOT NULL, -- 用户显示名，不作为目标身份
    original_title TEXT, -- 原名，可缺省
    source_language TEXT, -- 用户声明语言，未知不伪造日语
    cover_asset_ref TEXT, -- 本机封面资源引用，不存游戏资产二进制
    row_revision INTEGER NOT NULL CHECK(row_revision>0), -- 乐观并发基值
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms IS NULL OR created_at_ms>=0), -- 入库时间
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms IS NULL OR updated_at_ms>=0), -- 最后编辑时间
    archived_at_ms INTEGER CHECK(archived_at_ms IS NULL OR archived_at_ms>=0) -- 移出库时间；历史保留
) STRICT;
```

### lexical_snapshots

不可变真实词法结果，资源/schema身份独立于正文版本。Owner：JA；范围：FP-04, FP-07, FP-08；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `lexical_id` | TEXT | 否 | 1 | — | 分析快照ID |
| `unit_id` | TEXT | 否 | — | — | 固定正文 |
| `text_revision` | INTEGER | 否 | — | — | 固定正文版本 |
| `source_sha256` | TEXT | 否 | — | — | 必须匹配正文原文摘要 |
| `language_resource_snapshot_id` | TEXT | 否 | — | — | 实际语言资源manifest |
| `token_schema_version` | TEXT | 否 | — | — | 词法输出schema |
| `analysis_revision` | TEXT | 否 | — | — | 分析器/对齐规则版本 |
| `token_count` | INTEGER | 否 | — | — | 验证过的token数 |
| `created_at_ms` | INTEGER | 否 | — | — | 完成时间 |

外键：

- `(language_resource_snapshot_id) → resource_snapshots(resource_snapshot_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(unit_id, text_revision) → reading_unit_revisions(unit_id, text_revision)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_lexical_snapshots_1 (lexical_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_lexical_snapshots_2 (unit_id, text_revision, language_resource_snapshot_id, token_schema_version, analysis_revision)`；UNIQUE; origin=u。

触发器：`immutable_lexical_snapshots`、`lexical_resource_kind`、`lexical_source_hash`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE lexical_snapshots (
    lexical_id TEXT NOT NULL PRIMARY KEY, -- 分析快照ID
    unit_id TEXT NOT NULL, -- 固定正文
    text_revision INTEGER NOT NULL, -- 固定正文版本
    source_sha256 TEXT NOT NULL CHECK(instr(source_sha256,char(0))=0 AND (source_sha256 IS NULL OR (length(source_sha256)=64 AND source_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 必须匹配正文原文摘要
    language_resource_snapshot_id TEXT NOT NULL, -- 实际语言资源manifest
    token_schema_version TEXT NOT NULL, -- 词法输出schema
    analysis_revision TEXT NOT NULL, -- 分析器/对齐规则版本
    token_count INTEGER NOT NULL CHECK(token_count>=0), -- 验证过的token数
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms IS NULL OR created_at_ms>=0), -- 完成时间
    FOREIGN KEY(unit_id,text_revision) REFERENCES reading_unit_revisions(unit_id,text_revision) ON DELETE CASCADE,
    FOREIGN KEY(language_resource_snapshot_id) REFERENCES resource_snapshots(resource_snapshot_id) ON DELETE RESTRICT,
    UNIQUE(unit_id,text_revision,language_resource_snapshot_id,token_schema_version,analysis_revision)
) STRICT;
```

### lexical_tokens

相对原文UTF8字节的token；UTF16为投影换算值不持久作为真值。Owner：JA；范围：FP-07, FP-08；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `lexical_id` | TEXT | 否 | 1 | — | 分析快照 |
| `token_index` | INTEGER | 否 | 2 | — | token顺序 |
| `byte_start` | INTEGER | 否 | — | — | UTF8起始字节 |
| `byte_end` | INTEGER | 否 | — | — | UTF8结束字节 |
| `surface` | TEXT | 否 | — | — | 必须等于原文此字节片段 |
| `lemma` | TEXT | 否 | — | — | 词元，未知保留词面 |
| `reading_kana` | TEXT | 是 | — | — | 读音，未知不伪造 |
| `pos_major` | TEXT | 否 | — | — | 主词性 |
| `features_json` | TEXT | 否 | — | — | UniDic其余特征及明确schema字段 |
| `token_kind` | TEXT | 否 | — | — | 可逆重建所需类别 |
| `is_known` | INTEGER | 否 | — | — | 词法已知标记，不等于辞书中文命中 |

外键：

- `(lexical_id) → lexical_snapshots(lexical_id)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_lexical_tokens_1 (lexical_id, token_index)`；UNIQUE; origin=pk。

触发器：`immutable_lexical_tokens`、`lexical_token_slice`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE lexical_tokens (
    lexical_id TEXT NOT NULL, -- 分析快照
    token_index INTEGER NOT NULL CHECK(token_index>=0), -- token顺序
    byte_start INTEGER NOT NULL CHECK(byte_start>=0), -- UTF8起始字节
    byte_end INTEGER NOT NULL CHECK(byte_end>byte_start), -- UTF8结束字节
    surface TEXT NOT NULL CHECK(length(CAST(surface AS BLOB))=byte_end-byte_start), -- 必须等于原文此字节片段
    lemma TEXT NOT NULL, -- 词元，未知保留词面
    reading_kana TEXT, -- 读音，未知不伪造
    pos_major TEXT NOT NULL, -- 主词性
    features_json TEXT NOT NULL CHECK(instr(features_json,char(0))=0 AND (json_valid(features_json) AND json_type(features_json)='object')), -- UniDic其余特征及明确schema字段
    token_kind TEXT NOT NULL CHECK(token_kind IN ('word','punctuation','whitespace','unknown')), -- 可逆重建所需类别
    is_known INTEGER NOT NULL CHECK(is_known IN (0,1)), -- 词法已知标记，不等于辞书中文命中
    PRIMARY KEY(lexical_id,token_index),
    FOREIGN KEY(lexical_id) REFERENCES lexical_snapshots(lexical_id) ON DELETE CASCADE
) STRICT;
```

### model_default_templates

模型按用途默认模板，不把翻译模板充作文法模板。Owner：AP；范围：FP-06, FP-08；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 所属不可变整体配置版本 |
| `model_id` | TEXT | 否 | 2 | — | 同版本模型配置ID |
| `purpose` | TEXT | 否 | 3 | — | 独立用途 |
| `template_id` | TEXT | 否 | — | — | 同版本且同用途模板ID |

外键：

- `(revision_id, template_id, purpose) → templates(revision_id, template_id, purpose)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(revision_id, model_id) → models(revision_id, model_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_model_default_templates_1 (revision_id, model_id, purpose)`；UNIQUE; origin=pk。

触发器：`model_default_templates_no_update`、`model_default_templates_sealed_delete`、`model_default_templates_sealed_insert`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE model_default_templates (
    revision_id TEXT NOT NULL, -- 所属不可变整体配置版本
    model_id TEXT NOT NULL, -- 同版本模型配置ID
    purpose TEXT NOT NULL CHECK (purpose IN ('translation','grammar','context_word')), -- 独立用途
    template_id TEXT NOT NULL, -- 同版本且同用途模板ID
    PRIMARY KEY (revision_id, model_id, purpose),
    FOREIGN KEY (revision_id, model_id) REFERENCES models(revision_id, model_id) ON DELETE RESTRICT,
    FOREIGN KEY (revision_id, template_id, purpose) REFERENCES templates(revision_id, template_id, purpose) ON DELETE RESTRICT
) STRICT;
```

### model_parameters

模型默认参数逐键三态，不把显式零与省略混同。Owner：AP；范围：FP-06；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 所属不可变整体配置版本 |
| `model_id` | TEXT | 否 | 2 | — | 同版本模型配置ID |
| `parameter_key` | TEXT | 否 | 3 | — | 适配器认可的参数名；保留字段由应用拒绝 |
| `mode` | TEXT | 否 | — | — | 继承适配器默认、显式设置或省略发送 |
| `value_json` | TEXT | 是 | — | — | set时保存实际JSON值，显式0保留；其余模式SQL NULL |

外键：

- `(revision_id, model_id) → models(revision_id, model_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_model_parameters_1 (revision_id, model_id, parameter_key)`；UNIQUE; origin=pk。

触发器：`model_parameters_no_update`、`model_parameters_sealed_delete`、`model_parameters_sealed_insert`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE model_parameters (
    revision_id TEXT NOT NULL, -- 所属不可变整体配置版本
    model_id TEXT NOT NULL, -- 同版本模型配置ID
    parameter_key TEXT NOT NULL CHECK (length(parameter_key) > 0), -- 适配器认可的参数名；保留字段由应用拒绝
    mode TEXT NOT NULL CHECK (mode IN ('inherit','set','omit')), -- 继承适配器默认、显式设置或省略发送
    value_json TEXT CHECK (instr(value_json,char(0))=0 AND (value_json IS NULL OR json_valid(value_json))), -- set时保存实际JSON值，显式0保留；其余模式SQL NULL
    PRIMARY KEY (revision_id, model_id, parameter_key),
    FOREIGN KEY (revision_id, model_id) REFERENCES models(revision_id, model_id) ON DELETE RESTRICT,
    CHECK ((mode = 'set' AND value_json IS NOT NULL) OR (mode IN ('inherit','omit') AND value_json IS NULL))
) STRICT;
```

### models

某配置版本内的实际模型身份与有来源的能力声明。Owner：AP；范围：FP-06, FP-08；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 所属不可变整体配置版本 |
| `model_id` | TEXT | 否 | 2 | — | 跨配置版本稳定的模型配置ID，不等于供应商模型ID |
| `connection_id` | TEXT | 否 | — | — | 同一整体版本内引用的连接ID |
| `actual_model_id` | TEXT | 是 | — | — | 实际请求模型ID；草稿可空，传统翻译无模型时保持空 |
| `display_name` | TEXT | 否 | — | — | 用户显示名；重命名不改变实际请求模型身份 |
| `selection_source` | TEXT | 否 | — | 'manual' | 手填、列表发现或导入来源 |
| `capability_json` | TEXT | 否 | — | '{}' | 能力、unknown/unsupported及文档/用户/测试来源；列表出现不等于通过 |

外键：

- `(revision_id, connection_id) → connections(revision_id, connection_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(revision_id) → config_revisions(revision_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `models_by_connection (revision_id, connection_id)`；origin=c。

```sql
CREATE INDEX models_by_connection ON models(revision_id, connection_id);
```
- `sqlite_autoindex_models_1 (revision_id, model_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_models_2 (revision_id, model_id, connection_id)`；UNIQUE; origin=u。

触发器：`models_no_update`、`models_sealed_delete`、`models_sealed_insert`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE models (
    revision_id TEXT NOT NULL REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 所属不可变整体配置版本
    model_id TEXT NOT NULL CHECK (length(model_id) > 0), -- 跨配置版本稳定的模型配置ID，不等于供应商模型ID
    connection_id TEXT NOT NULL, -- 同一整体版本内引用的连接ID
    actual_model_id TEXT, -- 实际请求模型ID；草稿可空，传统翻译无模型时保持空
    display_name TEXT NOT NULL, -- 用户显示名；重命名不改变实际请求模型身份
    selection_source TEXT NOT NULL DEFAULT 'manual' CHECK (selection_source IN ('manual','discovered','imported')), -- 手填、列表发现或导入来源
    capability_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(capability_json,char(0))=0 AND (json_valid(capability_json) AND json_type(capability_json) = 'object')), -- 能力、unknown/unsupported及文档/用户/测试来源；列表出现不等于通过
    PRIMARY KEY (revision_id, model_id),
    UNIQUE (revision_id, model_id, connection_id),
    FOREIGN KEY (revision_id, connection_id) REFERENCES connections(revision_id, connection_id) ON DELETE RESTRICT
) STRICT;
```

### operation_receipts

业务幂等回执，与业务写入同事务提交。Owner：AP；范围：FP-02, FP-05, FP-06, FP-09；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `operation_id` | TEXT | 否 | 1 | — | 客户端操作幂等ID |
| `request_sha256` | TEXT | 否 | — | — | 相同操作ID的规范请求摘要，冲突拒绝 |
| `command_kind` | TEXT | 否 | — | — | 已注册命令种类 |
| `commit_revision` | INTEGER | 否 | — | — | 提交时的本地revision |
| `result_refs_json` | TEXT | 否 | — | — | 仅结果身份/版本与脱敏回执，不含原始秘密 |
| `committed_at_ms` | INTEGER | 否 | — | — | 提交时间 |

外键：

无。

索引：

- `sqlite_autoindex_operation_receipts_1 (operation_id)`；UNIQUE; origin=pk。

触发器：`immutable_operation_receipts`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE operation_receipts (
    operation_id TEXT NOT NULL PRIMARY KEY, -- 客户端操作幂等ID
    request_sha256 TEXT NOT NULL CHECK(instr(request_sha256,char(0))=0 AND (request_sha256 IS NULL OR (length(request_sha256)=64 AND request_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 相同操作ID的规范请求摘要，冲突拒绝
    command_kind TEXT NOT NULL, -- 已注册命令种类
    commit_revision INTEGER NOT NULL CHECK(commit_revision>0), -- 提交时的本地revision
    result_refs_json TEXT NOT NULL CHECK(instr(result_refs_json,char(0))=0 AND (json_valid(result_refs_json) AND json_type(result_refs_json)='object')), -- 仅结果身份/版本与脱敏回执，不含原始秘密
    committed_at_ms INTEGER NOT NULL CHECK(committed_at_ms IS NULL OR committed_at_ms>=0) -- 提交时间
) STRICT;
```

### preference_heads

每scope当前偏好头，可CAS更新。Owner：AP；范围：FP-11；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `scope_key` | TEXT | 否 | 1 | — | 偏好作用域 |
| `preference_revision_id` | TEXT | 否 | — | — | 当前偏好快照 |
| `head_revision` | INTEGER | 否 | — | — | 并发编辑版本 |
| `updated_at_ms` | INTEGER | 否 | — | — | 更新时间 |

外键：

- `(preference_revision_id, scope_key) → preference_revisions(preference_revision_id, scope_key)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_preference_heads_1 (scope_key)`；UNIQUE; origin=pk。

触发器：`preference_head_cas`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE preference_heads (
    scope_key TEXT NOT NULL PRIMARY KEY, -- 偏好作用域
    preference_revision_id TEXT NOT NULL, -- 当前偏好快照
    head_revision INTEGER NOT NULL CHECK(head_revision>0), -- 并发编辑版本
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms IS NULL OR updated_at_ms>=0), -- 更新时间
    FOREIGN KEY(preference_revision_id,scope_key) REFERENCES preference_revisions(preference_revision_id,scope_key) ON DELETE RESTRICT
) STRICT;
```

### preference_revisions

阅读偏好独立不可变快照，不影响无关在途请求语义。Owner：AP；范围：FP-04, FP-07, FP-11；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `preference_revision_id` | TEXT | 否 | 1 | — | 偏好版本ID |
| `scope_key` | TEXT | 否 | — | — | global或game:<id> |
| `game_id` | TEXT | 是 | — | — | 游戏覆盖；全局为NULL |
| `parent_revision_id` | TEXT | 是 | — | — | 前一偏好版本 |
| `theme` | TEXT | 否 | — | — | 主题意图 |
| `transparency_mode` | TEXT | 否 | — | — | 减少透明度与材质意图 |
| `show_ruby` | INTEGER | 否 | — | — | 注音开关 |
| `show_pos` | INTEGER | 否 | — | — | 词性显示 |
| `source_font_pt` | REAL | 否 | — | — | 原文字号pt，设计当前舒适18/紧凑16 |
| `translation_font_pt` | REAL | 否 | — | — | 译文字号pt，当前舒适24/紧凑22 |
| `hud_width_pt` | REAL | 否 | — | — | 当前舒适560/紧凑500；高度自适应 |
| `shortcuts_json` | TEXT | 否 | — | — | 平台键位与显示快捷键意图 |
| `overrides_json` | TEXT | 否 | — | — | 字段继承/显式覆盖掩码，区分全局与游戏来源 |
| `created_at_ms` | INTEGER | 否 | — | — | 保存时间 |

外键：

- `(parent_revision_id) → preference_revisions(preference_revision_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(game_id) → games(game_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_preference_revisions_1 (preference_revision_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_preference_revisions_2 (preference_revision_id, scope_key)`；UNIQUE; origin=u。

触发器：`immutable_preference_revisions`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE preference_revisions (
    preference_revision_id TEXT NOT NULL PRIMARY KEY, -- 偏好版本ID
    scope_key TEXT NOT NULL, -- global或game:<id>
    game_id TEXT, -- 游戏覆盖；全局为NULL
    parent_revision_id TEXT, -- 前一偏好版本
    theme TEXT NOT NULL CHECK(theme IN ('system','light','dark')), -- 主题意图
    transparency_mode TEXT NOT NULL CHECK(transparency_mode IN ('system','reduced','material')), -- 减少透明度与材质意图
    show_ruby INTEGER NOT NULL CHECK(show_ruby IN (0,1)), -- 注音开关
    show_pos INTEGER NOT NULL CHECK(show_pos IN (0,1)), -- 词性显示
    source_font_pt REAL NOT NULL CHECK(source_font_pt>0), -- 原文字号pt，设计当前舒适18/紧凑16
    translation_font_pt REAL NOT NULL CHECK(translation_font_pt>0), -- 译文字号pt，当前舒适24/紧凑22
    hud_width_pt REAL NOT NULL CHECK(hud_width_pt>0), -- 当前舒适560/紧凑500；高度自适应
    shortcuts_json TEXT NOT NULL CHECK(instr(shortcuts_json,char(0))=0 AND (json_valid(shortcuts_json) AND json_type(shortcuts_json)='object')), -- 平台键位与显示快捷键意图
    overrides_json TEXT NOT NULL CHECK(instr(overrides_json,char(0))=0 AND (json_valid(overrides_json) AND json_type(overrides_json)='object')), -- 字段继承/显式覆盖掩码，区分全局与游戏来源
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms IS NULL OR created_at_ms>=0), -- 保存时间
    FOREIGN KEY(game_id) REFERENCES games(game_id) ON DELETE RESTRICT,
    FOREIGN KEY(parent_revision_id) REFERENCES preference_revisions(preference_revision_id) ON DELETE RESTRICT,
    UNIQUE(preference_revision_id,scope_key),
    CHECK((scope_key='global' AND game_id IS NULL) OR (game_id IS NOT NULL AND scope_key='game:'||game_id))
) STRICT;
```

### profile_meta

本配置档唯一身份、writer提交序号及运行代次。Owner：AP；范围：FP-01, FP-12；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `singleton` | INTEGER | 否 | 1 | — | 唯一行固定1 |
| `profile_id` | TEXT | 否 | — | — | 与profiles.json索引一致的稳定档ID |
| `commit_revision` | INTEGER | 否 | — | 0 | 成功业务事务的本地递增提交号，非UI投影revision |
| `runtime_generation` | INTEGER | 否 | — | — | 本档激活/切换时递增的任务代次 |
| `server_instance_id` | TEXT | 否 | — | — | 当前应用服务实例，不复用旧进程值 |
| `runtime_state` | TEXT | 否 | — | — | 是否允许此档请求提交 |
| `created_at_ms` | INTEGER | 否 | — | — | 建档时间 |
| `updated_at_ms` | INTEGER | 否 | — | — | 最后运行身份更新时间 |

外键：

无。

索引：

- `sqlite_autoindex_profile_meta_1 (profile_id)`；UNIQUE; origin=u。

触发器：无。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE profile_meta (
    singleton INTEGER NOT NULL PRIMARY KEY CHECK(singleton=1), -- 唯一行固定1
    profile_id TEXT NOT NULL UNIQUE, -- 与profiles.json索引一致的稳定档ID
    commit_revision INTEGER NOT NULL DEFAULT 0 CHECK(commit_revision>=0), -- 成功业务事务的本地递增提交号，非UI投影revision
    runtime_generation INTEGER NOT NULL CHECK(runtime_generation>0), -- 本档激活/切换时递增的任务代次
    server_instance_id TEXT NOT NULL, -- 当前应用服务实例，不复用旧进程值
    runtime_state TEXT NOT NULL CHECK(runtime_state IN ('active','inactive')), -- 是否允许此档请求提交
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms IS NULL OR created_at_ms>=0), -- 建档时间
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms IS NULL OR updated_at_ms>=0) -- 最后运行身份更新时间
) STRICT;
```

### purpose_bindings

全局或游戏的用途意图、覆盖、输入范围与调度快照。Owner：AP；范围：FP-06, FP-08, FP-11；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 所属不可变整体配置版本 |
| `binding_id` | TEXT | 否 | 2 | — | 跨版本稳定的用途绑定ID |
| `scope_key` | TEXT | 否 | — | — | 显式作用域键global或game:<game_id>，避免NULL唯一约束歧义 |
| `game_id` | TEXT | 是 | — | — | 游戏覆盖的稳定库ID；全局为空，库移出不得硬删历史引用 |
| `purpose` | TEXT | 否 | — | — | 翻译、句子精读、语境查词彼此独立 |
| `binding_mode` | TEXT | 否 | — | 'override' | 游戏继承全局或显式覆盖；全局必须override |
| `enabled` | INTEGER | 否 | — | 0 | 用户用途启用意图；显式关闭在override下阻断继承 |
| `output_mode` | TEXT | 是 | — | — | 用途输出契约；未完成草稿允许为空 |
| `source_language` | TEXT | 是 | — | — | 原句语言；手动或自动策略由应用校验 |
| `target_language` | TEXT | 是 | — | — | 翻译目标或学习解释语言 |
| `template_id` | TEXT | 是 | — | — | 用途模板引用；为空表示继续按模型默认等规则解析 |
| `context_json` | TEXT | 否 | — | '{}' | 仅当前句/允许历史、条数与预算、术语和资料范围 |
| `schedule_json` | TEXT | 否 | — | '{}' | 有界超时、用途总并发、队列、显式重试与回退策略 |

外键：

- `(revision_id, template_id, purpose) → templates(revision_id, template_id, purpose)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(game_id) → games(game_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(revision_id) → config_revisions(revision_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `purpose_bindings_scope_identity (revision_id, binding_id, scope_key, purpose)`；UNIQUE; origin=c。

```sql
CREATE UNIQUE INDEX purpose_bindings_scope_identity ON purpose_bindings(revision_id,binding_id,scope_key,purpose);
```
- `sqlite_autoindex_purpose_bindings_1 (revision_id, binding_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_purpose_bindings_2 (revision_id, scope_key, purpose)`；UNIQUE; origin=u。
- `sqlite_autoindex_purpose_bindings_3 (revision_id, binding_id, purpose)`；UNIQUE; origin=u。

触发器：`purpose_bindings_no_update`、`purpose_bindings_sealed_delete`、`purpose_bindings_sealed_insert`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE purpose_bindings (
    revision_id TEXT NOT NULL REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 所属不可变整体配置版本
    binding_id TEXT NOT NULL CHECK (length(binding_id) > 0), -- 跨版本稳定的用途绑定ID
    scope_key TEXT NOT NULL, -- 显式作用域键global或game:<game_id>，避免NULL唯一约束歧义
    game_id TEXT REFERENCES games(game_id) ON DELETE RESTRICT, -- 游戏覆盖的稳定库ID；全局为空，库移出不得硬删历史引用
    purpose TEXT NOT NULL CHECK (purpose IN ('translation','grammar','context_word')), -- 翻译、句子精读、语境查词彼此独立
    binding_mode TEXT NOT NULL DEFAULT 'override' CHECK (binding_mode IN ('inherit','override')), -- 游戏继承全局或显式覆盖；全局必须override
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0,1)), -- 用户用途启用意图；显式关闭在override下阻断继承
    output_mode TEXT CHECK (output_mode IS NULL OR output_mode IN ('text','structured','plain_text')), -- 用途输出契约；未完成草稿允许为空
    source_language TEXT, -- 原句语言；手动或自动策略由应用校验
    target_language TEXT, -- 翻译目标或学习解释语言
    template_id TEXT, -- 用途模板引用；为空表示继续按模型默认等规则解析
    context_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(context_json,char(0))=0 AND (json_valid(context_json) AND json_type(context_json) = 'object')), -- 仅当前句/允许历史、条数与预算、术语和资料范围
    schedule_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(schedule_json,char(0))=0 AND (json_valid(schedule_json) AND json_type(schedule_json) = 'object')), -- 有界超时、用途总并发、队列、显式重试与回退策略
    PRIMARY KEY (revision_id, binding_id),
    UNIQUE (revision_id, scope_key, purpose),
    UNIQUE (revision_id, binding_id, purpose),
    FOREIGN KEY (revision_id, template_id, purpose) REFERENCES templates(revision_id, template_id, purpose) ON DELETE RESTRICT,
    CHECK ((scope_key = 'global' AND game_id IS NULL AND binding_mode = 'override') OR (game_id IS NOT NULL AND scope_key = 'game:' || game_id))
) STRICT;
```

### purpose_runtime_states

用途当前有效授权epoch；不以单纯配置显示名变更取消任务。Owner：AP；范围：FP-06, FP-08, FP-12；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `scope_key` | TEXT | 否 | 1 | — | global或game:<id>有效作用域 |
| `purpose` | TEXT | 否 | 2 | — | 用途 |
| `binding_id` | TEXT | 否 | — | — | 当前有效稳定绑定ID |
| `config_revision_id` | TEXT | 否 | — | — | 当前已应用配置快照 |
| `is_enabled` | INTEGER | 否 | — | — | 当前可发请求状态 |
| `authorization_epoch` | INTEGER | 否 | — | — | 语义/授权变化递增；停用先持久化 |
| `config_semantics_sha256` | TEXT | 否 | — | — | 仅有效配置语义，不包括界面显示名 |
| `updated_at_ms` | INTEGER | 否 | — | — | 最后应用/授权变化时间 |

外键：

- `(config_revision_id, binding_id, scope_key, purpose) → purpose_bindings(revision_id, binding_id, scope_key, purpose)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(config_revision_id, binding_id) → purpose_bindings(revision_id, binding_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_purpose_runtime_states_1 (scope_key, purpose)`；UNIQUE; origin=pk。

触发器：`purpose_runtime_epoch`、`purpose_runtime_states_requires_seal_insert`、`purpose_runtime_states_requires_seal_update`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE purpose_runtime_states (
    scope_key TEXT NOT NULL, -- global或game:<id>有效作用域
    purpose TEXT NOT NULL CHECK(purpose IN ('translation','grammar','context_word')), -- 用途
    binding_id TEXT NOT NULL, -- 当前有效稳定绑定ID
    config_revision_id TEXT NOT NULL, -- 当前已应用配置快照
    is_enabled INTEGER NOT NULL CHECK(is_enabled IN (0,1)), -- 当前可发请求状态
    authorization_epoch INTEGER NOT NULL CHECK(authorization_epoch>0), -- 语义/授权变化递增；停用先持久化
    config_semantics_sha256 TEXT NOT NULL CHECK(instr(config_semantics_sha256,char(0))=0 AND (config_semantics_sha256 IS NULL OR (length(config_semantics_sha256)=64 AND config_semantics_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 仅有效配置语义，不包括界面显示名
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms IS NULL OR updated_at_ms>=0), -- 最后应用/授权变化时间
    PRIMARY KEY(scope_key,purpose),
    FOREIGN KEY(config_revision_id,binding_id) REFERENCES purpose_bindings(revision_id,binding_id) ON DELETE RESTRICT,
    FOREIGN KEY(config_revision_id,binding_id,scope_key,purpose) REFERENCES purpose_bindings(revision_id,binding_id,scope_key,purpose) ON DELETE RESTRICT
) STRICT;
```

### raw_event_payloads

可按显式限额删除的原始字节，删除不级联历史与来源身份。Owner：AP；范围：FP-04, FP-09, FP-12；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `event_id` | TEXT | 否 | 1 | — | 对应事件 |
| `payload_bytes` | BLOB | 否 | — | — | 有界原始字节，可为空BLOB |
| `retained_at_ms` | INTEGER | 否 | — | — | 写入时间 |

外键：

- `(event_id) → raw_events(event_id)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_raw_event_payloads_1 (event_id)`；UNIQUE; origin=pk。

触发器：`immutable_raw_payload`、`raw_payload_length`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE raw_event_payloads (
    event_id TEXT NOT NULL PRIMARY KEY, -- 对应事件
    payload_bytes BLOB NOT NULL, -- 有界原始字节，可为空BLOB
    retained_at_ms INTEGER NOT NULL CHECK(retained_at_ms IS NULL OR retained_at_ms>=0), -- 写入时间
    FOREIGN KEY(event_id) REFERENCES raw_events(event_id) ON DELETE CASCADE
) STRICT;
```

### raw_events

不可变事件身份与payload摘要，原始payload独立限额清理。Owner：HK；范围：FP-04, FP-09, FP-12；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `event_id` | TEXT | 否 | 1 | — | 原始事件ID |
| `session_id` | TEXT | 否 | — | — | 来源会话 |
| `source_id` | TEXT | 是 | — | — | 控制/丢失事件可无正文流 |
| `wire_sequence_u64` | TEXT | 否 | — | — | broker规范uint64序号，排序使用ingest_sequence |
| `ingest_sequence` | INTEGER | 否 | — | — | DB本地本会话连续序号，安全整数上界由应用限制 |
| `event_kind` | TEXT | 否 | — | — | 正文、控制、流生命周期或完整性丢失 |
| `payload_byte_length` | INTEGER | 否 | — | — | 原始payload字节数，清理后仍保留 |
| `payload_sha256` | TEXT | 否 | — | — | 原始payload内容摘要 |
| `observed_at_ms` | INTEGER | 否 | — | — | 客户端观测时间 |
| `integrity_state` | TEXT | 否 | — | — | 当时完整性，不以缺行伪装正常 |

外键：

- `(source_id, session_id) → capture_sources(source_id, session_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(session_id) → sessions(session_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `ix_raw_source_order (source_id, ingest_sequence)`；origin=c。

```sql
CREATE INDEX ix_raw_source_order ON raw_events(source_id,ingest_sequence);
```
- `sqlite_autoindex_raw_events_1 (event_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_raw_events_2 (event_id, session_id)`；UNIQUE; origin=u。
- `sqlite_autoindex_raw_events_3 (session_id, wire_sequence_u64)`；UNIQUE; origin=u。
- `sqlite_autoindex_raw_events_4 (session_id, ingest_sequence)`；UNIQUE; origin=u。

触发器：`immutable_raw_events`、`raw_event_requires_hook`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE raw_events (
    event_id TEXT NOT NULL PRIMARY KEY, -- 原始事件ID
    session_id TEXT NOT NULL, -- 来源会话
    source_id TEXT, -- 控制/丢失事件可无正文流
    wire_sequence_u64 TEXT NOT NULL CHECK(instr(wire_sequence_u64,char(0))=0 AND (length(wire_sequence_u64) BETWEEN 1 AND 20 AND wire_sequence_u64 NOT GLOB '*[^0-9]*' AND (length(wire_sequence_u64)=1 OR substr(wire_sequence_u64,1,1)<>'0') AND (length(wire_sequence_u64)<20 OR wire_sequence_u64<='18446744073709551615'))), -- broker规范uint64序号，排序使用ingest_sequence
    ingest_sequence INTEGER NOT NULL CHECK(ingest_sequence>0), -- DB本地本会话连续序号，安全整数上界由应用限制
    event_kind TEXT NOT NULL CHECK(event_kind IN ('text','control','source','integrity_loss')), -- 正文、控制、流生命周期或完整性丢失
    payload_byte_length INTEGER NOT NULL CHECK(payload_byte_length>=0), -- 原始payload字节数，清理后仍保留
    payload_sha256 TEXT NOT NULL CHECK(instr(payload_sha256,char(0))=0 AND (payload_sha256 IS NULL OR (length(payload_sha256)=64 AND payload_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 原始payload内容摘要
    observed_at_ms INTEGER NOT NULL CHECK(observed_at_ms IS NULL OR observed_at_ms>=0), -- 客户端观测时间
    integrity_state TEXT NOT NULL CHECK(integrity_state IN ('intact','lost','unknown')), -- 当时完整性，不以缺行伪装正常
    FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE RESTRICT,
    FOREIGN KEY(source_id,session_id) REFERENCES capture_sources(source_id,session_id) ON DELETE RESTRICT,
    UNIQUE(event_id,session_id),
    UNIQUE(session_id,wire_sequence_u64),
    UNIQUE(session_id,ingest_sequence),
    CHECK(event_kind<>'text' OR source_id IS NOT NULL)
) STRICT;
```

### reading_unit_revisions

不可变正文版本；译文/词法/学习固定引用此版本。Owner：AP；范围：FP-04, FP-07, FP-08, FP-09, FP-10；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `unit_id` | TEXT | 否 | 1 | — | 稳定阅读单元 |
| `text_revision` | INTEGER | 否 | 2 | — | 原文变动即新版本 |
| `session_id` | TEXT | 否 | — | — | 复合外键保护所属会话 |
| `source_text` | TEXT | 否 | — | — | 权威纯原文，不读ruby DOM |
| `source_byte_length` | INTEGER | 否 | — | — | UTF8字节长度 |
| `source_sha256` | TEXT | 否 | — | — | UTF8原文摘要，Rust验证真实摘要 |
| `source_language` | TEXT | 否 | — | — | 已声明/检测语言；非日语可降级 |
| `processing_revision` | TEXT | 否 | — | — | 分段/去重/净化规则版本 |
| `processing_snapshot_json` | TEXT | 否 | — | — | 可追溯处理规则，不删除合法重复台词 |
| `selection_revision` | INTEGER | 是 | — | — | Hook所用选择版本，manual为空 |
| `created_at_ms` | INTEGER | 否 | — | — | 版本形成时间 |

外键：

- `(session_id, selection_revision) → source_selections(session_id, selection_revision)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(unit_id, session_id) → reading_units(unit_id, session_id)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `ix_reading_session_history (session_id, created_at_ms, unit_id, text_revision)`；origin=c。

```sql
CREATE INDEX ix_reading_session_history ON reading_unit_revisions(session_id,created_at_ms DESC,unit_id,text_revision);
```
- `sqlite_autoindex_reading_unit_revisions_1 (unit_id, text_revision)`；UNIQUE; origin=pk。
- `sqlite_autoindex_reading_unit_revisions_2 (unit_id, text_revision, session_id)`；UNIQUE; origin=u。

触发器：`immutable_reading_unit_revisions`、`reading_source_kind`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE reading_unit_revisions (
    unit_id TEXT NOT NULL, -- 稳定阅读单元
    text_revision INTEGER NOT NULL CHECK(text_revision>0), -- 原文变动即新版本
    session_id TEXT NOT NULL, -- 复合外键保护所属会话
    source_text TEXT NOT NULL, -- 权威纯原文，不读ruby DOM
    source_byte_length INTEGER NOT NULL CHECK(source_byte_length=length(CAST(source_text AS BLOB))), -- UTF8字节长度
    source_sha256 TEXT NOT NULL CHECK(instr(source_sha256,char(0))=0 AND (source_sha256 IS NULL OR (length(source_sha256)=64 AND source_sha256 NOT GLOB '*[^0-9a-f]*'))), -- UTF8原文摘要，Rust验证真实摘要
    source_language TEXT NOT NULL, -- 已声明/检测语言；非日语可降级
    processing_revision TEXT NOT NULL, -- 分段/去重/净化规则版本
    processing_snapshot_json TEXT NOT NULL CHECK(instr(processing_snapshot_json,char(0))=0 AND (json_valid(processing_snapshot_json) AND json_type(processing_snapshot_json)='object')), -- 可追溯处理规则，不删除合法重复台词
    selection_revision INTEGER, -- Hook所用选择版本，manual为空
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms IS NULL OR created_at_ms>=0), -- 版本形成时间
    PRIMARY KEY(unit_id,text_revision),
    UNIQUE(unit_id,text_revision,session_id),
    FOREIGN KEY(unit_id,session_id) REFERENCES reading_units(unit_id,session_id) ON DELETE CASCADE,
    FOREIGN KEY(session_id,selection_revision) REFERENCES source_selections(session_id,selection_revision) ON DELETE RESTRICT
) STRICT;
```

### reading_units

稳定阅读单元；当前文本版本由最大revision推导。Owner：AP；范围：FP-04, FP-09, FP-10；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `unit_id` | TEXT | 否 | 1 | — | 稳定阅读单元ID |
| `session_id` | TEXT | 否 | — | — | 所属Hook/手动会话 |
| `unit_ordinal` | INTEGER | 否 | — | — | 会话内阅读顺序 |
| `state` | TEXT | 否 | — | — | 增量聚合中或已结束 |
| `created_at_ms` | INTEGER | 否 | — | — | 首个版本时间 |

外键：

- `(session_id) → sessions(session_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_reading_units_1 (unit_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_reading_units_2 (unit_id, session_id)`；UNIQUE; origin=u。
- `sqlite_autoindex_reading_units_3 (session_id, unit_ordinal)`；UNIQUE; origin=u。

触发器：无。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE reading_units (
    unit_id TEXT NOT NULL PRIMARY KEY, -- 稳定阅读单元ID
    session_id TEXT NOT NULL, -- 所属Hook/手动会话
    unit_ordinal INTEGER NOT NULL CHECK(unit_ordinal>0), -- 会话内阅读顺序
    state TEXT NOT NULL CHECK(state IN ('open','finalized')), -- 增量聚合中或已结束
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms IS NULL OR created_at_ms>=0), -- 首个版本时间
    FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE RESTRICT,
    UNIQUE(unit_id,session_id),
    UNIQUE(session_id,unit_ordinal)
) STRICT;
```

### request_attempts

有限重试/回退的真实尝试身份，不将多结果等同回退。Owner：AI；范围：FP-06, FP-08, FP-12；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `attempt_id` | TEXT | 否 | 1 | — | 每次实际尝试独立ID |
| `task_id` | TEXT | 否 | — | — | 逻辑任务 |
| `attempt_number` | INTEGER | 否 | — | — | 有限尝试编号 |
| `config_revision_id` | TEXT | 否 | — | — | 任务冻结配置 |
| `binding_id` | TEXT | 否 | — | — | 绑定 |
| `purpose` | TEXT | 否 | — | — | 用途 |
| `actual_engine_id` | TEXT | 否 | — | — | 实际执行slot；回退必须已明确配置 |
| `credential_id` | TEXT | 是 | — | — | 本次所用不可变凭据引用身份；无认证为NULL |
| `dispatch_kind` | TEXT | 否 | — | — | 真实路径 |
| `state` | TEXT | 否 | — | — | 尝试状态 |
| `error_code` | TEXT | 是 | — | — | 脱敏稳定错误码，不存原HTTP header |
| `finish_reason` | TEXT | 是 | — | — | 完成/拒绝/截断等真实原因 |
| `usage_json` | TEXT | 否 | — | — | 服务报告用量与明确unknown，不伪造token数 |
| `started_at_ms` | INTEGER | 否 | — | — | 尝试开始 |
| `finished_at_ms` | INTEGER | 是 | — | — | 尝试结束 |

外键：

- `(credential_id) → credential_refs(credential_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(config_revision_id, binding_id, actual_engine_id, purpose) → binding_engines(revision_id, binding_id, engine_id, purpose)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(task_id, config_revision_id, binding_id, purpose) → request_tasks(task_id, config_revision_id, binding_id, purpose)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_request_attempts_1 (attempt_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_request_attempts_2 (task_id, attempt_number)`；UNIQUE; origin=u。
- `sqlite_autoindex_request_attempts_3 (attempt_id, task_id)`；UNIQUE; origin=u。

触发器：`attempt_identity_and_transition`、`attempt_initial_state`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE request_attempts (
    attempt_id TEXT NOT NULL PRIMARY KEY, -- 每次实际尝试独立ID
    task_id TEXT NOT NULL, -- 逻辑任务
    attempt_number INTEGER NOT NULL CHECK(attempt_number>0), -- 有限尝试编号
    config_revision_id TEXT NOT NULL, -- 任务冻结配置
    binding_id TEXT NOT NULL, -- 绑定
    purpose TEXT NOT NULL CHECK(purpose IN ('translation','grammar','context_word')), -- 用途
    actual_engine_id TEXT NOT NULL, -- 实际执行slot；回退必须已明确配置
    credential_id TEXT, -- 本次所用不可变凭据引用身份；无认证为NULL
    dispatch_kind TEXT NOT NULL CHECK(dispatch_kind IN ('initial','retry','fallback','cache')), -- 真实路径
    state TEXT NOT NULL CHECK(state IN ('started','completed','failed','cancelled')), -- 尝试状态
    error_code TEXT, -- 脱敏稳定错误码，不存原HTTP header
    finish_reason TEXT, -- 完成/拒绝/截断等真实原因
    usage_json TEXT NOT NULL CHECK(instr(usage_json,char(0))=0 AND (json_valid(usage_json) AND json_type(usage_json)='object')), -- 服务报告用量与明确unknown，不伪造token数
    started_at_ms INTEGER NOT NULL CHECK(started_at_ms IS NULL OR started_at_ms>=0), -- 尝试开始
    finished_at_ms INTEGER CHECK(finished_at_ms IS NULL OR finished_at_ms>=0), -- 尝试结束
    FOREIGN KEY(task_id,config_revision_id,binding_id,purpose) REFERENCES request_tasks(task_id,config_revision_id,binding_id,purpose) ON DELETE CASCADE,
    FOREIGN KEY(config_revision_id,binding_id,actual_engine_id,purpose) REFERENCES binding_engines(revision_id,binding_id,engine_id,purpose) ON DELETE RESTRICT,
    FOREIGN KEY(credential_id) REFERENCES credential_refs(credential_id) ON DELETE RESTRICT,
    UNIQUE(task_id,attempt_number),
    UNIQUE(attempt_id,task_id)
) STRICT;
```

### request_tasks

运行时翻译/精读/语境查词任务，身份与授权固定，配置测试另表。Owner：AI；范围：FP-04, FP-06, FP-08, FP-09；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `task_id` | TEXT | 否 | 1 | — | 任务ID |
| `unit_id` | TEXT | 否 | — | — | 固定阅读单元 |
| `text_revision` | INTEGER | 否 | — | — | 固定原文版本 |
| `session_id` | TEXT | 否 | — | — | 固定所属会话 |
| `scope_key` | TEXT | 否 | — | — | 用途有效作用域 |
| `purpose` | TEXT | 否 | — | — | 用途 |
| `config_revision_id` | TEXT | 否 | — | — | 请求冻结配置快照 |
| `binding_id` | TEXT | 否 | — | — | 具体用途绑定 |
| `engine_id` | TEXT | 否 | — | — | 请求目标引擎slot，学习用途至多一个 |
| `profile_generation` | INTEGER | 否 | — | — | 创建时本档激活代次 |
| `server_instance_id` | TEXT | 否 | — | — | 创建任务的服务实例 |
| `authorization_epoch` | INTEGER | 否 | — | — | 用途授权epoch快照 |
| `request_semantics_sha256` | TEXT | 否 | — | — | 输入/上下文/配置/模板/术语等规范实际语义 |
| `input_snapshot_json` | TEXT | 否 | — | — | 明确允许发送的输入/上下文/词典/术语引用与版本，无秘密 |
| `state` | TEXT | 否 | — | — | 持久状态；取消和完成通过同writer竞争 |
| `invalidation_code` | TEXT | 是 | — | — | 停用/撤销/切档等稳定失效原因 |
| `created_at_ms` | INTEGER | 否 | — | — | 排队时间 |
| `finished_at_ms` | INTEGER | 是 | — | — | 终态时间 |

外键：

- `(config_revision_id, binding_id, engine_id, purpose) → binding_engines(revision_id, binding_id, engine_id, purpose)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(scope_key, purpose) → purpose_runtime_states(scope_key, purpose)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(unit_id, text_revision, session_id) → reading_unit_revisions(unit_id, text_revision, session_id)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `ix_tasks_cancellable (scope_key, purpose, state, authorization_epoch)`；origin=c。

```sql
CREATE INDEX ix_tasks_cancellable ON request_tasks(scope_key,purpose,state,authorization_epoch);
```
- `ix_tasks_unit (unit_id, text_revision, purpose, created_at_ms)`；origin=c。

```sql
CREATE INDEX ix_tasks_unit ON request_tasks(unit_id,text_revision,purpose,created_at_ms);
```
- `sqlite_autoindex_request_tasks_1 (task_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_request_tasks_2 (task_id, config_revision_id, binding_id, purpose)`；UNIQUE; origin=u。

触发器：`request_tasks_requires_seal_insert`、`request_tasks_requires_seal_update`、`task_identity_and_transition`、`task_must_start_queued`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE request_tasks (
    task_id TEXT NOT NULL PRIMARY KEY, -- 任务ID
    unit_id TEXT NOT NULL, -- 固定阅读单元
    text_revision INTEGER NOT NULL, -- 固定原文版本
    session_id TEXT NOT NULL, -- 固定所属会话
    scope_key TEXT NOT NULL, -- 用途有效作用域
    purpose TEXT NOT NULL CHECK(purpose IN ('translation','grammar','context_word')), -- 用途
    config_revision_id TEXT NOT NULL, -- 请求冻结配置快照
    binding_id TEXT NOT NULL, -- 具体用途绑定
    engine_id TEXT NOT NULL, -- 请求目标引擎slot，学习用途至多一个
    profile_generation INTEGER NOT NULL CHECK(profile_generation>0), -- 创建时本档激活代次
    server_instance_id TEXT NOT NULL, -- 创建任务的服务实例
    authorization_epoch INTEGER NOT NULL CHECK(authorization_epoch>0), -- 用途授权epoch快照
    request_semantics_sha256 TEXT NOT NULL CHECK(instr(request_semantics_sha256,char(0))=0 AND (request_semantics_sha256 IS NULL OR (length(request_semantics_sha256)=64 AND request_semantics_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 输入/上下文/配置/模板/术语等规范实际语义
    input_snapshot_json TEXT NOT NULL CHECK(instr(input_snapshot_json,char(0))=0 AND (json_valid(input_snapshot_json) AND json_type(input_snapshot_json)='object')), -- 明确允许发送的输入/上下文/词典/术语引用与版本，无秘密
    state TEXT NOT NULL CHECK(state IN ('queued','running','completed','failed','cancelled','invalidated')), -- 持久状态；取消和完成通过同writer竞争
    invalidation_code TEXT, -- 停用/撤销/切档等稳定失效原因
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms IS NULL OR created_at_ms>=0), -- 排队时间
    finished_at_ms INTEGER CHECK(finished_at_ms IS NULL OR finished_at_ms>=0), -- 终态时间
    FOREIGN KEY(unit_id,text_revision,session_id) REFERENCES reading_unit_revisions(unit_id,text_revision,session_id) ON DELETE CASCADE,
    FOREIGN KEY(scope_key,purpose) REFERENCES purpose_runtime_states(scope_key,purpose) ON DELETE RESTRICT,
    FOREIGN KEY(config_revision_id,binding_id,engine_id,purpose) REFERENCES binding_engines(revision_id,binding_id,engine_id,purpose) ON DELETE RESTRICT,
    UNIQUE(task_id,config_revision_id,binding_id,purpose)
) STRICT;
```

### resource_snapshots

安装包实际资源manifest的不可变快照。Owner：QA；范围：FP-03, FP-07, FP-11, FP-12；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `resource_snapshot_id` | TEXT | 否 | 1 | — | 资源快照ID |
| `kind` | TEXT | 否 | — | — | 资源类别 |
| `version_label` | TEXT | 否 | — | — | 显示/来源版本 |
| `manifest_sha256` | TEXT | 否 | — | — | 整套资源manifest身份 |
| `contract_version` | TEXT | 否 | — | — | ABI或语言/索引schema版本 |
| `metadata_json` | TEXT | 否 | — | — | 平台/架构/来源/许可与条目索引，不含资源正文 |
| `recorded_at_ms` | INTEGER | 否 | — | — | 首次核验时间 |

外键：

无。

索引：

- `sqlite_autoindex_resource_snapshots_1 (resource_snapshot_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_resource_snapshots_2 (kind, manifest_sha256)`；UNIQUE; origin=u。

触发器：`immutable_resource_snapshots`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE resource_snapshots (
    resource_snapshot_id TEXT NOT NULL PRIMARY KEY, -- 资源快照ID
    kind TEXT NOT NULL CHECK(kind IN ('core','language','dictionary')), -- 资源类别
    version_label TEXT NOT NULL, -- 显示/来源版本
    manifest_sha256 TEXT NOT NULL CHECK(instr(manifest_sha256,char(0))=0 AND (manifest_sha256 IS NULL OR (length(manifest_sha256)=64 AND manifest_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 整套资源manifest身份
    contract_version TEXT NOT NULL, -- ABI或语言/索引schema版本
    metadata_json TEXT NOT NULL CHECK(instr(metadata_json,char(0))=0 AND (json_valid(metadata_json) AND json_type(metadata_json)='object')), -- 平台/架构/来源/许可与条目索引，不含资源正文
    recorded_at_ms INTEGER NOT NULL CHECK(recorded_at_ms IS NULL OR recorded_at_ms>=0), -- 首次核验时间
    UNIQUE(kind,manifest_sha256)
) STRICT;
```

### schema_migrations

模式版本与已执行迁移身份；无产品运行记录种子。Owner：AP；范围：FP-01, FP-12；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `version` | INTEGER | 否 | 1 | — | 递增模式版本 |
| `migration_sha256` | TEXT | 否 | — | — | 迁移源校验值 |
| `applied_at_ms` | INTEGER | 否 | — | — | 事务提交时间 |
| `app_build` | TEXT | 否 | — | — | 执行此迁移的实际包版本 |
| `previous_version` | INTEGER | 否 | — | — | 预检的前置版本 |

外键：

无。

索引：

无独立索引（INTEGER PRIMARY KEY 可由 rowid 实现）。

触发器：无。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE schema_migrations (
    version INTEGER NOT NULL PRIMARY KEY CHECK(version>0), -- 递增模式版本
    migration_sha256 TEXT NOT NULL CHECK(instr(migration_sha256,char(0))=0 AND (migration_sha256 IS NULL OR (length(migration_sha256)=64 AND migration_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 迁移源校验值
    applied_at_ms INTEGER NOT NULL CHECK(applied_at_ms IS NULL OR applied_at_ms>=0), -- 事务提交时间
    app_build TEXT NOT NULL, -- 执行此迁移的实际包版本
    previous_version INTEGER NOT NULL CHECK(previous_version>=0) -- 预检的前置版本
) STRICT;
```

### sessions

Hook或手动来源会话，固定启动时的身份。Owner：AP；范围：FP-03, FP-04, FP-05, FP-09, FP-10；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `session_id` | TEXT | 否 | 1 | — | 会话UUID，broker重建/来源切换不复用 |
| `game_id` | TEXT | 是 | — | — | 手动输入可无游戏；库移出不擦历史 |
| `source_kind` | TEXT | 否 | — | — | 来源类别 |
| `state` | TEXT | 否 | — | — | 持久终态/恢复标记；不是运行进程存活保证 |
| `environment_key` | TEXT | 是 | — | — | Hook实际Bottle身份，manual可NULL |
| `target_exe_path` | TEXT | 是 | — | — | 当时核验EXE路径快照 |
| `windows_pid` | INTEGER | 是 | — | — | 32位PID，不能单独作为身份 |
| `process_created_u64` | TEXT | 是 | — | — | Hook必填的规范uint64 FILETIME字符串，manual可NULL |
| `architecture` | TEXT | 是 | — | — | 目标位数快照 |
| `broker_instance_id` | TEXT | 是 | — | — | 此Hook连接的broker实例，manual为NULL |
| `core_resource_snapshot_id` | TEXT | 是 | — | — | 实际Core资源快照 |
| `target_identity_sha256` | TEXT | 是 | — | — | 环境+路径+PID+创建时间+位数规范身份 |
| `started_at_ms` | INTEGER | 否 | — | — | 创建时间 |
| `ended_at_ms` | INTEGER | 是 | — | — | 已结束时间 |

外键：

- `(core_resource_snapshot_id) → resource_snapshots(resource_snapshot_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(game_id) → games(game_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `ix_sessions_game_started (game_id, started_at_ms, session_id)`；origin=c。

```sql
CREATE INDEX ix_sessions_game_started ON sessions(game_id,started_at_ms DESC,session_id);
```
- `sqlite_autoindex_sessions_1 (session_id)`；UNIQUE; origin=pk。

触发器：`session_resource_kind`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE sessions (
    session_id TEXT NOT NULL PRIMARY KEY, -- 会话UUID，broker重建/来源切换不复用
    game_id TEXT, -- 手动输入可无游戏；库移出不擦历史
    source_kind TEXT NOT NULL CHECK(source_kind IN ('hook','manual')), -- 来源类别
    state TEXT NOT NULL CHECK(state IN ('opening','active','stopped','failed','crashed')), -- 持久终态/恢复标记；不是运行进程存活保证
    environment_key TEXT, -- Hook实际Bottle身份，manual可NULL
    target_exe_path TEXT, -- 当时核验EXE路径快照
    windows_pid INTEGER CHECK(windows_pid IS NULL OR windows_pid BETWEEN 1 AND 4294967295), -- 32位PID，不能单独作为身份
    process_created_u64 TEXT, -- Hook必填的规范uint64 FILETIME字符串，manual可NULL
    architecture TEXT CHECK(architecture IS NULL OR architecture IN ('x86','x64')), -- 目标位数快照
    broker_instance_id TEXT, -- 此Hook连接的broker实例，manual为NULL
    core_resource_snapshot_id TEXT, -- 实际Core资源快照
    target_identity_sha256 TEXT CHECK(instr(target_identity_sha256,char(0))=0 AND (target_identity_sha256 IS NULL OR (length(target_identity_sha256)=64 AND target_identity_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 环境+路径+PID+创建时间+位数规范身份
    started_at_ms INTEGER NOT NULL CHECK(started_at_ms IS NULL OR started_at_ms>=0), -- 创建时间
    ended_at_ms INTEGER CHECK(ended_at_ms IS NULL OR ended_at_ms>=0), -- 已结束时间
    FOREIGN KEY(game_id) REFERENCES games(game_id) ON DELETE RESTRICT,
    FOREIGN KEY(core_resource_snapshot_id) REFERENCES resource_snapshots(resource_snapshot_id) ON DELETE RESTRICT,
    CHECK((source_kind='manual' AND windows_pid IS NULL AND broker_instance_id IS NULL) OR (source_kind='hook' AND environment_key IS NOT NULL AND target_exe_path IS NOT NULL AND windows_pid IS NOT NULL AND process_created_u64 IS NOT NULL AND architecture IS NOT NULL AND broker_instance_id IS NOT NULL AND core_resource_snapshot_id IS NOT NULL AND target_identity_sha256 IS NOT NULL)),
    CHECK(instr(process_created_u64,char(0))=0 AND (process_created_u64 IS NULL OR (length(process_created_u64) BETWEEN 1 AND 20 AND process_created_u64 NOT GLOB '*[^0-9]*' AND (length(process_created_u64)=1 OR substr(process_created_u64,1,1)<>'0') AND (length(process_created_u64)<20 OR process_created_u64<='18446744073709551615'))))
) STRICT;
```

### source_refs

正文版本到原始事件的可追溯跨度，payload清理后身份仍存在。Owner：AP；范围：FP-04, FP-09；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `unit_id` | TEXT | 否 | 1 | — | 固定正文 |
| `text_revision` | INTEGER | 否 | 2 | — | 固定正文版本 |
| `session_id` | TEXT | 否 | — | — | 同时约束事件和正文的会话 |
| `ref_ordinal` | INTEGER | 否 | 3 | — | 重建次序 |
| `event_id` | TEXT | 否 | — | — | 原始事件身份 |
| `byte_start` | INTEGER | 否 | — | — | 事件payload起始UTF8字节 |
| `byte_end` | INTEGER | 否 | — | — | 事件payload结束字节，Rust另验边界 |

外键：

- `(event_id, session_id) → raw_events(event_id, session_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(unit_id, text_revision, session_id) → reading_unit_revisions(unit_id, text_revision, session_id)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `ix_source_refs_event (event_id, session_id)`；origin=c。

```sql
CREATE INDEX ix_source_refs_event ON source_refs(event_id,session_id);
```
- `sqlite_autoindex_source_refs_1 (unit_id, text_revision, ref_ordinal)`；UNIQUE; origin=pk。

触发器：`immutable_source_refs`、`source_ref_bounds`、`source_ref_selected_source`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE source_refs (
    unit_id TEXT NOT NULL, -- 固定正文
    text_revision INTEGER NOT NULL, -- 固定正文版本
    session_id TEXT NOT NULL, -- 同时约束事件和正文的会话
    ref_ordinal INTEGER NOT NULL CHECK(ref_ordinal>=0), -- 重建次序
    event_id TEXT NOT NULL, -- 原始事件身份
    byte_start INTEGER NOT NULL CHECK(byte_start>=0), -- 事件payload起始UTF8字节
    byte_end INTEGER NOT NULL CHECK(byte_end>=byte_start), -- 事件payload结束字节，Rust另验边界
    PRIMARY KEY(unit_id,text_revision,ref_ordinal),
    FOREIGN KEY(unit_id,text_revision,session_id) REFERENCES reading_unit_revisions(unit_id,text_revision,session_id) ON DELETE CASCADE,
    FOREIGN KEY(event_id,session_id) REFERENCES raw_events(event_id,session_id) ON DELETE RESTRICT
) STRICT;
```

### source_selections

用户显式选正文的不可变版本，临时选择与保存规则分离。Owner：AP；范围：FP-03, FP-04, FP-05；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `session_id` | TEXT | 否 | 1 | — | 所属会话 |
| `selection_revision` | INTEGER | 否 | 2 | — | 本会话选择版本 |
| `source_id` | TEXT | 否 | — | — | 选中的完整流身份 |
| `persistence_kind` | TEXT | 否 | — | — | 临时选中或明确保存匹配快照 |
| `match_snapshot_json` | TEXT | 否 | — | — | 保存时的规则/身份说明，非固定索引自动选择 |
| `selected_at_ms` | INTEGER | 否 | — | — | 确认选择时间 |

外键：

- `(source_id, session_id) → capture_sources(source_id, session_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(session_id) → sessions(session_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_source_selections_1 (session_id, selection_revision)`；UNIQUE; origin=pk。

触发器：`immutable_source_selections`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE source_selections (
    session_id TEXT NOT NULL, -- 所属会话
    selection_revision INTEGER NOT NULL CHECK(selection_revision>0), -- 本会话选择版本
    source_id TEXT NOT NULL, -- 选中的完整流身份
    persistence_kind TEXT NOT NULL CHECK(persistence_kind IN ('temporary','saved_snapshot')), -- 临时选中或明确保存匹配快照
    match_snapshot_json TEXT NOT NULL CHECK(instr(match_snapshot_json,char(0))=0 AND (json_valid(match_snapshot_json) AND json_type(match_snapshot_json)='object')), -- 保存时的规则/身份说明，非固定索引自动选择
    selected_at_ms INTEGER NOT NULL CHECK(selected_at_ms IS NULL OR selected_at_ms>=0), -- 确认选择时间
    PRIMARY KEY(session_id,selection_revision),
    FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE RESTRICT,
    FOREIGN KEY(source_id,session_id) REFERENCES capture_sources(source_id,session_id) ON DELETE RESTRICT
) STRICT;
```

### study_results

固定句/选词的按需解释终结果，失败/取消不伪装完成。Owner：AI；范围：FP-07, FP-08, FP-09；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `study_result_id` | TEXT | 否 | 1 | — | 学习结果ID |
| `task_id` | TEXT | 否 | — | — | 固定输入任务 |
| `attempt_id` | TEXT | 否 | — | — | 实际胜出尝试 |
| `output_mode` | TEXT | 否 | — | — | 文字模式不画精确引用高亮 |
| `explanation_text` | TEXT | 否 | — | — | 可读解释 |
| `structured_json` | TEXT | 否 | — | — | 已校验结构化输出；文字模式用空object |
| `output_schema_version` | TEXT | 是 | — | — | 结构化校验schema，文字模式可NULL |
| `selection_byte_start` | INTEGER | 是 | — | — | 选词相对固定原文的UTF8跨度 |
| `selection_byte_end` | INTEGER | 是 | — | — | 选词结束字节 |
| `ambiguity_json` | TEXT | 否 | — | '[]' | 省略/歧义与引用证据，不存隐藏思维链 |
| `created_at_ms` | INTEGER | 否 | — | — | 合法终结果提交时间 |

外键：

- `(attempt_id, task_id) → request_attempts(attempt_id, task_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(task_id) → request_tasks(task_id)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_study_results_1 (study_result_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_study_results_2 (task_id)`；UNIQUE; origin=u。

触发器：`immutable_study_results`、`study_results_connection_authority`、`study_task_authority`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE study_results (
    study_result_id TEXT NOT NULL PRIMARY KEY, -- 学习结果ID
    task_id TEXT NOT NULL UNIQUE, -- 固定输入任务
    attempt_id TEXT NOT NULL, -- 实际胜出尝试
    output_mode TEXT NOT NULL CHECK(output_mode IN ('structured','plain_text')), -- 文字模式不画精确引用高亮
    explanation_text TEXT NOT NULL, -- 可读解释
    structured_json TEXT NOT NULL CHECK(instr(structured_json,char(0))=0 AND (json_valid(structured_json) AND json_type(structured_json)='object')), -- 已校验结构化输出；文字模式用空object
    output_schema_version TEXT, -- 结构化校验schema，文字模式可NULL
    selection_byte_start INTEGER CHECK(selection_byte_start IS NULL OR selection_byte_start>=0), -- 选词相对固定原文的UTF8跨度
    selection_byte_end INTEGER, -- 选词结束字节
    ambiguity_json TEXT NOT NULL DEFAULT '[]' CHECK(instr(ambiguity_json,char(0))=0 AND (json_valid(ambiguity_json) AND json_type(ambiguity_json)='array')), -- 省略/歧义与引用证据，不存隐藏思维链
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms IS NULL OR created_at_ms>=0), -- 合法终结果提交时间
    FOREIGN KEY(task_id) REFERENCES request_tasks(task_id) ON DELETE CASCADE,
    FOREIGN KEY(attempt_id,task_id) REFERENCES request_attempts(attempt_id,task_id) ON DELETE RESTRICT,
    CHECK((selection_byte_start IS NULL AND selection_byte_end IS NULL) OR (selection_byte_start IS NOT NULL AND selection_byte_end IS NOT NULL AND selection_byte_end>selection_byte_start)),
    CHECK(output_mode<>'structured' OR output_schema_version IS NOT NULL)
) STRICT;
```

### templates

按用途不可变模板内容、变量目录与兼容来源。Owner：AP；范围：FP-06, FP-08；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 所属不可变整体配置版本 |
| `template_id` | TEXT | 否 | 2 | — | 跨整体版本稳定的模板ID；复制生成独立ID |
| `purpose` | TEXT | 否 | — | — | 模板适用用途，不跨用途偷用输出契约 |
| `display_name` | TEXT | 否 | — | — | 模板显示名 |
| `source_kind` | TEXT | 否 | — | — | 内置、自定义、复制或导入来源 |
| `source_version` | TEXT | 是 | — | — | 内置或专用模板的来源版本；未知可空而不冒称已适配 |
| `template_body_json` | TEXT | 否 | — | — | system/user/prefill等用途模板结构，不含凭据 |
| `template_variables_json` | TEXT | 否 | — | '[]' | 声明变量、必需性和输入资料类别 |
| `compatibility_json` | TEXT | 否 | — | '{}' | 协议/模型/语言/版本匹配声明及来源 |
| `content_hash` | TEXT | 否 | — | — | 规范化模板语义SHA256 |

外键：

- `(revision_id) → config_revisions(revision_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_templates_1 (revision_id, template_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_templates_2 (revision_id, template_id, purpose)`；UNIQUE; origin=u。

触发器：`templates_no_update`、`templates_sealed_delete`、`templates_sealed_insert`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE templates (
    revision_id TEXT NOT NULL REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 所属不可变整体配置版本
    template_id TEXT NOT NULL CHECK (length(template_id) > 0), -- 跨整体版本稳定的模板ID；复制生成独立ID
    purpose TEXT NOT NULL CHECK (purpose IN ('translation','grammar','context_word')), -- 模板适用用途，不跨用途偷用输出契约
    display_name TEXT NOT NULL, -- 模板显示名
    source_kind TEXT NOT NULL CHECK (source_kind IN ('builtin','custom','copied','imported')), -- 内置、自定义、复制或导入来源
    source_version TEXT, -- 内置或专用模板的来源版本；未知可空而不冒称已适配
    template_body_json TEXT NOT NULL CHECK (instr(template_body_json,char(0))=0 AND (json_valid(template_body_json) AND json_type(template_body_json) = 'object')), -- system/user/prefill等用途模板结构，不含凭据
    template_variables_json TEXT NOT NULL DEFAULT '[]' CHECK (instr(template_variables_json,char(0))=0 AND (json_valid(template_variables_json) AND json_type(template_variables_json) = 'array')), -- 声明变量、必需性和输入资料类别
    compatibility_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(compatibility_json,char(0))=0 AND (json_valid(compatibility_json) AND json_type(compatibility_json) = 'object')), -- 协议/模型/语言/版本匹配声明及来源
    content_hash TEXT NOT NULL CHECK (instr(content_hash,char(0))=0 AND (length(content_hash) = 64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 规范化模板语义SHA256
    PRIMARY KEY (revision_id, template_id),
    UNIQUE (revision_id, template_id, purpose)
) STRICT;
```

### term_sets

全局与游戏术语集的不可变范围及匹配规则。Owner：AP；范围：FP-06, FP-08, FP-09；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 所属不可变整体配置版本 |
| `term_set_id` | TEXT | 否 | 2 | — | 跨版本稳定的术语集ID |
| `scope_key` | TEXT | 否 | — | — | 显式全局或游戏作用域键 |
| `game_id` | TEXT | 是 | — | — | 游戏术语集引用；全局为空 |
| `display_name` | TEXT | 否 | — | — | 术语集显示名 |
| `enabled` | INTEGER | 否 | — | 1 | 术语集使用意图 |
| `match_rules_json` | TEXT | 否 | — | '{}' | 大小写、边界、优先级和允许用途等匹配规则 |
| `content_hash` | TEXT | 否 | — | — | 术语内容及规则的语义SHA256 |

外键：

- `(game_id) → games(game_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(revision_id) → config_revisions(revision_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_term_sets_1 (revision_id, term_set_id)`；UNIQUE; origin=pk。
- `term_sets_by_scope (revision_id, scope_key, enabled)`；origin=c。

```sql
CREATE INDEX term_sets_by_scope ON term_sets(revision_id, scope_key, enabled);
```

触发器：`term_sets_no_update`、`term_sets_sealed_delete`、`term_sets_sealed_insert`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE term_sets (
    revision_id TEXT NOT NULL REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 所属不可变整体配置版本
    term_set_id TEXT NOT NULL CHECK (length(term_set_id) > 0), -- 跨版本稳定的术语集ID
    scope_key TEXT NOT NULL, -- 显式全局或游戏作用域键
    game_id TEXT REFERENCES games(game_id) ON DELETE RESTRICT, -- 游戏术语集引用；全局为空
    display_name TEXT NOT NULL, -- 术语集显示名
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)), -- 术语集使用意图
    match_rules_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(match_rules_json,char(0))=0 AND (json_valid(match_rules_json) AND json_type(match_rules_json) = 'object')), -- 大小写、边界、优先级和允许用途等匹配规则
    content_hash TEXT NOT NULL CHECK (instr(content_hash,char(0))=0 AND (length(content_hash) = 64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 术语内容及规则的语义SHA256
    PRIMARY KEY (revision_id, term_set_id),
    CHECK ((scope_key = 'global' AND game_id IS NULL) OR (game_id IS NOT NULL AND scope_key = 'game:' || game_id))
) STRICT;
```

### terms

术语项冻结词面、译法、语言与匹配语义。Owner：AP；范围：FP-06, FP-08, FP-09；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `revision_id` | TEXT | 否 | 1 | — | 所属不可变整体配置版本 |
| `term_set_id` | TEXT | 否 | 2 | — | 同版本术语集ID |
| `term_id` | TEXT | 否 | 3 | — | 集合内稳定术语ID |
| `source_text` | TEXT | 是 | — | — | 待匹配词面；不完整草稿允许空值 |
| `target_text` | TEXT | 是 | — | — | 用户指定译法或解释；不完整草稿允许空值 |
| `source_language` | TEXT | 是 | — | — | 源语言约束 |
| `target_language` | TEXT | 是 | — | — | 译文或解释语言约束 |
| `position` | INTEGER | 否 | — | — | 稳定展示与匹配冲突排序 |
| `enabled` | INTEGER | 否 | — | 1 | 该术语项使用意图 |
| `match_metadata_json` | TEXT | 否 | — | '{}' | 非执行性匹配说明和来源归属 |

外键：

- `(revision_id, term_set_id) → term_sets(revision_id, term_set_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_terms_1 (revision_id, term_set_id, term_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_terms_2 (revision_id, term_set_id, position)`；UNIQUE; origin=u。

触发器：`terms_no_update`、`terms_sealed_delete`、`terms_sealed_insert`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE terms (
    revision_id TEXT NOT NULL, -- 所属不可变整体配置版本
    term_set_id TEXT NOT NULL, -- 同版本术语集ID
    term_id TEXT NOT NULL CHECK (length(term_id) > 0), -- 集合内稳定术语ID
    source_text TEXT, -- 待匹配词面；不完整草稿允许空值
    target_text TEXT, -- 用户指定译法或解释；不完整草稿允许空值
    source_language TEXT, -- 源语言约束
    target_language TEXT, -- 译文或解释语言约束
    position INTEGER NOT NULL CHECK (position >= 0), -- 稳定展示与匹配冲突排序
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)), -- 该术语项使用意图
    match_metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(match_metadata_json,char(0))=0 AND (json_valid(match_metadata_json) AND json_type(match_metadata_json) = 'object')), -- 非执行性匹配说明和来源归属
    PRIMARY KEY (revision_id, term_set_id, term_id),
    UNIQUE (revision_id, term_set_id, position),
    FOREIGN KEY (revision_id, term_set_id) REFERENCES term_sets(revision_id, term_set_id) ON DELETE RESTRICT
) STRICT;
```

### test_record_credentials

单次测试实际触及的精确凭据与池成员范围。Owner：AI；范围：FP-06, FP-08, FP-12；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `test_id` | TEXT | 否 | 1 | — | 所属独立测试完成记录 |
| `credential_id` | TEXT | 否 | 2 | — | 实际使用或校验的不可变秘密版本ID，不代表其他Key已测 |
| `pool_id` | TEXT | 是 | — | — | 当时显式选用的池ID；直接凭据本地校验可空 |
| `pool_revision` | INTEGER | 是 | — | — | 当时精确池版本 |
| `member_id` | TEXT | 是 | — | — | 当时池成员ID |
| `masked_identity_snapshot` | TEXT | 否 | — | — | 当时实际选中Key的脱敏身份快照，绝不保存明文 |
| `outcome` | TEXT | 否 | — | — | 此凭据实际检查结果，不能由另一成员的成功推导 |

外键：

- `(pool_id, pool_revision, member_id, credential_id) → credential_pool_members(pool_id, pool_revision, member_id, credential_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(credential_id) → credential_refs(credential_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(test_id) → test_records(test_id)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_test_record_credentials_1 (test_id, credential_id)`；UNIQUE; origin=pk。
- `test_record_credentials_by_credential (credential_id, test_id)`；origin=c。

```sql
CREATE INDEX test_record_credentials_by_credential ON test_record_credentials(credential_id, test_id);
```

触发器：`test_credentials_pool_not_purged`、`test_record_credentials_no_update`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE test_record_credentials (
    test_id TEXT NOT NULL REFERENCES test_records(test_id) ON DELETE CASCADE, -- 所属独立测试完成记录
    credential_id TEXT NOT NULL REFERENCES credential_refs(credential_id) ON DELETE RESTRICT, -- 实际使用或校验的不可变秘密版本ID，不代表其他Key已测
    pool_id TEXT, -- 当时显式选用的池ID；直接凭据本地校验可空
    pool_revision INTEGER CHECK (pool_revision IS NULL OR pool_revision > 0), -- 当时精确池版本
    member_id TEXT, -- 当时池成员ID
    masked_identity_snapshot TEXT NOT NULL, -- 当时实际选中Key的脱敏身份快照，绝不保存明文
    outcome TEXT NOT NULL CHECK (outcome IN ('passed','failed','cancelled','not_attempted')), -- 此凭据实际检查结果，不能由另一成员的成功推导
    PRIMARY KEY (test_id, credential_id),
    FOREIGN KEY (pool_id, pool_revision, member_id, credential_id) REFERENCES credential_pool_members(pool_id, pool_revision, member_id, credential_id) ON DELETE RESTRICT,
    CHECK ((pool_id IS NULL AND pool_revision IS NULL AND member_id IS NULL) OR (pool_id IS NOT NULL AND pool_revision IS NOT NULL AND member_id IS NOT NULL))
) STRICT;
```

### test_records

用户主动测试的独立完成记录与确切范围，不进入游戏正文历史。Owner：AI；范围：FP-06, FP-08, FP-12；STRICT：是；来源：`docs/architecture/database/config.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `test_id` | TEXT | 否 | 1 | — | 单次独立测试ID，不复用正文请求ID |
| `config_revision_id` | TEXT | 否 | — | — | 实际测试的整体草稿或已应用配置版本 |
| `connection_id` | TEXT | 是 | — | — | 明确选择的连接范围；本地校验可空 |
| `model_id` | TEXT | 是 | — | — | 明确选择的模型范围；连接层测试可空 |
| `binding_id` | TEXT | 是 | — | — | 明确选择的用途绑定范围；非用途测试可空 |
| `purpose` | TEXT | 是 | — | — | 指定用途；通过不提升其他用途状态 |
| `stage` | TEXT | 否 | — | — | 本地有效性、认证连接、实际模型、实际用途分层 |
| `request_semantics_hash` | TEXT | 否 | — | — | 实际测试请求/本地校验语义指纹，用于过时判断 |
| `input_scope` | TEXT | 否 | — | — | 自造样例或用户明确选择固定当前句；不默认取游戏台词 |
| `unit_id` | TEXT | 是 | — | — | 用户选择的原句稳定ID；历史明确清理后允许解除引用 |
| `text_revision` | INTEGER | 是 | — | — | 用户固定的确切文本版本 |
| `input_summary_json` | TEXT | 否 | — | '{}' | 样例ID、原文哈希与授权输入范围摘要，不保存默认完整正文 |
| `response_summary_json` | TEXT | 否 | — | '{}' | 协议状态、结果哈希和脱敏概要，不保存原始响应头或Key |
| `output_validation_json` | TEXT | 否 | — | '{}' | 结构、引用、拒绝/截断和用途契约校验结果 |
| `state` | TEXT | 否 | — | — | 已完成测试终态；运行中状态属于受控任务，不能预写通过 |
| `started_at_ms` | INTEGER | 否 | — | — | 测试开始时间，UTC毫秒 |
| `completed_at_ms` | INTEGER | 否 | — | — | 测试完成或终止时间，UTC毫秒 |
| `error_code` | TEXT | 是 | — | — | 稳定错误码；成功时为空，不写原始秘密错误内容 |
| `error_stage` | TEXT | 是 | — | — | 脱敏故障阶段 |

外键：

- `(unit_id, text_revision) → reading_unit_revisions(unit_id, text_revision)`；ON DELETE SET NULL；ON UPDATE NO ACTION。
- `(config_revision_id, binding_id, purpose) → purpose_bindings(revision_id, binding_id, purpose)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(config_revision_id, model_id, connection_id) → models(revision_id, model_id, connection_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(config_revision_id, connection_id) → connections(revision_id, connection_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(config_revision_id) → config_revisions(revision_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_test_records_1 (test_id)`；UNIQUE; origin=pk。
- `test_records_by_connection (config_revision_id, connection_id, stage, completed_at_ms)`；origin=c。

```sql
CREATE INDEX test_records_by_connection ON test_records(config_revision_id, connection_id, stage, completed_at_ms DESC);
```
- `test_records_by_model (config_revision_id, model_id, stage, completed_at_ms)`；origin=c。

```sql
CREATE INDEX test_records_by_model ON test_records(config_revision_id, model_id, stage, completed_at_ms DESC);
```
- `test_records_by_purpose (config_revision_id, binding_id, purpose, completed_at_ms)`；origin=c。

```sql
CREATE INDEX test_records_by_purpose ON test_records(config_revision_id, binding_id, purpose, completed_at_ms DESC);
```
- `test_records_by_semantics (request_semantics_hash, completed_at_ms, test_id)`；origin=c。

```sql
CREATE INDEX test_records_by_semantics ON test_records(request_semantics_hash, completed_at_ms DESC, test_id);
```
- `test_records_by_source (unit_id, text_revision)`；origin=c。

```sql
CREATE INDEX test_records_by_source ON test_records(unit_id, text_revision);
```

触发器：`test_records_immutable_result`、`test_records_requires_seal_insert`、`test_records_requires_seal_update`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE test_records (
    test_id TEXT NOT NULL PRIMARY KEY CHECK (length(test_id) > 0), -- 单次独立测试ID，不复用正文请求ID
    config_revision_id TEXT NOT NULL REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 实际测试的整体草稿或已应用配置版本
    connection_id TEXT, -- 明确选择的连接范围；本地校验可空
    model_id TEXT, -- 明确选择的模型范围；连接层测试可空
    binding_id TEXT, -- 明确选择的用途绑定范围；非用途测试可空
    purpose TEXT CHECK (purpose IS NULL OR purpose IN ('translation','grammar','context_word')), -- 指定用途；通过不提升其他用途状态
    stage TEXT NOT NULL CHECK (stage IN ('local','connection','model','purpose')), -- 本地有效性、认证连接、实际模型、实际用途分层
    request_semantics_hash TEXT NOT NULL CHECK (instr(request_semantics_hash,char(0))=0 AND (length(request_semantics_hash) = 64 AND request_semantics_hash NOT GLOB '*[^0-9a-f]*')), -- 实际测试请求/本地校验语义指纹，用于过时判断
    input_scope TEXT NOT NULL CHECK (input_scope IN ('sample','selected_current')), -- 自造样例或用户明确选择固定当前句；不默认取游戏台词
    unit_id TEXT, -- 用户选择的原句稳定ID；历史明确清理后允许解除引用
    text_revision INTEGER CHECK (text_revision IS NULL OR text_revision >= 0), -- 用户固定的确切文本版本
    input_summary_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(input_summary_json,char(0))=0 AND (json_valid(input_summary_json) AND json_type(input_summary_json) = 'object')), -- 样例ID、原文哈希与授权输入范围摘要，不保存默认完整正文
    response_summary_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(response_summary_json,char(0))=0 AND (json_valid(response_summary_json) AND json_type(response_summary_json) = 'object')), -- 协议状态、结果哈希和脱敏概要，不保存原始响应头或Key
    output_validation_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(output_validation_json,char(0))=0 AND (json_valid(output_validation_json) AND json_type(output_validation_json) = 'object')), -- 结构、引用、拒绝/截断和用途契约校验结果
    state TEXT NOT NULL CHECK (state IN ('passed','failed','cancelled','timed_out','unsupported')), -- 已完成测试终态；运行中状态属于受控任务，不能预写通过
    started_at_ms INTEGER NOT NULL CHECK (started_at_ms >= 0), -- 测试开始时间，UTC毫秒
    completed_at_ms INTEGER NOT NULL CHECK (completed_at_ms >= started_at_ms), -- 测试完成或终止时间，UTC毫秒
    error_code TEXT, -- 稳定错误码；成功时为空，不写原始秘密错误内容
    error_stage TEXT, -- 脱敏故障阶段
    FOREIGN KEY (config_revision_id, connection_id) REFERENCES connections(revision_id, connection_id) ON DELETE RESTRICT,
    FOREIGN KEY (config_revision_id, model_id, connection_id) REFERENCES models(revision_id, model_id, connection_id) ON DELETE RESTRICT,
    FOREIGN KEY (config_revision_id, binding_id, purpose) REFERENCES purpose_bindings(revision_id, binding_id, purpose) ON DELETE RESTRICT,
    FOREIGN KEY (unit_id, text_revision) REFERENCES reading_unit_revisions(unit_id, text_revision) ON DELETE SET NULL,
    CHECK ((unit_id IS NULL) = (text_revision IS NULL)),
    CHECK (stage = 'local' OR connection_id IS NOT NULL),
    CHECK (stage NOT IN ('model','purpose') OR model_id IS NOT NULL),
    CHECK (stage <> 'purpose' OR (binding_id IS NOT NULL AND purpose IS NOT NULL)),
    CHECK ((binding_id IS NULL) = (purpose IS NULL)),
    CHECK (model_id IS NULL OR connection_id IS NOT NULL),
    CHECK ((state = 'passed' AND error_code IS NULL) OR state <> 'passed')
) STRICT;
```

### translations

完成后的独立译文；输入和配置身份通过task不可歧义追溯。Owner：AI；范围：FP-04, FP-06, FP-09；STRICT：是；来源：`docs/architecture/database/profile.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `translation_id` | TEXT | 否 | 1 | — | 译文结果ID |
| `task_id` | TEXT | 否 | — | — | 每翻译任务一个终结果 |
| `attempt_id` | TEXT | 否 | — | — | 实际胜出的尝试，缓存也显式记录 |
| `translated_text` | TEXT | 否 | — | — | 真实译文，流中临时片段不标为完成 |
| `target_language` | TEXT | 否 | — | — | 实际语言方向 |
| `provider_label_snapshot` | TEXT | 否 | — | — | 当时显示来源，不替代模型身份 |
| `created_at_ms` | INTEGER | 否 | — | — | 合法完成提交时间 |

外键：

- `(attempt_id, task_id) → request_attempts(attempt_id, task_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。
- `(task_id) → request_tasks(task_id)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_translations_1 (translation_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_translations_2 (task_id)`；UNIQUE; origin=u。

触发器：`immutable_translations`、`translation_task_authority`、`translations_connection_authority`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE translations (
    translation_id TEXT NOT NULL PRIMARY KEY, -- 译文结果ID
    task_id TEXT NOT NULL UNIQUE, -- 每翻译任务一个终结果
    attempt_id TEXT NOT NULL, -- 实际胜出的尝试，缓存也显式记录
    translated_text TEXT NOT NULL, -- 真实译文，流中临时片段不标为完成
    target_language TEXT NOT NULL, -- 实际语言方向
    provider_label_snapshot TEXT NOT NULL, -- 当时显示来源，不替代模型身份
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms IS NULL OR created_at_ms>=0), -- 合法完成提交时间
    FOREIGN KEY(task_id) REFERENCES request_tasks(task_id) ON DELETE CASCADE,
    FOREIGN KEY(attempt_id,task_id) REFERENCES request_attempts(attempt_id,task_id) ON DELETE RESTRICT
) STRICT;
```

## dictionary

### dictionary_entries

日语词条身份与中文基础/英语补充的明确区分。Owner：JA；范围：FP-07；STRICT：是；来源：`docs/architecture/database/dictionary.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `entry_id` | TEXT | 否 | 1 | — | 本资源版本内词条ID |
| `source_id` | TEXT | 否 | — | — | 精确来源 |
| `source_entry_key` | TEXT | 否 | — | — | 来源内部稳定键 |
| `lemma` | TEXT | 否 | — | — | 词元 |
| `reading_kana` | TEXT | 是 | — | — | 已知读音，未知保持NULL |
| `pos_json` | TEXT | 否 | — | — | 结构化词性类别 |
| `content_kind` | TEXT | 否 | — | — | 中文基础或明确标注英语补充 |

外键：

- `(source_id) → dictionary_sources(source_id)`；ON DELETE RESTRICT；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_dictionary_entries_1 (entry_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_dictionary_entries_2 (source_id, source_entry_key)`；UNIQUE; origin=u。

触发器：`dictionary_entry_language_update`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE dictionary_entries (
    entry_id TEXT NOT NULL PRIMARY KEY, -- 本资源版本内词条ID
    source_id TEXT NOT NULL REFERENCES dictionary_sources(source_id) ON DELETE RESTRICT, -- 精确来源
    source_entry_key TEXT NOT NULL, -- 来源内部稳定键
    lemma TEXT NOT NULL, -- 词元
    reading_kana TEXT, -- 已知读音，未知保持NULL
    pos_json TEXT NOT NULL CHECK(instr(pos_json,char(0))=0 AND (json_valid(pos_json) AND json_type(pos_json)='array')), -- 结构化词性类别
    content_kind TEXT NOT NULL CHECK(content_kind IN ('zh_base','en_supplement')), -- 中文基础或明确标注英语补充
    UNIQUE(source_id,source_entry_key)
) STRICT;
```

### dictionary_forms

词面/词元/读音查询的B-tree键，未知词不伪造命中。Owner：JA；范围：FP-07；STRICT：是；来源：`docs/architecture/database/dictionary.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `form_id` | TEXT | 否 | 1 | — | 查询词形ID |
| `entry_id` | TEXT | 否 | — | — | 对应词条 |
| `form_text` | TEXT | 否 | — | — | 原词形 |
| `normalized_key` | TEXT | 否 | — | — | 按dictionary_meta规则生成的精确查询键 |
| `form_kind` | TEXT | 否 | — | — | 查询形态类别 |
| `rank_weight` | INTEGER | 否 | — | — | 资源构建确定的排序值 |

外键：

- `(entry_id) → dictionary_entries(entry_id)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `dictionary_forms_lookup (normalized_key, form_kind, rank_weight, entry_id)`；origin=c。

```sql
CREATE INDEX dictionary_forms_lookup ON dictionary_forms(normalized_key,form_kind,rank_weight,entry_id);
```
- `sqlite_autoindex_dictionary_forms_1 (form_id)`；UNIQUE; origin=pk。
- `sqlite_autoindex_dictionary_forms_2 (entry_id, normalized_key, form_kind)`；UNIQUE; origin=u。

触发器：无。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE dictionary_forms (
    form_id TEXT NOT NULL PRIMARY KEY, -- 查询词形ID
    entry_id TEXT NOT NULL REFERENCES dictionary_entries(entry_id) ON DELETE CASCADE, -- 对应词条
    form_text TEXT NOT NULL, -- 原词形
    normalized_key TEXT NOT NULL, -- 按dictionary_meta规则生成的精确查询键
    form_kind TEXT NOT NULL CHECK(form_kind IN ('surface','lemma','reading')), -- 查询形态类别
    rank_weight INTEGER NOT NULL CHECK(rank_weight>=0), -- 资源构建确定的排序值
    UNIQUE(entry_id,normalized_key,form_kind)
) STRICT;
```

### dictionary_meta

单个固定辞书索引及规范化算法的发布身份。Owner：JA；范围：FP-07, FP-11；STRICT：是；来源：`docs/architecture/database/dictionary.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `singleton` | INTEGER | 否 | 1 | — | 唯一元数据行 |
| `resource_version` | TEXT | 否 | — | — | 发布清单中的资源版本 |
| `manifest_sha256` | TEXT | 否 | — | — | 实际资源manifest摘要 |
| `index_schema_version` | INTEGER | 否 | — | — | 查询索引格式版本 |
| `normalization_version` | TEXT | 否 | — | — | 词形/读音键规范化规则版本 |
| `built_at_ms` | INTEGER | 否 | — | — | 离线构建时间UTC毫秒 |

外键：

无。

索引：

无独立索引（INTEGER PRIMARY KEY 可由 rowid 实现）。

触发器：无。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE dictionary_meta (
    singleton INTEGER NOT NULL PRIMARY KEY CHECK(singleton=1), -- 唯一元数据行
    resource_version TEXT NOT NULL, -- 发布清单中的资源版本
    manifest_sha256 TEXT NOT NULL CHECK(instr(manifest_sha256,char(0))=0 AND (length(manifest_sha256)=64 AND manifest_sha256 NOT GLOB '*[^0-9a-f]*')), -- 实际资源manifest摘要
    index_schema_version INTEGER NOT NULL CHECK(index_schema_version>0), -- 查询索引格式版本
    normalization_version TEXT NOT NULL, -- 词形/读音键规范化规则版本
    built_at_ms INTEGER NOT NULL CHECK(built_at_ms>=0) -- 离线构建时间UTC毫秒
) STRICT;
```

### dictionary_senses

释义正文、语言与来源定位，不把英语补充伪称中文释义。Owner：JA；范围：FP-07；STRICT：是；来源：`docs/architecture/database/dictionary.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `entry_id` | TEXT | 否 | 1 | — | 对应词条 |
| `sense_index` | INTEGER | 否 | 2 | — | 词条内释义顺序 |
| `gloss_language` | TEXT | 否 | — | — | 释义语言 |
| `gloss_text` | TEXT | 否 | — | — | 实际离线释义 |
| `usage_notes` | TEXT | 是 | — | — | 用法说明 |
| `source_locator` | TEXT | 否 | — | — | 来源页/条目定位，遵循原数据许可 |

外键：

- `(entry_id) → dictionary_entries(entry_id)`；ON DELETE CASCADE；ON UPDATE NO ACTION。

索引：

- `sqlite_autoindex_dictionary_senses_1 (entry_id, sense_index)`；UNIQUE; origin=pk。

触发器：`dictionary_gloss_language`、`dictionary_gloss_language_update`。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE dictionary_senses (
    entry_id TEXT NOT NULL REFERENCES dictionary_entries(entry_id) ON DELETE CASCADE, -- 对应词条
    sense_index INTEGER NOT NULL CHECK(sense_index>=0), -- 词条内释义顺序
    gloss_language TEXT NOT NULL CHECK(gloss_language IN ('zh','en')), -- 释义语言
    gloss_text TEXT NOT NULL, -- 实际离线释义
    usage_notes TEXT, -- 用法说明
    source_locator TEXT NOT NULL, -- 来源页/条目定位，遵循原数据许可
    PRIMARY KEY(entry_id,sense_index)
) STRICT;
```

### dictionary_sources

每个辞书来源、快照与必须显示的归属许可。Owner：JA；范围：FP-07, FP-11；STRICT：是；来源：`docs/architecture/database/dictionary.sql`。

| 字段 | SQLite类型 | 可空 | 主键序位 | 默认值 | 说明 |
| --- | --- | --- | --- | --- | --- |
| `source_id` | TEXT | 否 | 1 | — | 稳定来源标识 |
| `display_name` | TEXT | 否 | — | — | 用户可见来源名 |
| `snapshot_version` | TEXT | 否 | — | — | 数据快照版本 |
| `source_sha256` | TEXT | 否 | — | — | 原数据身份 |
| `source_url` | TEXT | 否 | — | — | 原来源定位，不在运行时自动联网 |
| `license_ref` | TEXT | 否 | — | — | 包内许可文件引用 |
| `attribution_text` | TEXT | 否 | — | — | 必要归属文字 |

外键：

无。

索引：

- `sqlite_autoindex_dictionary_sources_1 (source_id)`；UNIQUE; origin=pk。

触发器：无。完整定义见 JSON 与源 DDL。

字段声明与表级 CHECK/约束：

```sql
CREATE TABLE dictionary_sources (
    source_id TEXT NOT NULL PRIMARY KEY, -- 稳定来源标识
    display_name TEXT NOT NULL, -- 用户可见来源名
    snapshot_version TEXT NOT NULL, -- 数据快照版本
    source_sha256 TEXT NOT NULL CHECK(instr(source_sha256,char(0))=0 AND (length(source_sha256)=64 AND source_sha256 NOT GLOB '*[^0-9a-f]*')), -- 原数据身份
    source_url TEXT NOT NULL, -- 原来源定位，不在运行时自动联网
    license_ref TEXT NOT NULL, -- 包内许可文件引用
    attribution_text TEXT NOT NULL -- 必要归属文字
) STRICT;
```
