-- 首版数据库设计草案；仅用于内存DDL校验，不是产品迁移脚本。
PRAGMA foreign_keys = ON;

-- @table schema_migrations | AP | FP-01,FP-12 | 模式版本与已执行迁移身份；无产品运行记录种子
CREATE TABLE schema_migrations (
    version INTEGER NOT NULL PRIMARY KEY CHECK(version>0), -- 递增模式版本
    migration_sha256 TEXT NOT NULL CHECK(instr(migration_sha256,char(0))=0 AND (migration_sha256 IS NULL OR (length(migration_sha256)=64 AND migration_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 迁移源校验值
    applied_at_ms INTEGER NOT NULL CHECK(applied_at_ms IS NULL OR applied_at_ms>=0), -- 事务提交时间
    app_build TEXT NOT NULL, -- 执行此迁移的实际包版本
    previous_version INTEGER NOT NULL CHECK(previous_version>=0) -- 预检的前置版本
) STRICT;

-- @table profile_meta | AP | FP-01,FP-12 | 本配置档唯一身份、writer提交序号及运行代次
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

-- @table operation_receipts | AP | FP-02,FP-05,FP-06,FP-09 | 业务幂等回执，与业务写入同事务提交
CREATE TABLE operation_receipts (
    operation_id TEXT NOT NULL PRIMARY KEY, -- 客户端操作幂等ID
    request_sha256 TEXT NOT NULL CHECK(instr(request_sha256,char(0))=0 AND (request_sha256 IS NULL OR (length(request_sha256)=64 AND request_sha256 NOT GLOB '*[^0-9a-f]*'))), -- 相同操作ID的规范请求摘要，冲突拒绝
    command_kind TEXT NOT NULL, -- 已注册命令种类
    commit_revision INTEGER NOT NULL CHECK(commit_revision>0), -- 提交时的本地revision
    result_refs_json TEXT NOT NULL CHECK(instr(result_refs_json,char(0))=0 AND (json_valid(result_refs_json) AND json_type(result_refs_json)='object')), -- 仅结果身份/版本与脱敏回执，不含原始秘密
    committed_at_ms INTEGER NOT NULL CHECK(committed_at_ms IS NULL OR committed_at_ms>=0) -- 提交时间
) STRICT;

-- @table games | AP | FP-01,FP-02,FP-09 | 稳定游戏库记录，移出使用归档且不删除游戏文件
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
CREATE INDEX ix_games_active ON games(archived_at_ms, updated_at_ms DESC, game_id);

-- @table game_targets | AP | FP-02,FP-03 | 用户已核对的可编辑启动/目标配置；会话另存当时身份
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
CREATE UNIQUE INDEX uq_game_target_environment_path ON game_targets(environment_key,target_path_key) WHERE archived_at_ms IS NULL AND target_path_key IS NOT NULL;

-- @table resource_snapshots | QA | FP-03,FP-07,FP-11,FP-12 | 安装包实际资源manifest的不可变快照
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

-- @table sessions | AP | FP-03,FP-04,FP-05,FP-09,FP-10 | Hook或手动来源会话，固定启动时的身份
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
CREATE INDEX ix_sessions_game_started ON sessions(game_id,started_at_ms DESC,session_id);

-- @table capture_sources | HK | FP-03,FP-04,FP-12 | 完整候选文本流身份，不按TextOutW名字/固定行号选流
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

-- @table source_selections | AP | FP-03,FP-04,FP-05 | 用户显式选正文的不可变版本，临时选择与保存规则分离
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

-- @table raw_events | HK | FP-04,FP-09,FP-12 | 不可变事件身份与payload摘要，原始payload独立限额清理
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
CREATE INDEX ix_raw_source_order ON raw_events(source_id,ingest_sequence);

-- @table raw_event_payloads | AP | FP-04,FP-09,FP-12 | 可按显式限额删除的原始字节，删除不级联历史与来源身份
CREATE TABLE raw_event_payloads (
    event_id TEXT NOT NULL PRIMARY KEY, -- 对应事件
    payload_bytes BLOB NOT NULL, -- 有界原始字节，可为空BLOB
    retained_at_ms INTEGER NOT NULL CHECK(retained_at_ms IS NULL OR retained_at_ms>=0), -- 写入时间
    FOREIGN KEY(event_id) REFERENCES raw_events(event_id) ON DELETE CASCADE
) STRICT;

-- @table reading_units | AP | FP-04,FP-09,FP-10 | 稳定阅读单元；当前文本版本由最大revision推导
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

-- @table reading_unit_revisions | AP | FP-04,FP-07,FP-08,FP-09,FP-10 | 不可变正文版本；译文/词法/学习固定引用此版本
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
CREATE INDEX ix_reading_session_history ON reading_unit_revisions(session_id,created_at_ms DESC,unit_id,text_revision);

-- @table source_refs | AP | FP-04,FP-09 | 正文版本到原始事件的可追溯跨度，payload清理后身份仍存在
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
CREATE INDEX ix_source_refs_event ON source_refs(event_id,session_id);

-- @table lexical_snapshots | JA | FP-04,FP-07,FP-08 | 不可变真实词法结果，资源/schema身份独立于正文版本
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

-- @table lexical_tokens | JA | FP-07,FP-08 | 相对原文UTF8字节的token；UTF16为投影换算值不持久作为真值
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

-- @table purpose_runtime_states | AP | FP-06,FP-08,FP-12 | 用途当前有效授权epoch；不以单纯配置显示名变更取消任务
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

-- @table request_tasks | AI | FP-04,FP-06,FP-08,FP-09 | 运行时翻译/精读/语境查词任务，身份与授权固定，配置测试另表
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
CREATE INDEX ix_tasks_unit ON request_tasks(unit_id,text_revision,purpose,created_at_ms);
CREATE INDEX ix_tasks_cancellable ON request_tasks(scope_key,purpose,state,authorization_epoch);

-- @table request_attempts | AI | FP-06,FP-08,FP-12 | 有限重试/回退的真实尝试身份，不将多结果等同回退
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

-- @table translations | AI | FP-04,FP-06,FP-09 | 完成后的独立译文；输入和配置身份通过task不可歧义追溯
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

-- @table corrections | AP | FP-09 | 人工修正独立版本，机器新结果不能覆盖
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

-- @table study_results | AI | FP-07,FP-08,FP-09 | 固定句/选词的按需解释终结果，失败/取消不伪装完成
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

-- @table cache_entries | AI | FP-06,FP-07,FP-08,FP-09 | 按实际请求/资源语义命中的派生缓存，人工修正不作为机器缓存写回
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
CREATE INDEX ix_cache_expiry ON cache_entries(expires_at_ms,cache_key);
CREATE INDEX ix_cache_lru ON cache_entries(last_accessed_at_ms,cache_key);

-- @table preference_revisions | AP | FP-04,FP-07,FP-11 | 阅读偏好独立不可变快照，不影响无关在途请求语义
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

-- @table preference_heads | AP | FP-11 | 每scope当前偏好头，可CAS更新
CREATE TABLE preference_heads (
    scope_key TEXT NOT NULL PRIMARY KEY, -- 偏好作用域
    preference_revision_id TEXT NOT NULL, -- 当前偏好快照
    head_revision INTEGER NOT NULL CHECK(head_revision>0), -- 并发编辑版本
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms IS NULL OR updated_at_ms>=0), -- 更新时间
    FOREIGN KEY(preference_revision_id,scope_key) REFERENCES preference_revisions(preference_revision_id,scope_key) ON DELETE RESTRICT
) STRICT;

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

-- @table connection_runtime_states | AP | FP-06,FP-08,FP-12 | 连接即时停用的独立安全闸，恢复必须明确执行，不随保存草稿自动开启
CREATE TABLE connection_runtime_states (
    connection_id TEXT NOT NULL PRIMARY KEY, -- 稳定连接ID，跨配置版本安全闸
    config_revision_id TEXT NOT NULL, -- 最近关联的已应用连接快照，仅供来源追溯
    is_suspended INTEGER NOT NULL CHECK(is_suspended IN (0,1)), -- 即时禁止新请求并取消在途，不能等待下一次应用
    control_revision INTEGER NOT NULL CHECK(control_revision>0), -- 安全闸CAS版本，显式恢复也递增
    reason_code TEXT, -- 脱敏停用/恢复原因
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms>=0), -- 最后明确控制时间UTC毫秒
    FOREIGN KEY(config_revision_id,connection_id) REFERENCES connections(revision_id,connection_id) ON DELETE RESTRICT
) STRICT;
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
