BEGIN TRANSACTION;
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
INSERT INTO "binding_engines" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','z-binding','z-engine','translation','z-model',0,1,NULL);
INSERT INTO "binding_engines" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','z-binding','a-engine','translation','z-model',1,0,NULL);
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
CREATE TABLE body_record_positions (
    record_sequence INTEGER PRIMARY KEY AUTOINCREMENT CHECK(record_sequence>0),
    session_id TEXT NOT NULL,
    unit_id TEXT NOT NULL,
    text_revision INTEGER NOT NULL CHECK(text_revision>0),
    FOREIGN KEY(unit_id,text_revision,session_id) REFERENCES reading_unit_revisions(unit_id,text_revision,session_id) ON DELETE CASCADE,
    UNIQUE(unit_id,text_revision)
) STRICT;
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
CREATE TABLE capture_operations (
    operation_id TEXT NOT NULL PRIMARY KEY CHECK(length(operation_id)>0 AND instr(operation_id,char(0))=0),
    request_sha256 TEXT NOT NULL CHECK(instr(request_sha256,char(0))=0 AND length(request_sha256)=64 AND request_sha256 NOT GLOB '*[^0-9a-f]*'),
    session_id TEXT,
    state TEXT NOT NULL CHECK(state IN ('opening','active','failed','stopped','recovered')),
    started_at_ms INTEGER NOT NULL CHECK(started_at_ms>=0),
    ended_at_ms INTEGER CHECK(ended_at_ms IS NULL OR ended_at_ms>=0),
    legacy_unknown_start INTEGER NOT NULL DEFAULT 0 CHECK(legacy_unknown_start IN (0,1)),
    FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE RESTRICT,
    CHECK((state IN ('opening','failed')) OR session_id IS NOT NULL)
) STRICT;
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
CREATE TABLE config_gc_marks (
    revision_id TEXT NOT NULL PRIMARY KEY REFERENCES config_revision_seals(revision_id) ON DELETE RESTRICT, -- 已验证无外部引用的封存版本
    operation_id TEXT NOT NULL REFERENCES operation_receipts(operation_id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED, -- 与清理同事务提交的操作回执
    requested_at_ms INTEGER NOT NULL CHECK(requested_at_ms>=0) -- 授权保留策略执行时间
) STRICT;
CREATE TABLE config_heads (
    head_kind TEXT NOT NULL PRIMARY KEY CHECK (head_kind IN ('draft','applied')), -- 指针种类；保存草稿不移动applied
    revision_id TEXT NOT NULL REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 当前指向的整体配置版本
    head_revision INTEGER NOT NULL CHECK (head_revision >= 0), -- 乐观并发版本；更新须带expected_revision并原子递增
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= 0) -- 指针最后更新时间，UTC毫秒
) STRICT;
INSERT INTO "config_heads" VALUES('draft','config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd',1,20);
INSERT INTO "config_heads" VALUES('applied','config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd',1,21);
CREATE TABLE config_revision_seals (
    revision_id TEXT NOT NULL PRIMARY KEY REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 被封存的整体配置版本
    content_hash TEXT NOT NULL CHECK(instr(content_hash,char(0))=0 AND (length(content_hash)=64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 与头记录一致；实际规范化内容摘要由Rust计算
    sealed_at_ms INTEGER NOT NULL CHECK(sealed_at_ms>=0) -- 同保存事务的封存时间
) STRICT;
INSERT INTO "config_revision_seals" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','1cf7a481c12155893ba7d16e08abf26c8e01c43e347dab848f1cf95c8e2e0a4b',20);
CREATE TABLE config_revisions (
    revision_id TEXT NOT NULL PRIMARY KEY CHECK (length(revision_id) > 0), -- 不可变整体配置版本ID，与请求语义指纹分开
    parent_revision_id TEXT REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 来源版本；首个版本允许为空
    content_hash TEXT NOT NULL CHECK (instr(content_hash,char(0))=0 AND (length(content_hash) = 64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 规范化非秘密配置内容的SHA256
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0), -- 保存时间，UTC毫秒
    save_reason TEXT NOT NULL CHECK (save_reason IN ('create','edit','copy','import','restore','migrate')), -- 形成独立快照的原因
    local_validation_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(local_validation_json,char(0))=0 AND (json_valid(local_validation_json) AND json_type(local_validation_json) = 'object')), -- 本地缺项与字段错误摘要；不是网络测试通过标记
    CHECK (parent_revision_id IS NULL OR parent_revision_id <> revision_id)
) STRICT;
INSERT INTO "config_revisions" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd',NULL,'1cf7a481c12155893ba7d16e08abf26c8e01c43e347dab848f1cf95c8e2e0a4b',20,'edit','{}');
CREATE TABLE connection_runtime_states (
    connection_id TEXT NOT NULL PRIMARY KEY, -- 稳定连接ID，跨配置版本安全闸
    config_revision_id TEXT NOT NULL, -- 最近关联的已应用连接快照，仅供来源追溯
    is_suspended INTEGER NOT NULL CHECK(is_suspended IN (0,1)), -- 即时禁止新请求并取消在途，不能等待下一次应用
    control_revision INTEGER NOT NULL CHECK(control_revision>0), -- 安全闸CAS版本，显式恢复也递增
    reason_code TEXT, -- 脱敏停用/恢复原因
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms>=0), -- 最后明确控制时间UTC毫秒
    FOREIGN KEY(config_revision_id,connection_id) REFERENCES connections(revision_id,connection_id) ON DELETE RESTRICT
) STRICT;
INSERT INTO "connection_runtime_states" VALUES('a-connection','config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd',0,1,NULL,21);
INSERT INTO "connection_runtime_states" VALUES('z-connection','config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd',0,1,NULL,21);
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
INSERT INTO "connections" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','z-connection','原始🦀','openai_compatible','local','none','http://127.0.0.1:1234/v1',NULL,NULL,'disabled','{"region":"eastasia"}',1);
INSERT INTO "connections" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','a-connection','原始🦀','openai_compatible','local','none',NULL,NULL,NULL,'disabled','{"region":"eastasia"}',0);
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
CREATE TABLE credential_pool_gc_marks (
    pool_id TEXT NOT NULL, -- 清理池ID
    pool_revision INTEGER NOT NULL, -- 清理池版本
    operation_id TEXT NOT NULL REFERENCES operation_receipts(operation_id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED, -- 与清理同事务回执
    requested_at_ms INTEGER NOT NULL CHECK(requested_at_ms>=0), -- 清理时间
    PRIMARY KEY(pool_id,pool_revision),
    FOREIGN KEY(pool_id,pool_revision) REFERENCES credential_pool_seals(pool_id,pool_revision) ON DELETE RESTRICT
) STRICT;
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
INSERT INTO "credential_pool_members" VALUES('raw-pool',1,'member-5b80787b8b3cee7199cae0ecc62fb3532cd458fedf348640be828dcf78b36efd','raw-credential',0,1,1);
CREATE TABLE credential_pool_revisions (
    pool_id TEXT NOT NULL CHECK (length(pool_id) > 0), -- 跨池版本稳定的凭据池ID
    pool_revision INTEGER NOT NULL CHECK (pool_revision > 0), -- 池版本；成员/策略变更生成新版本
    policy TEXT NOT NULL CHECK (policy IN ('selected','round_robin','weighted')), -- 显式单选或用户授权轮询策略；不授权换供应商
    policy_metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(policy_metadata_json,char(0))=0 AND (json_valid(policy_metadata_json) AND json_type(policy_metadata_json) = 'object')), -- 有界退避、错误权重与次数等非秘密策略
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0), -- 池版本保存时间，UTC毫秒
    content_hash TEXT NOT NULL CHECK (instr(content_hash,char(0))=0 AND (length(content_hash) = 64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 池成员与策略的语义SHA256
    PRIMARY KEY (pool_id, pool_revision)
) STRICT;
INSERT INTO "credential_pool_revisions" VALUES('raw-pool',1,'selected','{}',11,'46b61382bc574d465a2c47de900c450cc81587991429f36f404e8b2f0b753334');
CREATE TABLE credential_pool_seals (
    pool_id TEXT NOT NULL, -- 被封存池ID
    pool_revision INTEGER NOT NULL, -- 被封存版本
    content_hash TEXT NOT NULL CHECK(instr(content_hash,char(0))=0 AND (length(content_hash)=64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 与池版本头相同的规范语义摘要
    sealed_at_ms INTEGER NOT NULL CHECK(sealed_at_ms>=0), -- 装配完成封存时间
    PRIMARY KEY(pool_id,pool_revision),
    FOREIGN KEY(pool_id,pool_revision) REFERENCES credential_pool_revisions(pool_id,pool_revision) ON DELETE RESTRICT
) STRICT;
INSERT INTO "credential_pool_seals" VALUES('raw-pool',1,'46b61382bc574d465a2c47de900c450cc81587991429f36f404e8b2f0b753334',11);
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
INSERT INTO "credential_refs" VALUES('raw-credential','vault-raw-json-like','Original raw key fixture','api_key','active',11,11,NULL,NULL,NULL,1);
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
CREATE TABLE hook_events (
    event_sequence INTEGER PRIMARY KEY AUTOINCREMENT CHECK(event_sequence>0),
    event_id TEXT NOT NULL UNIQUE CHECK(length(event_id)>0 AND instr(event_id,char(0))=0),
    operation_id TEXT NOT NULL,
    session_id TEXT,
    source_id TEXT,
    wire_sequence_u64 TEXT CHECK(wire_sequence_u64 IS NULL OR (
      length(wire_sequence_u64) BETWEEN 1 AND 20 AND wire_sequence_u64 NOT GLOB '*[^0-9]*'
      AND (length(wire_sequence_u64)=1 OR substr(wire_sequence_u64,1,1)<>'0')
      AND (length(wire_sequence_u64)<20 OR wire_sequence_u64<='18446744073709551615')
      AND instr(wire_sequence_u64,char(0))=0)),
    observed_at_ms INTEGER NOT NULL CHECK(observed_at_ms>=0),
    level TEXT NOT NULL CHECK(level IN ('info','warning','error')),
    stage TEXT NOT NULL CHECK(stage IN ('association','resource','identity','injection','handshake','stream','selection','capture','disconnect','recovery')),
    code TEXT NOT NULL CHECK(length(code)>0 AND instr(code,char(0))=0),
    message TEXT NOT NULL CHECK(length(message)<=4096 AND instr(message,char(0))=0),
    lost_count_u64 TEXT CHECK(lost_count_u64 IS NULL OR (
      length(lost_count_u64) BETWEEN 1 AND 20 AND lost_count_u64 NOT GLOB '*[^0-9]*'
      AND (length(lost_count_u64)=1 OR substr(lost_count_u64,1,1)<>'0')
      AND (length(lost_count_u64)<20 OR lost_count_u64<='18446744073709551615')
      AND instr(lost_count_u64,char(0))=0)),
    integrity TEXT NOT NULL CHECK(integrity IN ('intact','lost','unknown')),
    metadata_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(metadata_json) AND json_type(metadata_json)='object' AND instr(metadata_json,char(0))=0),
    FOREIGN KEY(operation_id) REFERENCES capture_operations(operation_id) ON DELETE RESTRICT,
    FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE RESTRICT,
    FOREIGN KEY(source_id,session_id) REFERENCES capture_sources(source_id,session_id) ON DELETE RESTRICT,
    CHECK(source_id IS NULL OR session_id IS NOT NULL),
    UNIQUE(operation_id,wire_sequence_u64)
) STRICT;
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
CREATE TABLE model_default_templates (
    revision_id TEXT NOT NULL, -- 所属不可变整体配置版本
    model_id TEXT NOT NULL, -- 同版本模型配置ID
    purpose TEXT NOT NULL CHECK (purpose IN ('translation','grammar','context_word')), -- 独立用途
    template_id TEXT NOT NULL, -- 同版本且同用途模板ID
    PRIMARY KEY (revision_id, model_id, purpose),
    FOREIGN KEY (revision_id, model_id) REFERENCES models(revision_id, model_id) ON DELETE RESTRICT,
    FOREIGN KEY (revision_id, template_id, purpose) REFERENCES templates(revision_id, template_id, purpose) ON DELETE RESTRICT
) STRICT;
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
INSERT INTO "model_parameters" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','z-model','temperature','set','0');
INSERT INTO "model_parameters" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','z-model','top_p','omit',NULL);
INSERT INTO "model_parameters" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','a-model','temperature','set','0');
INSERT INTO "model_parameters" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','a-model','top_p','omit',NULL);
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
INSERT INTO "models" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','z-model','z-connection','test-model','Test','manual','{}');
INSERT INTO "models" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','a-model','a-connection',NULL,'Test','manual','{}');
CREATE TABLE operation_receipts (
    operation_id TEXT NOT NULL PRIMARY KEY, -- 客户端操作幂等ID
    request_sha256 TEXT NOT NULL CHECK(instr(request_sha256,char(0))=0 AND (request_sha256 IS NULL OR (length(request_sha256)=64 AND request_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 相同操作ID的规范请求摘要，冲突拒绝
    command_kind TEXT NOT NULL, -- 已注册命令种类
    commit_revision INTEGER NOT NULL CHECK(commit_revision>0), -- 提交时的本地revision
    result_refs_json TEXT NOT NULL CHECK(instr(result_refs_json,char(0))=0 AND (json_valid(result_refs_json) AND json_type(result_refs_json)='object')), -- 仅结果身份/版本与脱敏回执，不含原始秘密
    committed_at_ms INTEGER NOT NULL CHECK(committed_at_ms IS NULL OR committed_at_ms>=0) -- 提交时间
) STRICT;
INSERT INTO "operation_receipts" VALUES('original-save','2bbe2bb3f86a0f6ae05f9c6df15ea9a46760d7330909823d8b377da838b524e0','save_configuration',3,'{"replayed":false,"requestId":"original-save","revision":3,"resultId":"config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd"}',20);
INSERT INTO "operation_receipts" VALUES('original-apply','c30aefb01098d2f122bec53b1528c42c8bf6f62dc67ba108646ac76b9ca7a9a8','apply_configuration',4,'{"replayed":false,"requestId":"original-apply","revision":4,"resultId":"config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd"}',21);
CREATE TABLE preference_heads (
    scope_key TEXT NOT NULL PRIMARY KEY, -- 偏好作用域
    preference_revision_id TEXT NOT NULL, -- 当前偏好快照
    head_revision INTEGER NOT NULL CHECK(head_revision>0), -- 并发编辑版本
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms IS NULL OR updated_at_ms>=0), -- 更新时间
    FOREIGN KEY(preference_revision_id,scope_key) REFERENCES preference_revisions(preference_revision_id,scope_key) ON DELETE RESTRICT
) STRICT;
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
INSERT INTO "profile_meta" VALUES(1,'legacy-http-profile',4,1,'legacy-writer','active',10,21);
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
INSERT INTO "purpose_bindings" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','z-binding','global',NULL,'translation','override',1,'text','ja','zh-Hans',NULL,'{}','{}');
INSERT INTO "purpose_bindings" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','a-binding','global',NULL,'grammar','override',0,NULL,'ja','zh-Hans',NULL,'{}','{}');
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
INSERT INTO "purpose_runtime_states" VALUES('global','translation','z-binding','config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd',1,1,'53671b140c959e67b345ec030a6d668b4ff6de530feaef3370b0329ab44707b7',21);
INSERT INTO "purpose_runtime_states" VALUES('global','grammar','a-binding','config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd',1,1,'be2dee30abbac4964d7c6e92d453e1903298dba475201ab41402369bb376ba98',21);
CREATE TABLE raw_event_payloads (
    event_id TEXT NOT NULL PRIMARY KEY, -- 对应事件
    payload_bytes BLOB NOT NULL, -- 有界原始字节，可为空BLOB
    retained_at_ms INTEGER NOT NULL CHECK(retained_at_ms IS NULL OR retained_at_ms>=0), -- 写入时间
    FOREIGN KEY(event_id) REFERENCES raw_events(event_id) ON DELETE CASCADE
) STRICT;
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
CREATE TABLE reading_preference_settings (
    preference_revision_id TEXT NOT NULL PRIMARY KEY,
    segmentation_enabled INTEGER NOT NULL DEFAULT 1 CHECK(segmentation_enabled IN (0,1)),
    reading_display_mode TEXT NOT NULL DEFAULT 'both' CHECK(reading_display_mode IN ('both','source','translation') AND instr(reading_display_mode,char(0))=0),
    hud_background_opacity REAL NOT NULL DEFAULT 0.70 CHECK(hud_background_opacity>=0.0 AND hud_background_opacity<=1.0),
    FOREIGN KEY(preference_revision_id) REFERENCES preference_revisions(preference_revision_id) ON DELETE CASCADE
) STRICT;
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
CREATE TABLE schema_migrations (
    version INTEGER NOT NULL PRIMARY KEY CHECK(version>0), -- 递增模式版本
    migration_sha256 TEXT NOT NULL CHECK(instr(migration_sha256,char(0))=0 AND (migration_sha256 IS NULL OR (length(migration_sha256)=64 AND migration_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 迁移源校验值
    applied_at_ms INTEGER NOT NULL CHECK(applied_at_ms IS NULL OR applied_at_ms>=0), -- 事务提交时间
    app_build TEXT NOT NULL, -- 执行此迁移的实际包版本
    previous_version INTEGER NOT NULL CHECK(previous_version>=0) -- 预检的前置版本
) STRICT;
INSERT INTO "schema_migrations" VALUES(1,'1f26620f1b22307ae07851c1e09c7ed44f888f6b9f88e9d9c34b6f7f1f3302c7',10,'0.2.0',0);
INSERT INTO "schema_migrations" VALUES(2,'ec4d1470721b9d81e20a3c5e1bb12aa6f59a2b3c32f6310691f332394781ca2e',10,'0.2.0',1);
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
INSERT INTO "templates" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','z-template','translation','z-template','custom',NULL,'{"user":"{{source}}"}','["source"]','{}','078882c63819ce6989f22b9c0b30c956f9929ccbcf6b08d3540a9af4bf08b8aa');
INSERT INTO "templates" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','a-template','translation','a-template','custom',NULL,'{"user":"{{source}}"}','["source"]','{}','078882c63819ce6989f22b9c0b30c956f9929ccbcf6b08d3540a9af4bf08b8aa');
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
INSERT INTO "term_sets" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','z-set','global',NULL,'z-set',1,'{}','b8b1a0fcbd9b771a47c32bfac84e297c1cf8ee41e0f71d32174d120205442518');
INSERT INTO "term_sets" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','a-set','global',NULL,'a-set',1,'{}','9f551c1ce659dbd7c88a07634ef6e3ad1beb739939b605e26574101dad93ebd0');
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
INSERT INTO "terms" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','z-set','z-set-z-term','𠮷野','吉野',NULL,NULL,0,1,'{}');
INSERT INTO "terms" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','z-set','z-set-a-term','𠮷野','吉野',NULL,NULL,1,1,'{}');
INSERT INTO "terms" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','a-set','a-set-z-term','𠮷野','吉野',NULL,NULL,0,1,'{}');
INSERT INTO "terms" VALUES('config-7f71f134c0fccd7fb6abe6b019dd5e89bce9b6290b8178ae37b47af0d1ce1fdd','a-set','a-set-a-term','𠮷野','吉野',NULL,NULL,1,1,'{}');
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
CREATE INDEX ix_games_active ON games(archived_at_ms, updated_at_ms DESC, game_id);
CREATE UNIQUE INDEX uq_game_target_environment_path ON game_targets(environment_key,target_path_key) WHERE archived_at_ms IS NULL AND target_path_key IS NOT NULL;
CREATE INDEX ix_sessions_game_started ON sessions(game_id,started_at_ms DESC,session_id);
CREATE INDEX ix_raw_source_order ON raw_events(source_id,ingest_sequence);
CREATE INDEX ix_reading_session_history ON reading_unit_revisions(session_id,created_at_ms DESC,unit_id,text_revision);
CREATE INDEX ix_source_refs_event ON source_refs(event_id,session_id);
CREATE INDEX ix_tasks_unit ON request_tasks(unit_id,text_revision,purpose,created_at_ms);
CREATE INDEX ix_tasks_cancellable ON request_tasks(scope_key,purpose,state,authorization_epoch);
CREATE INDEX ix_cache_expiry ON cache_entries(expires_at_ms,cache_key);
CREATE INDEX ix_cache_lru ON cache_entries(last_accessed_at_ms,cache_key);
CREATE TRIGGER raw_payload_length BEFORE INSERT ON raw_event_payloads BEGIN
 SELECT CASE WHEN length(NEW.payload_bytes)<>(SELECT payload_byte_length FROM raw_events WHERE event_id=NEW.event_id) THEN RAISE(ABORT,'raw_payload_length') END;
END;
CREATE TRIGGER source_ref_bounds BEFORE INSERT ON source_refs BEGIN
 SELECT CASE WHEN NEW.byte_end>(SELECT payload_byte_length FROM raw_events WHERE event_id=NEW.event_id) THEN RAISE(ABORT,'source_ref_bounds') END;
END;
CREATE TRIGGER lexical_source_hash BEFORE INSERT ON lexical_snapshots BEGIN
 SELECT CASE WHEN NEW.source_sha256<>(SELECT source_sha256 FROM reading_unit_revisions WHERE unit_id=NEW.unit_id AND text_revision=NEW.text_revision) THEN RAISE(ABORT,'lexical_source_hash') END;
END;
CREATE TRIGGER lexical_token_slice BEFORE INSERT ON lexical_tokens BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM lexical_snapshots l JOIN reading_unit_revisions r ON r.unit_id=l.unit_id AND r.text_revision=l.text_revision WHERE l.lexical_id=NEW.lexical_id AND NEW.byte_end<=r.source_byte_length AND CAST(NEW.surface AS BLOB)=substr(CAST(r.source_text AS BLOB),NEW.byte_start+1,NEW.byte_end-NEW.byte_start)) THEN RAISE(ABORT,'lexical_token_slice') END;
 SELECT CASE WHEN EXISTS(SELECT 1 FROM lexical_tokens WHERE lexical_id=NEW.lexical_id AND byte_start<NEW.byte_end AND byte_end>NEW.byte_start) THEN RAISE(ABORT,'lexical_token_overlap') END;
END;
CREATE TRIGGER translation_task_authority BEFORE INSERT ON translations BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM request_tasks t JOIN profile_meta p ON p.singleton=1 JOIN purpose_runtime_states s ON s.scope_key=t.scope_key AND s.purpose=t.purpose JOIN request_attempts a ON a.attempt_id=NEW.attempt_id AND a.task_id=t.task_id LEFT JOIN credential_refs c ON c.credential_id=a.credential_id WHERE t.task_id=NEW.task_id AND t.purpose='translation' AND t.state='completed' AND p.runtime_state='active' AND t.profile_generation=p.runtime_generation AND t.server_instance_id=p.server_instance_id AND s.is_enabled=1 AND t.authorization_epoch=s.authorization_epoch AND t.binding_id=s.binding_id AND a.state='completed' AND (a.credential_id IS NULL OR c.state='active')) THEN RAISE(ABORT,'translation_task_authority') END;
END;
CREATE TRIGGER study_task_authority BEFORE INSERT ON study_results BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM request_tasks t JOIN profile_meta p ON p.singleton=1 JOIN purpose_runtime_states s ON s.scope_key=t.scope_key AND s.purpose=t.purpose JOIN request_attempts a ON a.attempt_id=NEW.attempt_id AND a.task_id=t.task_id LEFT JOIN credential_refs c ON c.credential_id=a.credential_id WHERE t.task_id=NEW.task_id AND t.purpose IN ('grammar','context_word') AND t.state='completed' AND p.runtime_state='active' AND t.profile_generation=p.runtime_generation AND t.server_instance_id=p.server_instance_id AND s.is_enabled=1 AND t.authorization_epoch=s.authorization_epoch AND t.binding_id=s.binding_id AND a.state='completed' AND (a.credential_id IS NULL OR c.state='active')) THEN RAISE(ABORT,'study_task_authority') END;
 SELECT CASE WHEN NEW.selection_byte_end IS NOT NULL AND NEW.selection_byte_end>(SELECT r.source_byte_length FROM request_tasks t JOIN reading_unit_revisions r ON r.unit_id=t.unit_id AND r.text_revision=t.text_revision WHERE t.task_id=NEW.task_id) THEN RAISE(ABORT,'study_selection_bounds') END;
END;
CREATE TRIGGER immutable_operation_receipts BEFORE UPDATE ON operation_receipts BEGIN SELECT RAISE(ABORT,'immutable_operation_receipts'); END;
CREATE TRIGGER immutable_resource_snapshots BEFORE UPDATE ON resource_snapshots BEGIN SELECT RAISE(ABORT,'immutable_resource_snapshots'); END;
CREATE TRIGGER immutable_capture_sources BEFORE UPDATE ON capture_sources BEGIN SELECT RAISE(ABORT,'immutable_capture_sources'); END;
CREATE TRIGGER immutable_source_selections BEFORE UPDATE ON source_selections BEGIN SELECT RAISE(ABORT,'immutable_source_selections'); END;
CREATE TRIGGER immutable_raw_events BEFORE UPDATE ON raw_events BEGIN SELECT RAISE(ABORT,'immutable_raw_events'); END;
CREATE TRIGGER immutable_reading_unit_revisions BEFORE UPDATE ON reading_unit_revisions BEGIN SELECT RAISE(ABORT,'immutable_reading_unit_revisions'); END;
CREATE TRIGGER immutable_source_refs BEFORE UPDATE ON source_refs BEGIN SELECT RAISE(ABORT,'immutable_source_refs'); END;
CREATE TRIGGER immutable_lexical_snapshots BEFORE UPDATE ON lexical_snapshots BEGIN SELECT RAISE(ABORT,'immutable_lexical_snapshots'); END;
CREATE TRIGGER immutable_lexical_tokens BEFORE UPDATE ON lexical_tokens BEGIN SELECT RAISE(ABORT,'immutable_lexical_tokens'); END;
CREATE TRIGGER immutable_translations BEFORE UPDATE ON translations BEGIN SELECT RAISE(ABORT,'immutable_translations'); END;
CREATE TRIGGER immutable_corrections BEFORE UPDATE ON corrections BEGIN SELECT RAISE(ABORT,'immutable_corrections'); END;
CREATE TRIGGER immutable_study_results BEFORE UPDATE ON study_results BEGIN SELECT RAISE(ABORT,'immutable_study_results'); END;
CREATE TRIGGER immutable_preference_revisions BEFORE UPDATE ON preference_revisions BEGIN SELECT RAISE(ABORT,'immutable_preference_revisions'); END;
CREATE TRIGGER connection_control_cas BEFORE UPDATE ON connection_runtime_states
WHEN NEW.connection_id IS NOT OLD.connection_id OR NEW.control_revision<>OLD.control_revision+1
BEGIN SELECT RAISE(ABORT,'connection_control_revision'); END;
CREATE TRIGGER task_must_start_queued BEFORE INSERT ON request_tasks
WHEN NEW.state<>'queued' OR NEW.finished_at_ms IS NOT NULL
BEGIN SELECT RAISE(ABORT,'task_must_start_queued'); END;
CREATE TRIGGER task_identity_and_transition BEFORE UPDATE ON request_tasks
WHEN NEW.task_id IS NOT OLD.task_id OR NEW.unit_id IS NOT OLD.unit_id OR NEW.text_revision IS NOT OLD.text_revision OR NEW.session_id IS NOT OLD.session_id OR NEW.scope_key IS NOT OLD.scope_key OR NEW.purpose IS NOT OLD.purpose OR NEW.config_revision_id IS NOT OLD.config_revision_id OR NEW.binding_id IS NOT OLD.binding_id OR NEW.engine_id IS NOT OLD.engine_id OR NEW.profile_generation IS NOT OLD.profile_generation OR NEW.server_instance_id IS NOT OLD.server_instance_id OR NEW.authorization_epoch IS NOT OLD.authorization_epoch OR NEW.request_semantics_sha256 IS NOT OLD.request_semantics_sha256 OR NEW.input_snapshot_json IS NOT OLD.input_snapshot_json OR NEW.created_at_ms IS NOT OLD.created_at_ms OR (OLD.state IN ('completed','failed','cancelled','invalidated') AND NEW.state IS NOT OLD.state) OR (OLD.state='running' AND NEW.state='queued') OR (OLD.state='queued' AND NEW.state='completed') OR ((NEW.state IN ('queued','running'))<>(NEW.finished_at_ms IS NULL))
BEGIN SELECT RAISE(ABORT,'task_identity_or_transition'); END;
CREATE TRIGGER attempt_identity_and_transition BEFORE UPDATE ON request_attempts
WHEN NEW.attempt_id IS NOT OLD.attempt_id OR NEW.task_id IS NOT OLD.task_id OR NEW.attempt_number IS NOT OLD.attempt_number OR NEW.config_revision_id IS NOT OLD.config_revision_id OR NEW.binding_id IS NOT OLD.binding_id OR NEW.purpose IS NOT OLD.purpose OR NEW.actual_engine_id IS NOT OLD.actual_engine_id OR NEW.credential_id IS NOT OLD.credential_id OR NEW.dispatch_kind IS NOT OLD.dispatch_kind OR NEW.started_at_ms IS NOT OLD.started_at_ms OR (OLD.state<>'started' AND NEW.state IS NOT OLD.state) OR ((NEW.state='started')<>(NEW.finished_at_ms IS NULL))
BEGIN SELECT RAISE(ABORT,'attempt_identity_or_transition'); END;
CREATE TRIGGER translations_connection_authority BEFORE INSERT ON translations BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM request_tasks t JOIN request_attempts a ON a.task_id=t.task_id AND a.attempt_id=NEW.attempt_id JOIN binding_engines e ON e.revision_id=a.config_revision_id AND e.binding_id=a.binding_id AND e.engine_id=a.actual_engine_id AND e.purpose=a.purpose JOIN purpose_bindings b ON b.revision_id=e.revision_id AND b.binding_id=e.binding_id AND b.purpose=e.purpose JOIN models m ON m.revision_id=e.revision_id AND m.model_id=e.model_id JOIN connections c ON c.revision_id=m.revision_id AND c.connection_id=m.connection_id JOIN connection_runtime_states r ON r.connection_id=c.connection_id WHERE t.task_id=NEW.task_id AND t.purpose='translation' AND e.enabled=1 AND b.enabled=1 AND b.binding_mode='override' AND c.availability_intent=1 AND r.is_suspended=0 AND ((c.auth_mode='none' AND a.credential_id IS NULL) OR (c.auth_mode='api_key' AND EXISTS(SELECT 1 FROM credential_pool_members pm JOIN credential_refs cr ON cr.credential_id=pm.credential_id WHERE pm.pool_id=c.credential_pool_id AND pm.pool_revision=c.credential_pool_revision AND pm.credential_id=a.credential_id AND pm.enabled=1 AND cr.state='active')))) THEN RAISE(ABORT,'translations_connection_authority') END;
END;
CREATE TRIGGER study_results_connection_authority BEFORE INSERT ON study_results BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM request_tasks t JOIN request_attempts a ON a.task_id=t.task_id AND a.attempt_id=NEW.attempt_id JOIN binding_engines e ON e.revision_id=a.config_revision_id AND e.binding_id=a.binding_id AND e.engine_id=a.actual_engine_id AND e.purpose=a.purpose JOIN purpose_bindings b ON b.revision_id=e.revision_id AND b.binding_id=e.binding_id AND b.purpose=e.purpose JOIN models m ON m.revision_id=e.revision_id AND m.model_id=e.model_id JOIN connections c ON c.revision_id=m.revision_id AND c.connection_id=m.connection_id JOIN connection_runtime_states r ON r.connection_id=c.connection_id WHERE t.task_id=NEW.task_id AND t.purpose IN ('grammar','context_word') AND e.enabled=1 AND b.enabled=1 AND b.binding_mode='override' AND c.availability_intent=1 AND r.is_suspended=0 AND ((c.auth_mode='none' AND a.credential_id IS NULL) OR (c.auth_mode='api_key' AND EXISTS(SELECT 1 FROM credential_pool_members pm JOIN credential_refs cr ON cr.credential_id=pm.credential_id WHERE pm.pool_id=c.credential_pool_id AND pm.pool_revision=c.credential_pool_revision AND pm.credential_id=a.credential_id AND pm.enabled=1 AND cr.state='active')))) THEN RAISE(ABORT,'study_results_connection_authority') END;
END;
CREATE TRIGGER immutable_raw_payload BEFORE UPDATE ON raw_event_payloads BEGIN SELECT RAISE(ABORT,'immutable_raw_payload'); END;
CREATE TRIGGER preference_head_cas BEFORE UPDATE ON preference_heads
WHEN NEW.scope_key IS NOT OLD.scope_key OR NEW.head_revision<>OLD.head_revision+1
BEGIN SELECT RAISE(ABORT,'preference_head_revision'); END;
CREATE TRIGGER session_resource_kind BEFORE INSERT ON sessions
WHEN NEW.source_kind='hook' AND NOT EXISTS(SELECT 1 FROM resource_snapshots WHERE resource_snapshot_id=NEW.core_resource_snapshot_id AND kind='core')
BEGIN SELECT RAISE(ABORT,'session_resource_kind'); END;
CREATE TRIGGER capture_source_requires_hook BEFORE INSERT ON capture_sources
WHEN NOT EXISTS(SELECT 1 FROM sessions WHERE session_id=NEW.session_id AND source_kind='hook')
BEGIN SELECT RAISE(ABORT,'capture_source_requires_hook'); END;
CREATE TRIGGER raw_event_requires_hook BEFORE INSERT ON raw_events
WHEN NOT EXISTS(SELECT 1 FROM sessions WHERE session_id=NEW.session_id AND source_kind='hook')
BEGIN SELECT RAISE(ABORT,'raw_event_requires_hook'); END;
CREATE TRIGGER reading_source_kind BEFORE INSERT ON reading_unit_revisions
WHEN NOT EXISTS(SELECT 1 FROM sessions WHERE session_id=NEW.session_id AND ((source_kind='manual' AND NEW.selection_revision IS NULL) OR (source_kind='hook' AND NEW.selection_revision IS NOT NULL)))
BEGIN SELECT RAISE(ABORT,'reading_source_kind'); END;
CREATE TRIGGER source_ref_selected_source BEFORE INSERT ON source_refs
WHEN NOT EXISTS(SELECT 1 FROM reading_unit_revisions r JOIN source_selections s ON s.session_id=r.session_id AND s.selection_revision=r.selection_revision JOIN raw_events e ON e.session_id=r.session_id AND e.event_id=NEW.event_id WHERE r.unit_id=NEW.unit_id AND r.text_revision=NEW.text_revision AND e.event_kind='text' AND e.source_id=s.source_id)
BEGIN SELECT RAISE(ABORT,'source_ref_selected_source'); END;
CREATE TRIGGER lexical_resource_kind BEFORE INSERT ON lexical_snapshots
WHEN NOT EXISTS(SELECT 1 FROM resource_snapshots WHERE resource_snapshot_id=NEW.language_resource_snapshot_id AND kind='language')
BEGIN SELECT RAISE(ABORT,'lexical_resource_kind'); END;
CREATE TRIGGER purpose_runtime_epoch BEFORE UPDATE ON purpose_runtime_states
WHEN NEW.scope_key IS NOT OLD.scope_key OR NEW.purpose IS NOT OLD.purpose
 OR NEW.authorization_epoch NOT IN (OLD.authorization_epoch,OLD.authorization_epoch+1)
 OR ((NEW.is_enabled IS NOT OLD.is_enabled OR NEW.binding_id IS NOT OLD.binding_id OR NEW.config_semantics_sha256 IS NOT OLD.config_semantics_sha256) AND NEW.authorization_epoch<>OLD.authorization_epoch+1)
BEGIN SELECT RAISE(ABORT,'invalid_purpose_authorization_epoch'); END;
CREATE TRIGGER attempt_initial_state BEFORE INSERT ON request_attempts
WHEN NEW.state<>'started' OR NEW.finished_at_ms IS NOT NULL
BEGIN SELECT RAISE(ABORT,'attempt_must_start'); END;
CREATE UNIQUE INDEX binding_engines_one_study_engine ON binding_engines(revision_id, binding_id) WHERE purpose <> 'translation';
CREATE INDEX binding_engines_by_model ON binding_engines(revision_id, model_id);
CREATE INDEX term_sets_by_scope ON term_sets(revision_id, scope_key, enabled);
CREATE INDEX credential_refs_cleanup ON credential_refs(state, cleanup_requested_at_ms);
CREATE INDEX credential_pool_members_by_credential ON credential_pool_members(credential_id, pool_id, pool_revision);
CREATE INDEX connections_by_credential_pool ON connections(credential_pool_id, credential_pool_revision);
CREATE INDEX models_by_connection ON models(revision_id, connection_id);
CREATE INDEX test_records_by_semantics ON test_records(request_semantics_hash, completed_at_ms DESC, test_id);
CREATE INDEX test_records_by_connection ON test_records(config_revision_id, connection_id, stage, completed_at_ms DESC);
CREATE INDEX test_records_by_model ON test_records(config_revision_id, model_id, stage, completed_at_ms DESC);
CREATE INDEX test_records_by_purpose ON test_records(config_revision_id, binding_id, purpose, completed_at_ms DESC);
CREATE INDEX test_records_by_source ON test_records(unit_id, text_revision);
CREATE INDEX test_record_credentials_by_credential ON test_record_credentials(credential_id, test_id);
CREATE TRIGGER config_heads_revision_monotonic BEFORE UPDATE ON config_heads
WHEN NEW.head_kind IS NOT OLD.head_kind OR NEW.head_revision <> OLD.head_revision + 1
BEGIN SELECT RAISE(ABORT, 'invalid_config_head_revision'); END;
CREATE TRIGGER config_revisions_no_update BEFORE UPDATE ON config_revisions BEGIN SELECT RAISE(ABORT, 'immutable_config_revision'); END;
CREATE TRIGGER connections_no_update BEFORE UPDATE ON connections BEGIN SELECT RAISE(ABORT, 'immutable_connection_snapshot'); END;
CREATE TRIGGER models_no_update BEFORE UPDATE ON models BEGIN SELECT RAISE(ABORT, 'immutable_model_snapshot'); END;
CREATE TRIGGER purpose_bindings_no_update BEFORE UPDATE ON purpose_bindings BEGIN SELECT RAISE(ABORT, 'immutable_purpose_snapshot'); END;
CREATE TRIGGER binding_engines_no_update BEFORE UPDATE ON binding_engines BEGIN SELECT RAISE(ABORT, 'immutable_engine_snapshot'); END;
CREATE TRIGGER model_parameters_no_update BEFORE UPDATE ON model_parameters BEGIN SELECT RAISE(ABORT, 'immutable_model_parameter'); END;
CREATE TRIGGER binding_parameters_no_update BEFORE UPDATE ON binding_parameters BEGIN SELECT RAISE(ABORT, 'immutable_binding_parameter'); END;
CREATE TRIGGER engine_parameters_no_update BEFORE UPDATE ON engine_parameters BEGIN SELECT RAISE(ABORT, 'immutable_engine_parameter'); END;
CREATE TRIGGER templates_no_update BEFORE UPDATE ON templates BEGIN SELECT RAISE(ABORT, 'immutable_template_snapshot'); END;
CREATE TRIGGER model_default_templates_no_update BEFORE UPDATE ON model_default_templates BEGIN SELECT RAISE(ABORT, 'immutable_model_template'); END;
CREATE TRIGGER term_sets_no_update BEFORE UPDATE ON term_sets BEGIN SELECT RAISE(ABORT, 'immutable_term_set'); END;
CREATE TRIGGER terms_no_update BEFORE UPDATE ON terms BEGIN SELECT RAISE(ABORT, 'immutable_term'); END;
CREATE TRIGGER credential_pool_revisions_no_update BEFORE UPDATE ON credential_pool_revisions BEGIN SELECT RAISE(ABORT, 'immutable_credential_pool'); END;
CREATE TRIGGER credential_pool_members_no_update BEFORE UPDATE ON credential_pool_members BEGIN SELECT RAISE(ABORT, 'immutable_credential_pool_member'); END;
CREATE TRIGGER test_record_credentials_no_update BEFORE UPDATE ON test_record_credentials BEGIN SELECT RAISE(ABORT, 'immutable_test_credential_scope'); END;
CREATE TRIGGER test_records_immutable_result BEFORE UPDATE ON test_records
WHEN NEW.test_id IS NOT OLD.test_id OR NEW.config_revision_id IS NOT OLD.config_revision_id OR NEW.connection_id IS NOT OLD.connection_id OR NEW.model_id IS NOT OLD.model_id OR NEW.binding_id IS NOT OLD.binding_id OR NEW.purpose IS NOT OLD.purpose OR NEW.stage IS NOT OLD.stage OR NEW.request_semantics_hash IS NOT OLD.request_semantics_hash OR NEW.input_scope IS NOT OLD.input_scope OR NEW.input_summary_json IS NOT OLD.input_summary_json OR NEW.response_summary_json IS NOT OLD.response_summary_json OR NEW.output_validation_json IS NOT OLD.output_validation_json OR NEW.state IS NOT OLD.state OR NEW.started_at_ms IS NOT OLD.started_at_ms OR NEW.completed_at_ms IS NOT OLD.completed_at_ms OR NEW.error_code IS NOT OLD.error_code OR NEW.error_stage IS NOT OLD.error_stage OR ((NEW.unit_id IS NOT OLD.unit_id OR NEW.text_revision IS NOT OLD.text_revision) AND NOT (NEW.unit_id IS NULL AND NEW.text_revision IS NULL))
BEGIN SELECT RAISE(ABORT, 'immutable_test_result'); END;
CREATE TRIGGER credential_refs_identity_immutable BEFORE UPDATE ON credential_refs
WHEN NEW.credential_id IS NOT OLD.credential_id OR NEW.vault_ref IS NOT OLD.vault_ref OR NEW.masked_label IS NOT OLD.masked_label OR NEW.auth_kind IS NOT OLD.auth_kind OR NEW.created_at_ms IS NOT OLD.created_at_ms
BEGIN SELECT RAISE(ABORT, 'immutable_credential_identity'); END;
CREATE TRIGGER credential_refs_lifecycle_monotonic BEFORE UPDATE ON credential_refs
WHEN NEW.lifecycle_revision <> OLD.lifecycle_revision + 1 OR (OLD.state IN ('revoked','cleanup_pending') AND NEW.state IN ('pending','active')) OR (OLD.state = 'active' AND NEW.state = 'pending') OR (OLD.activated_at_ms IS NOT NULL AND NEW.activated_at_ms IS NOT OLD.activated_at_ms) OR (OLD.revoked_at_ms IS NOT NULL AND NEW.revoked_at_ms IS NOT OLD.revoked_at_ms) OR (OLD.cleaned_at_ms IS NOT NULL AND NEW.cleaned_at_ms IS NOT OLD.cleaned_at_ms)
BEGIN SELECT RAISE(ABORT, 'invalid_credential_lifecycle_transition'); END;
CREATE UNIQUE INDEX purpose_bindings_scope_identity ON purpose_bindings(revision_id,binding_id,scope_key,purpose);
CREATE TRIGGER config_seal_identity BEFORE INSERT ON config_revision_seals
WHEN NOT EXISTS(SELECT 1 FROM config_revisions c WHERE c.revision_id=NEW.revision_id AND c.content_hash=NEW.content_hash)
 OR EXISTS(SELECT 1 FROM connections c WHERE c.revision_id=NEW.revision_id AND c.credential_pool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM credential_pool_seals s WHERE s.pool_id=c.credential_pool_id AND s.pool_revision=c.credential_pool_revision AND NOT EXISTS(SELECT 1 FROM credential_pool_gc_marks g WHERE g.pool_id=s.pool_id AND g.pool_revision=s.pool_revision)))
BEGIN SELECT RAISE(ABORT,'invalid_config_seal'); END;
CREATE TRIGGER pool_seal_identity BEFORE INSERT ON credential_pool_seals
WHEN NOT EXISTS(SELECT 1 FROM credential_pool_revisions p WHERE p.pool_id=NEW.pool_id AND p.pool_revision=NEW.pool_revision AND p.content_hash=NEW.content_hash)
BEGIN SELECT RAISE(ABORT,'invalid_pool_seal'); END;
CREATE TRIGGER config_gc_unreferenced BEFORE INSERT ON config_gc_marks
WHEN EXISTS(SELECT 1 FROM config_heads WHERE revision_id=NEW.revision_id)
 OR EXISTS(SELECT 1 FROM request_tasks WHERE config_revision_id=NEW.revision_id)
 OR EXISTS(SELECT 1 FROM test_records WHERE config_revision_id=NEW.revision_id)
 OR EXISTS(SELECT 1 FROM purpose_runtime_states WHERE config_revision_id=NEW.revision_id)
 OR EXISTS(SELECT 1 FROM connection_runtime_states WHERE config_revision_id=NEW.revision_id)
BEGIN SELECT RAISE(ABORT,'config_gc_still_referenced'); END;
CREATE TRIGGER pool_gc_unreferenced BEFORE INSERT ON credential_pool_gc_marks
WHEN EXISTS(SELECT 1 FROM connections WHERE credential_pool_id=NEW.pool_id AND credential_pool_revision=NEW.pool_revision)
 OR EXISTS(SELECT 1 FROM test_record_credentials WHERE pool_id=NEW.pool_id AND pool_revision=NEW.pool_revision)
BEGIN SELECT RAISE(ABORT,'pool_gc_still_referenced'); END;
CREATE TRIGGER config_revision_seals_no_update BEFORE UPDATE ON config_revision_seals BEGIN SELECT RAISE(ABORT,'immutable_config_revision_seals'); END;
CREATE TRIGGER config_revision_seals_no_delete BEFORE DELETE ON config_revision_seals BEGIN SELECT RAISE(ABORT,'immutable_config_revision_seals'); END;
CREATE TRIGGER credential_pool_seals_no_update BEFORE UPDATE ON credential_pool_seals BEGIN SELECT RAISE(ABORT,'immutable_credential_pool_seals'); END;
CREATE TRIGGER credential_pool_seals_no_delete BEFORE DELETE ON credential_pool_seals BEGIN SELECT RAISE(ABORT,'immutable_credential_pool_seals'); END;
CREATE TRIGGER config_gc_marks_no_update BEFORE UPDATE ON config_gc_marks BEGIN SELECT RAISE(ABORT,'immutable_config_gc_marks'); END;
CREATE TRIGGER config_gc_marks_no_delete BEFORE DELETE ON config_gc_marks BEGIN SELECT RAISE(ABORT,'immutable_config_gc_marks'); END;
CREATE TRIGGER credential_pool_gc_marks_no_update BEFORE UPDATE ON credential_pool_gc_marks BEGIN SELECT RAISE(ABORT,'immutable_credential_pool_gc_marks'); END;
CREATE TRIGGER credential_pool_gc_marks_no_delete BEFORE DELETE ON credential_pool_gc_marks BEGIN SELECT RAISE(ABORT,'immutable_credential_pool_gc_marks'); END;
CREATE TRIGGER connections_sealed_insert BEFORE INSERT ON connections WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER connections_sealed_delete BEFORE DELETE ON connections WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER models_sealed_insert BEFORE INSERT ON models WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER models_sealed_delete BEFORE DELETE ON models WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER purpose_bindings_sealed_insert BEFORE INSERT ON purpose_bindings WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER purpose_bindings_sealed_delete BEFORE DELETE ON purpose_bindings WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER binding_engines_sealed_insert BEFORE INSERT ON binding_engines WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER binding_engines_sealed_delete BEFORE DELETE ON binding_engines WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER model_parameters_sealed_insert BEFORE INSERT ON model_parameters WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER model_parameters_sealed_delete BEFORE DELETE ON model_parameters WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER binding_parameters_sealed_insert BEFORE INSERT ON binding_parameters WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER binding_parameters_sealed_delete BEFORE DELETE ON binding_parameters WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER engine_parameters_sealed_insert BEFORE INSERT ON engine_parameters WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER engine_parameters_sealed_delete BEFORE DELETE ON engine_parameters WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER templates_sealed_insert BEFORE INSERT ON templates WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER templates_sealed_delete BEFORE DELETE ON templates WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER model_default_templates_sealed_insert BEFORE INSERT ON model_default_templates WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER model_default_templates_sealed_delete BEFORE DELETE ON model_default_templates WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER term_sets_sealed_insert BEFORE INSERT ON term_sets WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER term_sets_sealed_delete BEFORE DELETE ON term_sets WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER terms_sealed_insert BEFORE INSERT ON terms WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER terms_sealed_delete BEFORE DELETE ON terms WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;
CREATE TRIGGER pool_members_sealed_insert BEFORE INSERT ON credential_pool_members
WHEN EXISTS(SELECT 1 FROM credential_pool_seals WHERE pool_id=NEW.pool_id AND pool_revision=NEW.pool_revision)
BEGIN SELECT RAISE(ABORT,'sealed_pool_snapshot'); END;
CREATE TRIGGER pool_members_sealed_delete BEFORE DELETE ON credential_pool_members
WHEN EXISTS(SELECT 1 FROM credential_pool_seals WHERE pool_id=OLD.pool_id AND pool_revision=OLD.pool_revision)
 AND NOT EXISTS(SELECT 1 FROM credential_pool_gc_marks WHERE pool_id=OLD.pool_id AND pool_revision=OLD.pool_revision)
BEGIN SELECT RAISE(ABORT,'sealed_pool_snapshot'); END;
CREATE TRIGGER config_heads_requires_seal_insert BEFORE INSERT ON config_heads WHEN NOT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=NEW.revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id)) BEGIN SELECT RAISE(ABORT,'config_not_sealed_or_purged'); END;
CREATE TRIGGER config_heads_requires_seal_update BEFORE UPDATE ON config_heads WHEN NOT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=NEW.revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id)) BEGIN SELECT RAISE(ABORT,'config_not_sealed_or_purged'); END;
CREATE TRIGGER test_records_requires_seal_insert BEFORE INSERT ON test_records WHEN NOT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=NEW.config_revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id)) BEGIN SELECT RAISE(ABORT,'config_not_sealed_or_purged'); END;
CREATE TRIGGER test_records_requires_seal_update BEFORE UPDATE ON test_records WHEN NOT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=NEW.config_revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id)) BEGIN SELECT RAISE(ABORT,'config_not_sealed_or_purged'); END;
CREATE TRIGGER purpose_runtime_states_requires_seal_insert BEFORE INSERT ON purpose_runtime_states WHEN NOT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=NEW.config_revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id)) BEGIN SELECT RAISE(ABORT,'config_not_sealed_or_purged'); END;
CREATE TRIGGER purpose_runtime_states_requires_seal_update BEFORE UPDATE ON purpose_runtime_states WHEN NOT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=NEW.config_revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id)) BEGIN SELECT RAISE(ABORT,'config_not_sealed_or_purged'); END;
CREATE TRIGGER connection_runtime_states_requires_seal_insert BEFORE INSERT ON connection_runtime_states WHEN NOT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=NEW.config_revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id)) BEGIN SELECT RAISE(ABORT,'config_not_sealed_or_purged'); END;
CREATE TRIGGER connection_runtime_states_requires_seal_update BEFORE UPDATE ON connection_runtime_states WHEN NOT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=NEW.config_revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id)) BEGIN SELECT RAISE(ABORT,'config_not_sealed_or_purged'); END;
CREATE TRIGGER request_tasks_requires_seal_insert BEFORE INSERT ON request_tasks WHEN NOT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=NEW.config_revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id)) BEGIN SELECT RAISE(ABORT,'config_not_sealed_or_purged'); END;
CREATE TRIGGER request_tasks_requires_seal_update BEFORE UPDATE ON request_tasks WHEN NOT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=NEW.config_revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id)) BEGIN SELECT RAISE(ABORT,'config_not_sealed_or_purged'); END;
CREATE TRIGGER connections_pool_not_purged BEFORE INSERT ON connections
WHEN NEW.credential_pool_id IS NOT NULL AND EXISTS(SELECT 1 FROM credential_pool_gc_marks WHERE pool_id=NEW.credential_pool_id AND pool_revision=NEW.credential_pool_revision)
BEGIN SELECT RAISE(ABORT,'pool_already_purged'); END;
CREATE TRIGGER test_credentials_pool_not_purged BEFORE INSERT ON test_record_credentials
WHEN NEW.pool_id IS NOT NULL AND EXISTS(SELECT 1 FROM credential_pool_gc_marks WHERE pool_id=NEW.pool_id AND pool_revision=NEW.pool_revision)
BEGIN SELECT RAISE(ABORT,'pool_already_purged'); END;
CREATE INDEX ix_capture_operations_state ON capture_operations(state,started_at_ms);
CREATE INDEX ix_capture_operations_session ON capture_operations(session_id,started_at_ms,operation_id);
CREATE INDEX ix_hook_events_operation_order ON hook_events(operation_id,event_sequence);
CREATE INDEX ix_hook_events_session_order ON hook_events(session_id,event_sequence);
CREATE INDEX ix_hook_events_filter ON hook_events(operation_id,level,stage,event_sequence);
CREATE INDEX ix_body_records_session_order ON body_record_positions(session_id,record_sequence);
CREATE TRIGGER immutable_reading_preference_settings BEFORE UPDATE ON reading_preference_settings BEGIN SELECT RAISE(ABORT,'immutable_reading_preference_settings'); END;
CREATE TRIGGER body_record_position AFTER INSERT ON reading_unit_revisions BEGIN
  INSERT INTO body_record_positions(session_id,unit_id,text_revision) VALUES(NEW.session_id,NEW.unit_id,NEW.text_revision);
END;
CREATE TRIGGER immutable_hook_events BEFORE UPDATE ON hook_events BEGIN SELECT RAISE(ABORT,'immutable_hook_events'); END;
CREATE TRIGGER immutable_body_record_positions BEFORE UPDATE ON body_record_positions BEGIN SELECT RAISE(ABORT,'immutable_body_record_positions'); END;
CREATE TRIGGER capture_operation_identity BEFORE UPDATE ON capture_operations
WHEN NEW.operation_id<>OLD.operation_id OR NEW.request_sha256<>OLD.request_sha256 OR NEW.started_at_ms<>OLD.started_at_ms OR NEW.legacy_unknown_start<>OLD.legacy_unknown_start
 OR (OLD.session_id IS NOT NULL AND NEW.session_id IS NOT OLD.session_id)
BEGIN SELECT RAISE(ABORT,'immutable_capture_operation_identity'); END;
CREATE TRIGGER hook_event_operation_session BEFORE INSERT ON hook_events
WHEN NEW.session_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM capture_operations o WHERE o.operation_id=NEW.operation_id AND o.session_id=NEW.session_id)
BEGIN SELECT RAISE(ABORT,'hook_event_operation_session'); END;
CREATE TRIGGER capture_operation_hook_session_insert BEFORE INSERT ON capture_operations
WHEN NEW.session_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM sessions WHERE session_id=NEW.session_id AND source_kind='hook')
BEGIN SELECT RAISE(ABORT,'capture_operation_requires_hook'); END;
CREATE TRIGGER capture_operation_hook_session_update BEFORE UPDATE ON capture_operations
WHEN NEW.session_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM sessions WHERE session_id=NEW.session_id AND source_kind='hook')
BEGIN SELECT RAISE(ABORT,'capture_operation_requires_hook'); END;
CREATE TRIGGER capture_operation_transition BEFORE UPDATE ON capture_operations
WHEN NOT (NEW.state=OLD.state OR (OLD.state='opening' AND NEW.state IN ('active','failed')) OR (OLD.state='active' AND NEW.state IN ('stopped','failed','recovered')))
BEGIN SELECT RAISE(ABORT,'capture_operation_transition'); END;
DELETE FROM "sqlite_sequence";
INSERT INTO "sqlite_sequence" VALUES('body_record_positions',0);
COMMIT;
