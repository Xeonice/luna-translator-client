-- Production schema v1, derived from reviewed FP1-20261019-R6 field contract.
-- Source design retained separately; runtime embeds this immutable migration.
-- 与同目录Core DDL合并后在foreign_keys=ON下验证。可运行性由应用层应用预检决定，保存草稿不要求可运行。
-- 所有*_json仅保存明确白名单的非秘密结构；不得写入Key、令牌、认证Header、原始HTTP body或完整HTTP headers。
-- 配置快照、池版本和测试完成记录不可UPDATE；删除只能通过核对引用与保留政策的受控GC。

-- @table config_revisions | AP | FP-06,FP-08,FP-11 | 配置档内不可变整体配置快照与本地校验摘要
CREATE TABLE config_revisions (
    revision_id TEXT NOT NULL PRIMARY KEY CHECK (length(revision_id) > 0), -- 不可变整体配置版本ID，与请求语义指纹分开
    parent_revision_id TEXT REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 来源版本；首个版本允许为空
    content_hash TEXT NOT NULL CHECK (instr(content_hash,char(0))=0 AND (length(content_hash) = 64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 规范化非秘密配置内容的SHA256
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0), -- 保存时间，UTC毫秒
    save_reason TEXT NOT NULL CHECK (save_reason IN ('create','edit','copy','import','restore','migrate')), -- 形成独立快照的原因
    local_validation_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(local_validation_json,char(0))=0 AND (json_valid(local_validation_json) AND json_type(local_validation_json) = 'object')), -- 本地缺项与字段错误摘要；不是网络测试通过标记
    CHECK (parent_revision_id IS NULL OR parent_revision_id <> revision_id)
) STRICT;

-- @table config_heads | AP | FP-06,FP-08,FP-11 | 草稿与已应用版本的独立可CAS更新指针
CREATE TABLE config_heads (
    head_kind TEXT NOT NULL PRIMARY KEY CHECK (head_kind IN ('draft','applied')), -- 指针种类；保存草稿不移动applied
    revision_id TEXT NOT NULL REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 当前指向的整体配置版本
    head_revision INTEGER NOT NULL CHECK (head_revision >= 0), -- 乐观并发版本；更新须带expected_revision并原子递增
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= 0) -- 指针最后更新时间，UTC毫秒
) STRICT;

-- @table connections | AP | FP-06,FP-08,FP-11 | 某整体配置版本内的连接、路由和非秘密认证引用
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

-- @table models | AP | FP-06,FP-08 | 某配置版本内的实际模型身份与有来源的能力声明
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

-- @table purpose_bindings | AP | FP-06,FP-08,FP-11 | 全局或游戏的用途意图、覆盖、输入范围与调度快照
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

-- @table binding_engines | AP | FP-06,FP-08,FP-09 | 用途的有序模型分配；翻译可多引擎而学习用途仅单引擎
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
CREATE UNIQUE INDEX binding_engines_one_study_engine ON binding_engines(revision_id, binding_id) WHERE purpose <> 'translation';
CREATE INDEX binding_engines_by_model ON binding_engines(revision_id, model_id);

-- @table model_parameters | AP | FP-06 | 模型默认参数逐键三态，不把显式零与省略混同
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

-- @table binding_parameters | AP | FP-06 | 用途层参数三态覆盖与来源追溯
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

-- @table engine_parameters | AP | FP-06 | 有序引擎项的模型特定参数三态覆盖
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

-- @table templates | AP | FP-06,FP-08 | 按用途不可变模板内容、变量目录与兼容来源
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

-- @table model_default_templates | AP | FP-06,FP-08 | 模型按用途默认模板，不把翻译模板充作文法模板
CREATE TABLE model_default_templates (
    revision_id TEXT NOT NULL, -- 所属不可变整体配置版本
    model_id TEXT NOT NULL, -- 同版本模型配置ID
    purpose TEXT NOT NULL CHECK (purpose IN ('translation','grammar','context_word')), -- 独立用途
    template_id TEXT NOT NULL, -- 同版本且同用途模板ID
    PRIMARY KEY (revision_id, model_id, purpose),
    FOREIGN KEY (revision_id, model_id) REFERENCES models(revision_id, model_id) ON DELETE RESTRICT,
    FOREIGN KEY (revision_id, template_id, purpose) REFERENCES templates(revision_id, template_id, purpose) ON DELETE RESTRICT
) STRICT;

-- @table term_sets | AP | FP-06,FP-08,FP-09 | 全局与游戏术语集的不可变范围及匹配规则
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
CREATE INDEX term_sets_by_scope ON term_sets(revision_id, scope_key, enabled);

-- @table terms | AP | FP-06,FP-08,FP-09 | 术语项冻结词面、译法、语言与匹配语义
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

-- @table credential_refs | PL | FP-06,FP-08,FP-11,FP-12 | 系统安全库不可变秘密身份及可恢复生命周期元数据
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
CREATE INDEX credential_refs_cleanup ON credential_refs(state, cleanup_requested_at_ms);

-- @table credential_pool_revisions | PL | FP-06,FP-08,FP-11 | 显式凭据池的不可变成员策略版本
CREATE TABLE credential_pool_revisions (
    pool_id TEXT NOT NULL CHECK (length(pool_id) > 0), -- 跨池版本稳定的凭据池ID
    pool_revision INTEGER NOT NULL CHECK (pool_revision > 0), -- 池版本；成员/策略变更生成新版本
    policy TEXT NOT NULL CHECK (policy IN ('selected','round_robin','weighted')), -- 显式单选或用户授权轮询策略；不授权换供应商
    policy_metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(policy_metadata_json,char(0))=0 AND (json_valid(policy_metadata_json) AND json_type(policy_metadata_json) = 'object')), -- 有界退避、错误权重与次数等非秘密策略
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0), -- 池版本保存时间，UTC毫秒
    content_hash TEXT NOT NULL CHECK (instr(content_hash,char(0))=0 AND (length(content_hash) = 64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 池成员与策略的语义SHA256
    PRIMARY KEY (pool_id, pool_revision)
) STRICT;

-- @table credential_pool_members | PL | FP-06,FP-08,FP-11 | 某池版本中的精确凭据版本分配
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
CREATE INDEX credential_pool_members_by_credential ON credential_pool_members(credential_id, pool_id, pool_revision);
CREATE INDEX connections_by_credential_pool ON connections(credential_pool_id, credential_pool_revision);
CREATE INDEX models_by_connection ON models(revision_id, connection_id);

-- @table test_records | AI | FP-06,FP-08,FP-12 | 用户主动测试的独立完成记录与确切范围，不进入游戏正文历史
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
CREATE INDEX test_records_by_semantics ON test_records(request_semantics_hash, completed_at_ms DESC, test_id);
CREATE INDEX test_records_by_connection ON test_records(config_revision_id, connection_id, stage, completed_at_ms DESC);
CREATE INDEX test_records_by_model ON test_records(config_revision_id, model_id, stage, completed_at_ms DESC);
CREATE INDEX test_records_by_purpose ON test_records(config_revision_id, binding_id, purpose, completed_at_ms DESC);
CREATE INDEX test_records_by_source ON test_records(unit_id, text_revision);

-- @table test_record_credentials | AI | FP-06,FP-08,FP-12 | 单次测试实际触及的精确凭据与池成员范围
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
CREATE INDEX test_record_credentials_by_credential ON test_record_credentials(credential_id, test_id);

-- 不可变整体快照不可被UPDATE；新草稿、复制、应用后的后续编辑必须创建新revision。
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

-- 测试完成记录只允许ON DELETE SET NULL解除已清理原句引用；测试语义与结论不能重写。
CREATE TRIGGER test_records_immutable_result BEFORE UPDATE ON test_records
WHEN NEW.test_id IS NOT OLD.test_id OR NEW.config_revision_id IS NOT OLD.config_revision_id OR NEW.connection_id IS NOT OLD.connection_id OR NEW.model_id IS NOT OLD.model_id OR NEW.binding_id IS NOT OLD.binding_id OR NEW.purpose IS NOT OLD.purpose OR NEW.stage IS NOT OLD.stage OR NEW.request_semantics_hash IS NOT OLD.request_semantics_hash OR NEW.input_scope IS NOT OLD.input_scope OR NEW.input_summary_json IS NOT OLD.input_summary_json OR NEW.response_summary_json IS NOT OLD.response_summary_json OR NEW.output_validation_json IS NOT OLD.output_validation_json OR NEW.state IS NOT OLD.state OR NEW.started_at_ms IS NOT OLD.started_at_ms OR NEW.completed_at_ms IS NOT OLD.completed_at_ms OR NEW.error_code IS NOT OLD.error_code OR NEW.error_stage IS NOT OLD.error_stage OR ((NEW.unit_id IS NOT OLD.unit_id OR NEW.text_revision IS NOT OLD.text_revision) AND NOT (NEW.unit_id IS NULL AND NEW.text_revision IS NULL))
BEGIN SELECT RAISE(ABORT, 'immutable_test_result'); END;

-- 秘密身份不可原地更换；生命周期CAS与撤销为单独的安全控制，不能等配置应用。
CREATE TRIGGER credential_refs_identity_immutable BEFORE UPDATE ON credential_refs
WHEN NEW.credential_id IS NOT OLD.credential_id OR NEW.vault_ref IS NOT OLD.vault_ref OR NEW.masked_label IS NOT OLD.masked_label OR NEW.auth_kind IS NOT OLD.auth_kind OR NEW.created_at_ms IS NOT OLD.created_at_ms
BEGIN SELECT RAISE(ABORT, 'immutable_credential_identity'); END;
CREATE TRIGGER credential_refs_lifecycle_monotonic BEFORE UPDATE ON credential_refs
WHEN NEW.lifecycle_revision <> OLD.lifecycle_revision + 1 OR (OLD.state IN ('revoked','cleanup_pending') AND NEW.state IN ('pending','active')) OR (OLD.state = 'active' AND NEW.state = 'pending') OR (OLD.activated_at_ms IS NOT NULL AND NEW.activated_at_ms IS NOT OLD.activated_at_ms) OR (OLD.revoked_at_ms IS NOT NULL AND NEW.revoked_at_ms IS NOT OLD.revoked_at_ms) OR (OLD.cleaned_at_ms IS NOT NULL AND NEW.cleaned_at_ms IS NOT OLD.cleaned_at_ms)
BEGIN SELECT RAISE(ABORT, 'invalid_credential_lifecycle_transition'); END;

-- 当前用途授权闸按完整作用域引用绑定，避免全局/游戏或用途串台。
CREATE UNIQUE INDEX purpose_bindings_scope_identity ON purpose_bindings(revision_id,binding_id,scope_key,purpose);

-- 封存是保存事务的最后步骤；未封存数据不可作为已保存快照引用。
-- @table config_revision_seals | AP | FP-06,FP-08,FP-11 | 完整配置装配后的不可撤销封存凭证
CREATE TABLE config_revision_seals (
    revision_id TEXT NOT NULL PRIMARY KEY REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 被封存的整体配置版本
    content_hash TEXT NOT NULL CHECK(instr(content_hash,char(0))=0 AND (length(content_hash)=64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 与头记录一致；实际规范化内容摘要由Rust计算
    sealed_at_ms INTEGER NOT NULL CHECK(sealed_at_ms>=0) -- 同保存事务的封存时间
) STRICT;
-- @table credential_pool_seals | PL | FP-06,FP-08,FP-11 | 凭据池成员和策略装配完成的不可撤销封存凭证
CREATE TABLE credential_pool_seals (
    pool_id TEXT NOT NULL, -- 被封存池ID
    pool_revision INTEGER NOT NULL, -- 被封存版本
    content_hash TEXT NOT NULL CHECK(instr(content_hash,char(0))=0 AND (length(content_hash)=64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 与池版本头相同的规范语义摘要
    sealed_at_ms INTEGER NOT NULL CHECK(sealed_at_ms>=0), -- 装配完成封存时间
    PRIMARY KEY(pool_id,pool_revision),
    FOREIGN KEY(pool_id,pool_revision) REFERENCES credential_pool_revisions(pool_id,pool_revision) ON DELETE RESTRICT
) STRICT;
-- @table config_gc_marks | AP | FP-09,FP-11 | 不再被使用版本的受控清理墓碑；保留头和封存
CREATE TABLE config_gc_marks (
    revision_id TEXT NOT NULL PRIMARY KEY REFERENCES config_revision_seals(revision_id) ON DELETE RESTRICT, -- 已验证无外部引用的封存版本
    operation_id TEXT NOT NULL REFERENCES operation_receipts(operation_id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED, -- 与清理同事务提交的操作回执
    requested_at_ms INTEGER NOT NULL CHECK(requested_at_ms>=0) -- 授权保留策略执行时间
) STRICT;
-- @table credential_pool_gc_marks | PL | FP-09,FP-11 | 不再被配置或测试引用的池版本清理墓碑
CREATE TABLE credential_pool_gc_marks (
    pool_id TEXT NOT NULL, -- 清理池ID
    pool_revision INTEGER NOT NULL, -- 清理池版本
    operation_id TEXT NOT NULL REFERENCES operation_receipts(operation_id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED, -- 与清理同事务回执
    requested_at_ms INTEGER NOT NULL CHECK(requested_at_ms>=0), -- 清理时间
    PRIMARY KEY(pool_id,pool_revision),
    FOREIGN KEY(pool_id,pool_revision) REFERENCES credential_pool_seals(pool_id,pool_revision) ON DELETE RESTRICT
) STRICT;
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
