-- DRAFT FOR INDEPENDENT REVIEW ONLY: NOT INSTALLED, NOT PRODUCT ACCEPTANCE.
-- Source schema is exact reviewed schema2. Migration host must hold profile lease,
-- verify/backup before writing, set foreign_keys=OFF outside transaction, BEGIN IMMEDIATE,
-- execute this body, foreign_key_check + exact independently reviewed shape/old-row checks,
-- COMMIT then set foreign_keys=ON and read back 1. No writable_schema, old-table rename or network.
ALTER TABLE config_revisions ADD COLUMN codec_version INTEGER NOT NULL DEFAULT 1 CHECK(codec_version IN(1,2));


CREATE TABLE provider_preset_versions (
 preset_id TEXT NOT NULL CHECK(length(preset_id)>5 AND length(preset_id)<=160 AND preset_id NOT GLOB '*[^A-Za-z0-9_.-]*' AND instr(preset_id,char(0))=0),
 preset_revision INTEGER NOT NULL CHECK(preset_revision BETWEEN 1 AND 4294967295),
 namespace TEXT NOT NULL CHECK(namespace IN('builtin','user')),
 provider_id TEXT NOT NULL CHECK(length(provider_id)>0 AND length(provider_id)<=160 AND provider_id NOT GLOB '*[^A-Za-z0-9_.-]*' AND instr(provider_id,char(0))=0),
 provider_label TEXT NOT NULL CHECK(length(provider_label)>0 AND instr(provider_label,char(0))=0),
 display_name TEXT NOT NULL CHECK(length(display_name)>0 AND instr(display_name,char(0))=0),
 protocol_id TEXT NOT NULL CHECK(length(protocol_id)>0 AND length(protocol_id)<=160 AND protocol_id NOT GLOB '*[^A-Za-z0-9_.-]*' AND instr(protocol_id,char(0))=0),
 adapter_contract_revision INTEGER NOT NULL CHECK(adapter_contract_revision BETWEEN 1 AND 4294967295),
 compatibility_profile_id TEXT NOT NULL CHECK(length(compatibility_profile_id)>0 AND length(compatibility_profile_id)<=160 AND compatibility_profile_id NOT GLOB '*[^A-Za-z0-9_.-]*' AND instr(compatibility_profile_id,char(0))=0),
 compatibility_profile_revision INTEGER NOT NULL CHECK(compatibility_profile_revision BETWEEN 1 AND 4294967295),
 endpoint TEXT CHECK(endpoint IS NULL OR (instr(endpoint,char(0))=0 AND instr(endpoint,'?')=0 AND instr(endpoint,'#')=0)),
 auth_kind TEXT NOT NULL CHECK(auth_kind IN('none','api_key','baidu_app_secret','youdao_app_secret','google_service_account')),
 options_json TEXT NOT NULL CHECK(instr(options_json,char(0))=0 AND json_valid(options_json) AND json_type(options_json)='object'),
 content_hash TEXT NOT NULL CHECK(length(content_hash)=64 AND content_hash NOT GLOB '*[^0-9a-f]*' AND instr(content_hash,char(0))=0),
 source_json TEXT NOT NULL CHECK(instr(source_json,char(0))=0 AND json_valid(source_json) AND json_type(source_json)='array'),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 PRIMARY KEY(preset_id,preset_revision),
 UNIQUE(preset_id,preset_revision,protocol_id,adapter_contract_revision,compatibility_profile_id,compatibility_profile_revision),
 CHECK((namespace='builtin' AND substr(preset_id,1,8)='builtin.' AND length(preset_id)>8) OR (namespace='user' AND substr(preset_id,1,5)='user.'))
) STRICT;

DROP TRIGGER config_seal_identity;

DROP TRIGGER connections_no_update;

DROP TRIGGER connections_pool_not_purged;

DROP TRIGGER connections_sealed_delete;

DROP TRIGGER connections_sealed_insert;

DROP TRIGGER credential_refs_identity_immutable;

DROP TRIGGER credential_refs_lifecycle_monotonic;

DROP TRIGGER pool_gc_unreferenced;

DROP TRIGGER study_results_connection_authority;

DROP TRIGGER study_task_authority;

DROP TRIGGER translation_task_authority;

DROP TRIGGER translations_connection_authority;

CREATE TABLE connections_new (
    revision_id TEXT NOT NULL REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 所属不可变整体配置版本
    connection_id TEXT NOT NULL CHECK (length(connection_id) > 0), -- 跨版本稳定的连接ID
    display_name TEXT NOT NULL, -- 用户显示名；不参与请求语义身份
    protocol TEXT NOT NULL CHECK (length(protocol)>0 AND length(protocol)<=160 AND protocol NOT GLOB '*[^A-Za-z0-9_.-]*' AND instr(protocol,char(0))=0), -- 协议适配目标；unsupported可保存草稿但不可应用为运行用途
    deployment TEXT NOT NULL CHECK (deployment IN ('local','remote','cloud')), -- 部署位置意图；实际地址与路由仍须应用校验
    auth_mode TEXT NOT NULL CHECK (auth_mode IN ('none','api_key','baidu_app_secret','youdao_app_secret','google_service_account')), -- 认证方式；无认证只允许适配器认可的实际服务
    endpoint TEXT, -- 非秘密服务地址；未完成草稿允许为空，禁止URL内嵌凭据
    credential_pool_id TEXT, -- 显式选择的不可变凭据池ID；草稿允许未选择
    credential_pool_revision INTEGER CHECK (credential_pool_revision IS NULL OR credential_pool_revision > 0), -- 精确凭据池版本，不随池新建版本自动变化
    proxy_mode TEXT NOT NULL DEFAULT 'inherit' CHECK (proxy_mode IN ('inherit','override','disabled')), -- 网络代理继承、显式覆盖或禁用
    network_metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(network_metadata_json,char(0))=0 AND (json_valid(network_metadata_json) AND json_type(network_metadata_json) = 'object')), -- 安全网络配置白名单，例如无秘密代理地址、超时和region
    availability_intent INTEGER NOT NULL DEFAULT 0 CHECK (availability_intent IN (0,1)), -- 用户连接可用意图；运行安全停用和恢复须另经应用控制
    adapter_binding_schema INTEGER,
    adapter_contract_revision INTEGER,
    compatibility_profile_id TEXT,
    compatibility_profile_revision INTEGER,
    provider_preset_id TEXT,
    provider_preset_revision INTEGER,
    PRIMARY KEY (revision_id, connection_id),
    FOREIGN KEY (credential_pool_id, credential_pool_revision) REFERENCES credential_pool_revisions(pool_id, pool_revision) ON DELETE RESTRICT,
    CHECK ((credential_pool_id IS NULL) = (credential_pool_revision IS NULL))
    , CHECK ((adapter_binding_schema IS NULL AND adapter_contract_revision IS NULL AND compatibility_profile_id IS NULL AND compatibility_profile_revision IS NULL)
        OR (adapter_binding_schema IS NOT NULL AND adapter_binding_schema=1
            AND adapter_contract_revision IS NOT NULL AND adapter_contract_revision BETWEEN 1 AND 4294967295
            AND compatibility_profile_id IS NOT NULL AND length(compatibility_profile_id)>0 AND length(compatibility_profile_id)<=160 AND compatibility_profile_id NOT GLOB '*[^A-Za-z0-9_.-]*' AND instr(compatibility_profile_id,char(0))=0
            AND compatibility_profile_revision IS NOT NULL AND compatibility_profile_revision BETWEEN 1 AND 4294967295)),
    CHECK ((provider_preset_id IS NULL AND provider_preset_revision IS NULL)
        OR (provider_preset_id IS NOT NULL AND provider_preset_revision IS NOT NULL AND provider_preset_revision BETWEEN 1 AND 4294967295 AND adapter_binding_schema IS NOT NULL)),
    FOREIGN KEY(provider_preset_id,provider_preset_revision,protocol,adapter_contract_revision,compatibility_profile_id,compatibility_profile_revision)
        REFERENCES provider_preset_versions(preset_id,preset_revision,protocol_id,adapter_contract_revision,compatibility_profile_id,compatibility_profile_revision) ON DELETE RESTRICT
) STRICT;

CREATE TABLE credential_refs_new (
    credential_id TEXT NOT NULL PRIMARY KEY CHECK (length(credential_id) > 0), -- 单个不可变秘密版本ID；替换秘密必须生成新ID
    vault_ref TEXT NOT NULL UNIQUE CHECK (length(vault_ref) > 0), -- 系统安全库定位引用，不是Key值或可导出认证材料
    masked_label TEXT NOT NULL, -- 脱敏Key身份标签，不回显完整秘密
    auth_kind TEXT NOT NULL CHECK (auth_kind IN ('api_key','baidu_app_secret','youdao_app_secret','google_service_account','google_oauth_refresh')), -- 安全库保存的认证材料种类
    state TEXT NOT NULL CHECK (state IN ('pending','active','revoked','cleanup_pending')), -- pending不可授权；撤销或待清理均禁止发送
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0), -- pending意图写入时间，UTC毫秒
    activated_at_ms INTEGER CHECK (activated_at_ms IS NULL OR activated_at_ms >= created_at_ms), -- 系统秘密成功建立且本地承认可引用的时间
    revoked_at_ms INTEGER CHECK (revoked_at_ms IS NULL OR revoked_at_ms >= created_at_ms), -- 撤销先持久化，系统库删除失败也不得清除该时间
    cleanup_requested_at_ms INTEGER CHECK (cleanup_requested_at_ms IS NULL OR cleanup_requested_at_ms >= created_at_ms), -- 待回收或系统删除的请求时间
    cleaned_at_ms INTEGER CHECK (cleaned_at_ms IS NULL OR cleaned_at_ms >= created_at_ms), -- 系统秘密已清理时间；元数据仍可支持旧历史来源
    lifecycle_revision INTEGER NOT NULL DEFAULT 0 CHECK (lifecycle_revision >= 0), -- 生命周期CAS版本；秘密身份和vault_ref不随更新变化
    envelope_schema INTEGER,
    UNIQUE(credential_id,auth_kind),
    CHECK ((envelope_schema IS NULL AND auth_kind='api_key') OR (envelope_schema IS NOT NULL AND envelope_schema=1)),
    CHECK (state <> 'active' OR (activated_at_ms IS NOT NULL AND revoked_at_ms IS NULL AND cleaned_at_ms IS NULL)),
    CHECK (state NOT IN ('revoked','cleanup_pending') OR revoked_at_ms IS NOT NULL),
    CHECK (cleaned_at_ms IS NULL OR state = 'cleanup_pending')
) STRICT;

INSERT INTO connections_new (revision_id,connection_id,display_name,protocol,deployment,auth_mode,endpoint,credential_pool_id,credential_pool_revision,proxy_mode,network_metadata_json,availability_intent,adapter_binding_schema,adapter_contract_revision,compatibility_profile_id,compatibility_profile_revision,provider_preset_id,provider_preset_revision) SELECT revision_id,connection_id,display_name,protocol,deployment,auth_mode,endpoint,credential_pool_id,credential_pool_revision,proxy_mode,network_metadata_json,availability_intent,NULL,NULL,NULL,NULL,NULL,NULL FROM connections;

DROP TABLE connections;

ALTER TABLE connections_new RENAME TO connections;

INSERT INTO credential_refs_new (credential_id,vault_ref,masked_label,auth_kind,state,created_at_ms,activated_at_ms,revoked_at_ms,cleanup_requested_at_ms,cleaned_at_ms,lifecycle_revision,envelope_schema) SELECT credential_id,vault_ref,masked_label,auth_kind,state,created_at_ms,activated_at_ms,revoked_at_ms,cleanup_requested_at_ms,cleaned_at_ms,lifecycle_revision,NULL FROM credential_refs;

DROP TABLE credential_refs;

ALTER TABLE credential_refs_new RENAME TO credential_refs;

CREATE INDEX connections_by_credential_pool ON connections(credential_pool_id, credential_pool_revision);

CREATE INDEX credential_refs_cleanup ON credential_refs(state, cleanup_requested_at_ms);

CREATE TRIGGER config_seal_identity BEFORE INSERT ON config_revision_seals
WHEN NOT EXISTS(SELECT 1 FROM config_revisions c WHERE c.revision_id=NEW.revision_id AND c.content_hash=NEW.content_hash)
 OR EXISTS(SELECT 1 FROM connections c WHERE c.revision_id=NEW.revision_id AND c.credential_pool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM credential_pool_seals s WHERE s.pool_id=c.credential_pool_id AND s.pool_revision=c.credential_pool_revision AND NOT EXISTS(SELECT 1 FROM credential_pool_gc_marks g WHERE g.pool_id=s.pool_id AND g.pool_revision=s.pool_revision)))
BEGIN SELECT RAISE(ABORT,'invalid_config_seal'); END;

CREATE TRIGGER connections_no_update BEFORE UPDATE ON connections BEGIN SELECT RAISE(ABORT, 'immutable_connection_snapshot'); END;

CREATE TRIGGER connections_pool_not_purged BEFORE INSERT ON connections
WHEN NEW.credential_pool_id IS NOT NULL AND EXISTS(SELECT 1 FROM credential_pool_gc_marks WHERE pool_id=NEW.credential_pool_id AND pool_revision=NEW.credential_pool_revision)
BEGIN SELECT RAISE(ABORT,'pool_already_purged'); END;

CREATE TRIGGER connections_sealed_delete BEFORE DELETE ON connections WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;

CREATE TRIGGER connections_sealed_insert BEFORE INSERT ON connections WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;

CREATE TRIGGER credential_refs_identity_immutable BEFORE UPDATE ON credential_refs
WHEN NEW.envelope_schema IS NOT OLD.envelope_schema OR NEW.credential_id IS NOT OLD.credential_id OR NEW.vault_ref IS NOT OLD.vault_ref OR NEW.masked_label IS NOT OLD.masked_label OR NEW.auth_kind IS NOT OLD.auth_kind OR NEW.created_at_ms IS NOT OLD.created_at_ms
BEGIN SELECT RAISE(ABORT, 'immutable_credential_identity'); END;

CREATE TRIGGER credential_refs_lifecycle_monotonic BEFORE UPDATE ON credential_refs
WHEN NEW.lifecycle_revision <> OLD.lifecycle_revision + 1 OR (OLD.state IN ('revoked','cleanup_pending') AND NEW.state IN ('pending','active')) OR (OLD.state = 'active' AND NEW.state = 'pending') OR (OLD.activated_at_ms IS NOT NULL AND NEW.activated_at_ms IS NOT OLD.activated_at_ms) OR (OLD.revoked_at_ms IS NOT NULL AND NEW.revoked_at_ms IS NOT OLD.revoked_at_ms) OR (OLD.cleaned_at_ms IS NOT NULL AND NEW.cleaned_at_ms IS NOT OLD.cleaned_at_ms)
BEGIN SELECT RAISE(ABORT, 'invalid_credential_lifecycle_transition'); END;

CREATE TRIGGER pool_gc_unreferenced BEFORE INSERT ON credential_pool_gc_marks
WHEN EXISTS(SELECT 1 FROM connections WHERE credential_pool_id=NEW.pool_id AND credential_pool_revision=NEW.pool_revision)
 OR EXISTS(SELECT 1 FROM test_record_credentials WHERE pool_id=NEW.pool_id AND pool_revision=NEW.pool_revision)
BEGIN SELECT RAISE(ABORT,'pool_gc_still_referenced'); END;

CREATE TRIGGER study_results_connection_authority BEFORE INSERT ON study_results BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM request_tasks t JOIN request_attempts a ON a.task_id=t.task_id AND a.attempt_id=NEW.attempt_id JOIN binding_engines e ON e.revision_id=a.config_revision_id AND e.binding_id=a.binding_id AND e.engine_id=a.actual_engine_id AND e.purpose=a.purpose JOIN purpose_bindings b ON b.revision_id=e.revision_id AND b.binding_id=e.binding_id AND b.purpose=e.purpose JOIN models m ON m.revision_id=e.revision_id AND m.model_id=e.model_id JOIN connections c ON c.revision_id=m.revision_id AND c.connection_id=m.connection_id JOIN connection_runtime_states r ON r.connection_id=c.connection_id WHERE t.task_id=NEW.task_id AND t.purpose IN ('grammar','context_word') AND e.enabled=1 AND b.enabled=1 AND b.binding_mode='override' AND c.availability_intent=1 AND r.is_suspended=0 AND ((c.auth_mode='none' AND a.credential_id IS NULL) OR (c.auth_mode IN ('api_key','baidu_app_secret','youdao_app_secret','google_service_account') AND EXISTS(SELECT 1 FROM credential_pool_members pm JOIN credential_refs cr ON cr.credential_id=pm.credential_id WHERE pm.pool_id=c.credential_pool_id AND pm.pool_revision=c.credential_pool_revision AND pm.credential_id=a.credential_id AND pm.enabled=1 AND cr.state='active' AND cr.auth_kind=c.auth_mode)))) THEN RAISE(ABORT,'study_results_connection_authority') END;
END;

CREATE TRIGGER study_task_authority BEFORE INSERT ON study_results BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM request_tasks t JOIN profile_meta p ON p.singleton=1 JOIN purpose_runtime_states s ON s.scope_key=t.scope_key AND s.purpose=t.purpose JOIN request_attempts a ON a.attempt_id=NEW.attempt_id AND a.task_id=t.task_id LEFT JOIN credential_refs c ON c.credential_id=a.credential_id WHERE t.task_id=NEW.task_id AND t.purpose IN ('grammar','context_word') AND t.state='completed' AND p.runtime_state='active' AND t.profile_generation=p.runtime_generation AND t.server_instance_id=p.server_instance_id AND s.is_enabled=1 AND t.authorization_epoch=s.authorization_epoch AND t.binding_id=s.binding_id AND a.state='completed' AND (a.credential_id IS NULL OR c.state='active')) THEN RAISE(ABORT,'study_task_authority') END;
 SELECT CASE WHEN NEW.selection_byte_end IS NOT NULL AND NEW.selection_byte_end>(SELECT r.source_byte_length FROM request_tasks t JOIN reading_unit_revisions r ON r.unit_id=t.unit_id AND r.text_revision=t.text_revision WHERE t.task_id=NEW.task_id) THEN RAISE(ABORT,'study_selection_bounds') END;
END;

CREATE TRIGGER translation_task_authority BEFORE INSERT ON translations BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM request_tasks t JOIN profile_meta p ON p.singleton=1 JOIN purpose_runtime_states s ON s.scope_key=t.scope_key AND s.purpose=t.purpose JOIN request_attempts a ON a.attempt_id=NEW.attempt_id AND a.task_id=t.task_id LEFT JOIN credential_refs c ON c.credential_id=a.credential_id WHERE t.task_id=NEW.task_id AND t.purpose='translation' AND t.state='completed' AND p.runtime_state='active' AND t.profile_generation=p.runtime_generation AND t.server_instance_id=p.server_instance_id AND s.is_enabled=1 AND t.authorization_epoch=s.authorization_epoch AND t.binding_id=s.binding_id AND a.state='completed' AND (a.credential_id IS NULL OR c.state='active')) THEN RAISE(ABORT,'translation_task_authority') END;
END;

CREATE TRIGGER translations_connection_authority BEFORE INSERT ON translations BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM request_tasks t JOIN request_attempts a ON a.task_id=t.task_id AND a.attempt_id=NEW.attempt_id JOIN binding_engines e ON e.revision_id=a.config_revision_id AND e.binding_id=a.binding_id AND e.engine_id=a.actual_engine_id AND e.purpose=a.purpose JOIN purpose_bindings b ON b.revision_id=e.revision_id AND b.binding_id=e.binding_id AND b.purpose=e.purpose JOIN models m ON m.revision_id=e.revision_id AND m.model_id=e.model_id JOIN connections c ON c.revision_id=m.revision_id AND c.connection_id=m.connection_id JOIN connection_runtime_states r ON r.connection_id=c.connection_id WHERE t.task_id=NEW.task_id AND t.purpose='translation' AND e.enabled=1 AND b.enabled=1 AND b.binding_mode='override' AND c.availability_intent=1 AND r.is_suspended=0 AND ((c.auth_mode='none' AND a.credential_id IS NULL) OR (c.auth_mode IN ('api_key','baidu_app_secret','youdao_app_secret','google_service_account') AND EXISTS(SELECT 1 FROM credential_pool_members pm JOIN credential_refs cr ON cr.credential_id=pm.credential_id WHERE pm.pool_id=c.credential_pool_id AND pm.pool_revision=c.credential_pool_revision AND pm.credential_id=a.credential_id AND pm.enabled=1 AND cr.state='active' AND cr.auth_kind=c.auth_mode)))) THEN RAISE(ABORT,'translations_connection_authority') END;
END;

CREATE TRIGGER connections_codec_shape BEFORE INSERT ON connections
WHEN (SELECT codec_version FROM config_revisions WHERE revision_id=NEW.revision_id)=1
 AND (NEW.adapter_binding_schema IS NOT NULL OR NEW.provider_preset_id IS NOT NULL
      OR NEW.protocol NOT IN('openai_compatible','anthropic','azure_translator','unsupported')
      OR NEW.auth_mode NOT IN('none','api_key'))
BEGIN SELECT RAISE(ABORT,'legacy_codec_extension_forbidden'); END;
CREATE TRIGGER provider_presets_no_update BEFORE UPDATE ON provider_preset_versions BEGIN SELECT RAISE(ABORT,'immutable_provider_preset'); END;
CREATE TRIGGER provider_presets_no_delete BEFORE DELETE ON provider_preset_versions BEGIN SELECT RAISE(ABORT,'immutable_provider_preset'); END;
CREATE TRIGGER provider_presets_safe_options BEFORE INSERT ON provider_preset_versions
WHEN EXISTS(SELECT 1 FROM json_each(NEW.options_json) WHERE key IN('api_key','app_secret','service_account_json','credential_ref','vault_ref','credential_pool_id','headers','authorization','script','wire_mapping','protocol','model','endpoint','auth_mode') OR type NOT IN('text','integer','real','true','false'))
BEGIN SELECT RAISE(ABORT,'protected_provider_preset_option'); END;

