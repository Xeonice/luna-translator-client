-- FROZEN INDEPENDENT SCHEMA4 TECHNICAL DESIGN: admitted to implement, not migration/product acceptance.
-- Source 53958d..., 46 new tables reduced to 28 by reusing the existing config graph and typed provenance.
DROP TRIGGER "test_records_requires_seal_update";

DROP TRIGGER "test_records_requires_seal_insert";

DROP TRIGGER "config_gc_unreferenced";

DROP TRIGGER "test_records_immutable_result";

DROP TRIGGER "config_revisions_no_update";

DROP TRIGGER "binding_engines_no_update";

DROP TRIGGER "binding_engines_sealed_insert";

DROP TRIGGER "binding_engines_sealed_delete";

DROP TRIGGER "config_seal_identity";

DROP TRIGGER "connections_no_update";

DROP TRIGGER "connections_pool_not_purged";

DROP TRIGGER "connections_sealed_delete";

DROP TRIGGER "connections_sealed_insert";

DROP TRIGGER "pool_gc_unreferenced";

DROP TRIGGER "study_results_connection_authority";

DROP TRIGGER "translations_connection_authority";

DROP TRIGGER "connections_codec_shape";

DROP TRIGGER "provider_presets_no_update";

DROP TRIGGER "provider_presets_no_delete";

DROP TRIGGER "provider_presets_safe_options";

CREATE TABLE config_revisions_v4 (
    revision_id TEXT NOT NULL PRIMARY KEY CHECK (length(revision_id) > 0), -- 不可变整体配置版本ID，与请求语义指纹分开
    parent_revision_id TEXT REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 来源版本；首个版本允许为空
    content_hash TEXT NOT NULL CHECK (instr(content_hash,char(0))=0 AND (length(content_hash) = 64 AND content_hash NOT GLOB '*[^0-9a-f]*')), -- 规范化非秘密配置内容的SHA256
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0), -- 保存时间，UTC毫秒
    save_reason TEXT NOT NULL CHECK (save_reason IN ('create','edit','copy','import','restore','migrate')), -- 形成独立快照的原因
    local_validation_json TEXT NOT NULL DEFAULT '{}' CHECK (instr(local_validation_json,char(0))=0 AND (json_valid(local_validation_json) AND json_type(local_validation_json) = 'object')), codec_version INTEGER NOT NULL DEFAULT 1 CHECK(codec_version IN(1,2,3)), -- 本地缺项与字段错误摘要；不是网络测试通过标记
    revision_role TEXT NOT NULL DEFAULT 'user' CHECK(revision_role IN('user','activation')),
    CHECK (parent_revision_id IS NULL OR parent_revision_id <> revision_id)
) STRICT;

INSERT INTO config_revisions_v4 ("revision_id","parent_revision_id","content_hash","created_at_ms","save_reason","local_validation_json","codec_version") SELECT "revision_id","parent_revision_id","content_hash","created_at_ms","save_reason","local_validation_json","codec_version" FROM "config_revisions";

DROP TABLE "config_revisions";

ALTER TABLE config_revisions_v4 RENAME TO "config_revisions";

ALTER TABLE models ADD COLUMN template_purpose TEXT CHECK(template_purpose IS NULL OR template_purpose IN('translation','grammar','context_word'));

ALTER TABLE purpose_bindings ADD COLUMN semantics_version INTEGER NOT NULL DEFAULT 2 CHECK(semantics_version IN(2,3));

CREATE TABLE oauth_client_profiles (
 client_profile_id TEXT NOT NULL CHECK(length(client_profile_id)>0 AND instr(client_profile_id,char(0))=0),
 client_profile_revision INTEGER NOT NULL CHECK(client_profile_revision BETWEEN 1 AND 4294967295),
 issuer TEXT NOT NULL CHECK(issuer='google_accounts'),
 public_client_id TEXT NOT NULL CHECK(length(CAST(public_client_id AS BLOB)) BETWEEN 1 AND 512 AND instr(public_client_id,char(0))=0),
 client_mode TEXT NOT NULL CHECK(client_mode IN('application','custom')),
 redirect_policy TEXT NOT NULL CHECK(redirect_policy='loopback_pkce'),
 policy_sha256 TEXT NOT NULL CHECK(length(CAST(policy_sha256 AS BLOB))=64 AND policy_sha256 NOT GLOB '*[^0-9a-f]*' AND instr(policy_sha256,char(0))=0),
 setup_state TEXT NOT NULL CHECK(setup_state IN('unknown','verified','unsupported')),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 PRIMARY KEY(client_profile_id,client_profile_revision)
) STRICT;

CREATE TABLE oauth_grants (
 grant_id TEXT NOT NULL CHECK(length(grant_id)>0 AND instr(grant_id,char(0))=0),
 client_profile_id TEXT NOT NULL CHECK(length(client_profile_id)>0 AND instr(client_profile_id,char(0))=0),
 client_profile_revision INTEGER NOT NULL CHECK(client_profile_revision BETWEEN 1 AND 4294967295),
 principal_state TEXT NOT NULL CHECK(principal_state IN('unknown','verified')),
 principal_subject TEXT CHECK(principal_subject IS NULL OR (length(CAST(principal_subject AS BLOB)) BETWEEN 1 AND 512 AND instr(principal_subject,char(0))=0)),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 authorization_epoch INTEGER CHECK(authorization_epoch IS NULL OR authorization_epoch BETWEEN 1 AND 4294967295),
 material_revision INTEGER CHECK(material_revision IS NULL OR material_revision BETWEEN 1 AND 4294967295),
 control_revision INTEGER NOT NULL DEFAULT 0 CHECK(control_revision BETWEEN 0 AND 4294967295),
 PRIMARY KEY(grant_id),
 FOREIGN KEY(grant_id,authorization_epoch) REFERENCES oauth_authorization_snapshots(grant_id,authorization_epoch) DEFERRABLE INITIALLY DEFERRED,
 FOREIGN KEY(grant_id,material_revision) REFERENCES oauth_token_material_revisions(grant_id,material_revision) DEFERRABLE INITIALLY DEFERRED,
 CHECK((principal_state='unknown' AND principal_subject IS NULL) OR (principal_state='verified' AND principal_subject IS NOT NULL)),
 FOREIGN KEY(client_profile_id,client_profile_revision) REFERENCES oauth_client_profiles(client_profile_id,client_profile_revision) ON DELETE RESTRICT
) STRICT;

CREATE TABLE oauth_authorization_snapshots (
 grant_id TEXT NOT NULL CHECK(length(grant_id)>0 AND instr(grant_id,char(0))=0),
 authorization_epoch INTEGER NOT NULL CHECK(authorization_epoch BETWEEN 1 AND 4294967295),
 state TEXT NOT NULL CHECK(state IN('pending','active','revoked','needs_reauth')),
 scope TEXT NOT NULL DEFAULT 'https://www.googleapis.com/auth/cloud-translation' CHECK(scope='https://www.googleapis.com/auth/cloud-translation'),
 scopes_sha256 TEXT NOT NULL CHECK(length(CAST(scopes_sha256 AS BLOB))=64 AND scopes_sha256 NOT GLOB '*[^0-9a-f]*' AND instr(scopes_sha256,char(0))=0),
 previous_epoch INTEGER CHECK(previous_epoch IS NULL OR (previous_epoch BETWEEN 1 AND 4294967295)),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 PRIMARY KEY(grant_id,authorization_epoch),
 FOREIGN KEY(grant_id) REFERENCES oauth_grants(grant_id) ON DELETE RESTRICT,
 FOREIGN KEY(grant_id,previous_epoch) REFERENCES oauth_authorization_snapshots(grant_id,authorization_epoch) ON DELETE RESTRICT,
 CHECK(previous_epoch IS NULL OR previous_epoch<authorization_epoch)
) STRICT;

CREATE TABLE oauth_token_material_revisions (
 grant_id TEXT NOT NULL CHECK(length(grant_id)>0 AND instr(grant_id,char(0))=0),
 material_revision INTEGER NOT NULL CHECK(material_revision BETWEEN 1 AND 4294967295),
 credential_id TEXT NOT NULL CHECK(length(credential_id)>0 AND instr(credential_id,char(0))=0),
 auth_kind TEXT NOT NULL CHECK(auth_kind='google_oauth_refresh'),
 parent_material_revision INTEGER CHECK(parent_material_revision IS NULL OR (parent_material_revision BETWEEN 1 AND 4294967295)),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 PRIMARY KEY(grant_id,material_revision),
 UNIQUE(grant_id,material_revision,credential_id),
 UNIQUE(credential_id),
 FOREIGN KEY(grant_id) REFERENCES oauth_grants(grant_id) ON DELETE RESTRICT,
 FOREIGN KEY(grant_id,parent_material_revision) REFERENCES oauth_token_material_revisions(grant_id,material_revision) ON DELETE RESTRICT,
 FOREIGN KEY(credential_id,auth_kind) REFERENCES credential_refs(credential_id,auth_kind) ON DELETE RESTRICT,
 CHECK(parent_material_revision IS NULL OR parent_material_revision<material_revision)
) STRICT;

CREATE TABLE connections_v4 (
    revision_id TEXT NOT NULL REFERENCES config_revisions(revision_id) ON DELETE RESTRICT, -- 所属不可变整体配置版本
    connection_id TEXT NOT NULL CHECK (length(connection_id) > 0), -- 跨版本稳定的连接ID
    display_name TEXT NOT NULL, -- 用户显示名；不参与请求语义身份
    protocol TEXT NOT NULL CHECK (length(protocol)>0 AND length(protocol)<=160 AND protocol NOT GLOB '*[^A-Za-z0-9_.-]*' AND instr(protocol,char(0))=0), -- 协议适配目标；unsupported可保存草稿但不可应用为运行用途
    deployment TEXT NOT NULL CHECK (deployment IN ('local','remote','cloud')), -- 部署位置意图；实际地址与路由仍须应用校验
    auth_mode TEXT NOT NULL CHECK (auth_mode IN ('none','api_key','baidu_app_secret','youdao_app_secret','google_service_account','google_oauth')), -- 认证方式；无认证只允许适配器认可的实际服务
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
    http_options_schema INTEGER,
    http_options_json TEXT CHECK(http_options_json IS NULL OR (instr(http_options_json,char(0))=0 AND length(CAST(http_options_json AS BLOB))<=65536 AND json_valid(http_options_json) AND json_type(http_options_json)='object')),
    origin_schema INTEGER,
    origin_json TEXT CHECK(origin_json IS NULL OR (instr(origin_json,char(0))=0 AND length(CAST(origin_json AS BLOB))<=65536 AND json_valid(origin_json) AND json_type(origin_json)='object')),
    oauth_grant_id TEXT,
    oauth_authorization_epoch INTEGER,
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
,
    CHECK((http_options_schema IS NULL AND http_options_json IS NULL) OR (http_options_schema IS NOT NULL AND http_options_schema=1 AND http_options_json IS NOT NULL AND json_type(http_options_json,'$.protocol') IS 'text' AND json_extract(http_options_json,'$.protocol')=protocol)),
    CHECK((origin_schema IS NULL AND origin_json IS NULL) OR (origin_schema IS NOT NULL AND origin_schema=1 AND origin_json IS NOT NULL AND json_extract(origin_json,'$.kind') IN('preset','custom'))),
    CHECK((oauth_grant_id IS NULL AND oauth_authorization_epoch IS NULL) OR (oauth_grant_id IS NOT NULL AND oauth_authorization_epoch IS NOT NULL AND oauth_authorization_epoch BETWEEN 1 AND 4294967295 AND auth_mode='google_oauth')),
    CHECK(auth_mode<>'google_oauth' OR (credential_pool_id IS NULL AND credential_pool_revision IS NULL)),
    FOREIGN KEY(oauth_grant_id,oauth_authorization_epoch) REFERENCES oauth_authorization_snapshots(grant_id,authorization_epoch) ON DELETE RESTRICT
) STRICT;

INSERT INTO connections_v4 ("revision_id","connection_id","display_name","protocol","deployment","auth_mode","endpoint","credential_pool_id","credential_pool_revision","proxy_mode","network_metadata_json","availability_intent","adapter_binding_schema","adapter_contract_revision","compatibility_profile_id","compatibility_profile_revision","provider_preset_id","provider_preset_revision") SELECT "revision_id","connection_id","display_name","protocol","deployment","auth_mode","endpoint","credential_pool_id","credential_pool_revision","proxy_mode","network_metadata_json","availability_intent","adapter_binding_schema","adapter_contract_revision","compatibility_profile_id","compatibility_profile_revision","provider_preset_id","provider_preset_revision" FROM "connections";

DROP TABLE "connections";

ALTER TABLE connections_v4 RENAME TO "connections";

CREATE TABLE provider_preset_versions_v4 (
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
 auth_kind TEXT NOT NULL CHECK(auth_kind IN('none','api_key','baidu_app_secret','youdao_app_secret','google_service_account','google_oauth')),
 options_json TEXT NOT NULL CHECK(instr(options_json,char(0))=0 AND json_valid(options_json) AND json_type(options_json)='object'),
 content_hash TEXT NOT NULL CHECK(length(content_hash)=64 AND content_hash NOT GLOB '*[^0-9a-f]*' AND instr(content_hash,char(0))=0),
 source_json TEXT NOT NULL CHECK(instr(source_json,char(0))=0 AND json_valid(source_json) AND json_type(source_json)='array'),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 PRIMARY KEY(preset_id,preset_revision),
 UNIQUE(preset_id,preset_revision,protocol_id,adapter_contract_revision,compatibility_profile_id,compatibility_profile_revision),
 CHECK((namespace='builtin' AND substr(preset_id,1,8)='builtin.' AND length(preset_id)>8) OR (namespace='user' AND substr(preset_id,1,5)='user.'))
) STRICT;

INSERT INTO provider_preset_versions_v4 ("preset_id","preset_revision","namespace","provider_id","provider_label","display_name","protocol_id","adapter_contract_revision","compatibility_profile_id","compatibility_profile_revision","endpoint","auth_kind","options_json","content_hash","source_json","created_at_ms") SELECT "preset_id","preset_revision","namespace","provider_id","provider_label","display_name","protocol_id","adapter_contract_revision","compatibility_profile_id","compatibility_profile_revision","endpoint","auth_kind","options_json","content_hash","source_json","created_at_ms" FROM "provider_preset_versions";

DROP TABLE "provider_preset_versions";

ALTER TABLE provider_preset_versions_v4 RENAME TO "provider_preset_versions";

CREATE TABLE offline_runtime_profiles (
 runtime_profile_id TEXT NOT NULL CHECK(length(runtime_profile_id)>0 AND instr(runtime_profile_id,char(0))=0),
 runtime_profile_revision INTEGER NOT NULL CHECK(runtime_profile_revision BETWEEN 1 AND 4294967295),
 engine_id TEXT NOT NULL CHECK(length(engine_id)>0 AND instr(engine_id,char(0))=0),
 engine_contract_revision INTEGER NOT NULL CHECK(engine_contract_revision BETWEEN 1 AND 4294967295),
 backend TEXT NOT NULL CHECK(backend IN('platform_native','local_worker')),
 platform TEXT NOT NULL CHECK(platform IN('macos','windows','crossover')),
 architecture TEXT NOT NULL CHECK(architecture IN('arm64','x86','x64')),
 private_protocol_revision INTEGER CHECK(private_protocol_revision IS NULL OR (private_protocol_revision BETWEEN 1 AND 4294967295)),
 abi_profile_id TEXT CHECK(abi_profile_id IS NULL OR (length(CAST(abi_profile_id AS BLOB)) BETWEEN 1 AND 512 AND instr(abi_profile_id,char(0))=0)),
 verification_state TEXT NOT NULL CHECK(verification_state IN('unknown','verified','unsupported')),
 policy_sha256 TEXT NOT NULL CHECK(length(CAST(policy_sha256 AS BLOB))=64 AND policy_sha256 NOT GLOB '*[^0-9a-f]*' AND instr(policy_sha256,char(0))=0),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 PRIMARY KEY(runtime_profile_id,runtime_profile_revision),
 UNIQUE(runtime_profile_id,runtime_profile_revision,engine_id,engine_contract_revision,backend)
) STRICT;

CREATE TABLE offline_execution_environments (
 environment_id TEXT NOT NULL CHECK(length(environment_id)>0 AND instr(environment_id,char(0))=0),
 environment_revision INTEGER NOT NULL CHECK(environment_revision BETWEEN 1 AND 4294967295),
 platform TEXT NOT NULL CHECK(platform IN('macos','windows','crossover')),
 architecture TEXT NOT NULL CHECK(architecture IN('arm64','x86','x64')),
 machine_fingerprint TEXT NOT NULL CHECK(length(CAST(machine_fingerprint AS BLOB))=64 AND machine_fingerprint NOT GLOB '*[^0-9a-f]*' AND instr(machine_fingerprint,char(0))=0),
 os_build TEXT NOT NULL CHECK(length(CAST(os_build AS BLOB)) BETWEEN 1 AND 512 AND instr(os_build,char(0))=0),
 bottle_identity TEXT CHECK(bottle_identity IS NULL OR (length(CAST(bottle_identity AS BLOB)) BETWEEN 1 AND 512 AND instr(bottle_identity,char(0))=0)),
 local_prefix_path TEXT CHECK(local_prefix_path IS NULL OR (length(CAST(local_prefix_path AS BLOB)) BETWEEN 1 AND 8192 AND instr(local_prefix_path,char(0))=0)),
 wine_build TEXT CHECK(wine_build IS NULL OR (length(CAST(wine_build AS BLOB)) BETWEEN 1 AND 512 AND instr(wine_build,char(0))=0)),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 PRIMARY KEY(environment_id,environment_revision),
 CHECK((platform='crossover' AND bottle_identity IS NOT NULL AND local_prefix_path IS NOT NULL AND wine_build IS NOT NULL) OR (platform<>'crossover' AND bottle_identity IS NULL AND local_prefix_path IS NULL AND wine_build IS NULL))
) STRICT;

CREATE TABLE offline_local_grants (
 grant_id TEXT NOT NULL CHECK(length(grant_id)>0 AND instr(grant_id,char(0))=0),
 machine_fingerprint TEXT NOT NULL CHECK(length(CAST(machine_fingerprint AS BLOB))=64 AND machine_fingerprint NOT GLOB '*[^0-9a-f]*' AND instr(machine_fingerprint,char(0))=0),
 opaque_locator TEXT NOT NULL CHECK(length(CAST(opaque_locator AS BLOB)) BETWEEN 1 AND 512 AND instr(opaque_locator,char(0))=0),
 authorization_epoch INTEGER NOT NULL CHECK(authorization_epoch BETWEEN 1 AND 4294967295),
 state TEXT NOT NULL CHECK(state IN('unknown','active','revoked')),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 PRIMARY KEY(grant_id)
) STRICT;

CREATE TABLE offline_installation_versions (
 installation_id TEXT NOT NULL CHECK(length(installation_id)>0 AND instr(installation_id,char(0))=0),
 installation_revision INTEGER NOT NULL CHECK(installation_revision BETWEEN 1 AND 4294967295),
 engine_id TEXT NOT NULL CHECK(length(engine_id)>0 AND instr(engine_id,char(0))=0),
 engine_contract_revision INTEGER NOT NULL CHECK(engine_contract_revision BETWEEN 1 AND 4294967295),
 environment_id TEXT NOT NULL CHECK(length(environment_id)>0 AND instr(environment_id,char(0))=0),
 environment_revision INTEGER NOT NULL CHECK(environment_revision BETWEEN 1 AND 4294967295),
 local_native_path TEXT NOT NULL CHECK(length(CAST(local_native_path AS BLOB)) BETWEEN 1 AND 8192 AND instr(local_native_path,char(0))=0),
 local_grant_id TEXT NOT NULL CHECK(length(local_grant_id)>0 AND instr(local_grant_id,char(0))=0),
 runtime_profile_id TEXT NOT NULL CHECK(length(runtime_profile_id)>0 AND instr(runtime_profile_id,char(0))=0),
 runtime_profile_revision INTEGER NOT NULL CHECK(runtime_profile_revision BETWEEN 1 AND 4294967295),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 PRIMARY KEY(installation_id,installation_revision),
 UNIQUE(installation_id,installation_revision,engine_id,engine_contract_revision),
 FOREIGN KEY(environment_id,environment_revision) REFERENCES offline_execution_environments(environment_id,environment_revision) ON DELETE RESTRICT,
 FOREIGN KEY(local_grant_id) REFERENCES offline_local_grants(grant_id) ON DELETE RESTRICT,
 FOREIGN KEY(runtime_profile_id,runtime_profile_revision,engine_id,engine_contract_revision) REFERENCES offline_runtime_profiles(runtime_profile_id,runtime_profile_revision,engine_id,engine_contract_revision) ON DELETE RESTRICT
) STRICT;

CREATE TABLE offline_installation_heads (
 installation_id TEXT NOT NULL CHECK(length(installation_id)>0 AND instr(installation_id,char(0))=0),
 installation_revision INTEGER NOT NULL CHECK(installation_revision BETWEEN 1 AND 4294967295),
 row_revision INTEGER NOT NULL CHECK(row_revision BETWEEN 1 AND 4294967295),
 enabled INTEGER NOT NULL CHECK(enabled IN(0,1)),
 PRIMARY KEY(installation_id),
 FOREIGN KEY(installation_id,installation_revision) REFERENCES offline_installation_versions(installation_id,installation_revision) ON DELETE RESTRICT
) STRICT;

CREATE TABLE offline_resource_snapshots (
 snapshot_id TEXT NOT NULL CHECK(length(snapshot_id)>0 AND instr(snapshot_id,char(0))=0),
 engine_id TEXT NOT NULL CHECK(length(engine_id)>0 AND instr(engine_id,char(0))=0),
 engine_contract_revision INTEGER NOT NULL CHECK(engine_contract_revision BETWEEN 1 AND 4294967295),
 backend TEXT NOT NULL CHECK(backend IN('platform_native','local_worker')),
 runtime_profile_id TEXT NOT NULL CHECK(length(runtime_profile_id)>0 AND instr(runtime_profile_id,char(0))=0),
 runtime_profile_revision INTEGER NOT NULL CHECK(runtime_profile_revision BETWEEN 1 AND 4294967295),
 identity_kind TEXT NOT NULL CHECK(identity_kind IN('file_manifest','apple_observation')),
 manifest_sha256 TEXT CHECK(manifest_sha256 IS NULL OR (length(CAST(manifest_sha256 AS BLOB))=64 AND manifest_sha256 NOT GLOB '*[^0-9a-f]*' AND instr(manifest_sha256,char(0))=0)),
 os_build TEXT CHECK(os_build IS NULL OR (length(CAST(os_build AS BLOB)) BETWEEN 1 AND 512 AND instr(os_build,char(0))=0)),
 bridge_build_sha256 TEXT CHECK(bridge_build_sha256 IS NULL OR (length(CAST(bridge_build_sha256 AS BLOB))=64 AND bridge_build_sha256 NOT GLOB '*[^0-9a-f]*' AND instr(bridge_build_sha256,char(0))=0)),
 api_profile TEXT CHECK(api_profile IS NULL OR (length(CAST(api_profile AS BLOB)) BETWEEN 1 AND 512 AND instr(api_profile,char(0))=0)),
 strategy TEXT CHECK(strategy IS NULL OR strategy='traditional_low_latency'),
 source_language TEXT CHECK(source_language IS NULL OR (length(CAST(source_language AS BLOB)) BETWEEN 1 AND 32 AND instr(source_language,char(0))=0)),
 target_language TEXT CHECK(target_language IS NULL OR (length(CAST(target_language AS BLOB)) BETWEEN 1 AND 32 AND instr(target_language,char(0))=0)),
 observation_epoch INTEGER CHECK(observation_epoch IS NULL OR (observation_epoch BETWEEN 1 AND 4294967295)),
 content_hash TEXT NOT NULL CHECK(length(CAST(content_hash AS BLOB))=64 AND content_hash NOT GLOB '*[^0-9a-f]*' AND instr(content_hash,char(0))=0),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 PRIMARY KEY(snapshot_id),
 UNIQUE(snapshot_id,engine_id,engine_contract_revision,backend),
 FOREIGN KEY(runtime_profile_id,runtime_profile_revision,engine_id,engine_contract_revision,backend) REFERENCES offline_runtime_profiles(runtime_profile_id,runtime_profile_revision,engine_id,engine_contract_revision,backend) ON DELETE RESTRICT,
 CHECK((identity_kind='file_manifest' AND backend='local_worker' AND manifest_sha256 IS NOT NULL AND os_build IS NULL AND bridge_build_sha256 IS NULL AND api_profile IS NULL AND strategy IS NULL AND source_language IS NULL AND target_language IS NULL AND observation_epoch IS NULL) OR (identity_kind='apple_observation' AND backend='platform_native' AND engine_id='apple_translation' AND manifest_sha256 IS NULL AND os_build IS NOT NULL AND bridge_build_sha256 IS NOT NULL AND api_profile IS NOT NULL AND strategy IS NOT NULL AND source_language IS NOT NULL AND target_language IS NOT NULL AND observation_epoch IS NOT NULL))
) STRICT;

CREATE TABLE offline_resource_snapshot_files (
 snapshot_id TEXT NOT NULL CHECK(length(snapshot_id)>0 AND instr(snapshot_id,char(0))=0),
 relative_key TEXT NOT NULL CHECK(length(CAST(relative_key AS BLOB)) BETWEEN 1 AND 1024 AND instr(relative_key,char(0))=0),
 byte_length INTEGER NOT NULL CHECK(byte_length>=0),
 sha256 TEXT NOT NULL CHECK(length(CAST(sha256 AS BLOB))=64 AND sha256 NOT GLOB '*[^0-9a-f]*' AND instr(sha256,char(0))=0),
 role TEXT NOT NULL CHECK(role IN('engine','dictionary','language_package','tokenizer','sentence_boundary','runtime','worker')),
 PRIMARY KEY(snapshot_id,relative_key),
 FOREIGN KEY(snapshot_id) REFERENCES offline_resource_snapshots(snapshot_id) ON DELETE RESTRICT,
 CHECK(substr(relative_key,1,1)<>'/' AND instr(relative_key,'\')=0 AND instr(relative_key,':')=0 AND relative_key<>'.' AND relative_key<>'..' AND instr('/'||relative_key||'/','/../')=0 AND instr('/'||relative_key||'/','/./')=0)
) STRICT;

CREATE TABLE offline_resource_pairs (
 snapshot_id TEXT NOT NULL CHECK(length(snapshot_id)>0 AND instr(snapshot_id,char(0))=0),
 source_language TEXT NOT NULL CHECK(length(CAST(source_language AS BLOB)) BETWEEN 1 AND 32 AND instr(source_language,char(0))=0),
 target_language TEXT NOT NULL CHECK(length(CAST(target_language AS BLOB)) BETWEEN 1 AND 32 AND instr(target_language,char(0))=0),
 pipeline_profile_id TEXT NOT NULL CHECK(length(CAST(pipeline_profile_id AS BLOB)) BETWEEN 1 AND 160 AND instr(pipeline_profile_id,char(0))=0),
 PRIMARY KEY(snapshot_id,source_language,target_language,pipeline_profile_id),
 FOREIGN KEY(snapshot_id) REFERENCES offline_resource_snapshots(snapshot_id) ON DELETE RESTRICT,
 CHECK(source_language<>target_language)
) STRICT;

CREATE TABLE offline_resource_seals (
 snapshot_id TEXT NOT NULL CHECK(length(snapshot_id)>0 AND instr(snapshot_id,char(0))=0),
 content_hash TEXT NOT NULL CHECK(length(CAST(content_hash AS BLOB))=64 AND content_hash NOT GLOB '*[^0-9a-f]*' AND instr(content_hash,char(0))=0),
 sealed_at_ms INTEGER NOT NULL CHECK(sealed_at_ms>=0),
 PRIMARY KEY(snapshot_id),
 FOREIGN KEY(snapshot_id) REFERENCES offline_resource_snapshots(snapshot_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE offline_translation_routes (
 route_id TEXT NOT NULL CHECK(length(route_id)>0 AND instr(route_id,char(0))=0),
 engine_id TEXT NOT NULL CHECK(length(engine_id)>0 AND instr(engine_id,char(0))=0),
 engine_contract_revision INTEGER NOT NULL CHECK(engine_contract_revision BETWEEN 1 AND 4294967295),
 backend TEXT NOT NULL CHECK(backend IN('platform_native','local_worker')),
 route_kind TEXT NOT NULL CHECK(route_kind IN('direct','explicit_pivot')),
 source_language TEXT NOT NULL CHECK(length(CAST(source_language AS BLOB)) BETWEEN 1 AND 32 AND instr(source_language,char(0))=0),
 target_language TEXT NOT NULL CHECK(length(CAST(target_language AS BLOB)) BETWEEN 1 AND 32 AND instr(target_language,char(0))=0),
 consent_revision INTEGER CHECK(consent_revision IS NULL OR (consent_revision BETWEEN 1 AND 4294967295)),
 content_hash TEXT NOT NULL CHECK(length(CAST(content_hash AS BLOB))=64 AND content_hash NOT GLOB '*[^0-9a-f]*' AND instr(content_hash,char(0))=0),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 PRIMARY KEY(route_id),
 UNIQUE(route_id,engine_id,engine_contract_revision,backend),
 CHECK((route_kind='direct' AND consent_revision IS NULL) OR (route_kind='explicit_pivot' AND consent_revision IS NOT NULL)),
 CHECK(source_language<>target_language)
) STRICT;

CREATE TABLE offline_translation_route_hops (
 route_id TEXT NOT NULL CHECK(length(route_id)>0 AND instr(route_id,char(0))=0),
 hop_ordinal INTEGER NOT NULL CHECK(hop_ordinal BETWEEN 0 AND 2),
 snapshot_id TEXT NOT NULL CHECK(length(snapshot_id)>0 AND instr(snapshot_id,char(0))=0),
 engine_id TEXT NOT NULL CHECK(length(engine_id)>0 AND instr(engine_id,char(0))=0),
 engine_contract_revision INTEGER NOT NULL CHECK(engine_contract_revision BETWEEN 1 AND 4294967295),
 backend TEXT NOT NULL CHECK(backend IN('platform_native','local_worker')),
 source_language TEXT NOT NULL CHECK(length(CAST(source_language AS BLOB)) BETWEEN 1 AND 32 AND instr(source_language,char(0))=0),
 target_language TEXT NOT NULL CHECK(length(CAST(target_language AS BLOB)) BETWEEN 1 AND 32 AND instr(target_language,char(0))=0),
 pipeline_profile_id TEXT NOT NULL CHECK(length(CAST(pipeline_profile_id AS BLOB)) BETWEEN 1 AND 160 AND instr(pipeline_profile_id,char(0))=0),
 PRIMARY KEY(route_id,hop_ordinal),
 FOREIGN KEY(route_id,engine_id,engine_contract_revision,backend) REFERENCES offline_translation_routes(route_id,engine_id,engine_contract_revision,backend) ON DELETE RESTRICT,
 FOREIGN KEY(snapshot_id,engine_id,engine_contract_revision,backend) REFERENCES offline_resource_snapshots(snapshot_id,engine_id,engine_contract_revision,backend) ON DELETE RESTRICT,
 FOREIGN KEY(snapshot_id,source_language,target_language,pipeline_profile_id) REFERENCES offline_resource_pairs(snapshot_id,source_language,target_language,pipeline_profile_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE offline_route_seals (
 route_id TEXT NOT NULL CHECK(length(route_id)>0 AND instr(route_id,char(0))=0),
 content_hash TEXT NOT NULL CHECK(length(CAST(content_hash AS BLOB))=64 AND content_hash NOT GLOB '*[^0-9a-f]*' AND instr(content_hash,char(0))=0),
 sealed_at_ms INTEGER NOT NULL CHECK(sealed_at_ms>=0),
 PRIMARY KEY(route_id),
 FOREIGN KEY(route_id) REFERENCES offline_translation_routes(route_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE offline_resource_observations (
 observation_id TEXT NOT NULL CHECK(length(observation_id)>0 AND instr(observation_id,char(0))=0),
 subject_key TEXT NOT NULL CHECK(length(CAST(subject_key AS BLOB)) BETWEEN 1 AND 512 AND instr(subject_key,char(0))=0),
 observation_epoch INTEGER NOT NULL CHECK(observation_epoch BETWEEN 1 AND 4294967295),
 installation_id TEXT CHECK(installation_id IS NULL OR (length(installation_id)>0 AND instr(installation_id,char(0))=0)),
 installation_revision INTEGER CHECK(installation_revision IS NULL OR (installation_revision BETWEEN 1 AND 4294967295)),
 snapshot_id TEXT CHECK(snapshot_id IS NULL OR (length(snapshot_id)>0 AND instr(snapshot_id,char(0))=0)),
 local_grant_id TEXT CHECK(local_grant_id IS NULL OR (length(local_grant_id)>0 AND instr(local_grant_id,char(0))=0)),
 local_grant_epoch INTEGER CHECK(local_grant_epoch IS NULL OR (local_grant_epoch BETWEEN 1 AND 4294967295)),
 state TEXT NOT NULL CHECK(state IN('unknown','installed','missing','changed','permission_denied','unsupported','error')),
 reason_code TEXT CHECK(reason_code IS NULL OR (length(CAST(reason_code AS BLOB)) BETWEEN 1 AND 160 AND instr(reason_code,char(0))=0)),
 observed_at_ms INTEGER NOT NULL CHECK(observed_at_ms>=0),
 PRIMARY KEY(observation_id),
 CHECK((local_grant_id IS NULL)=(local_grant_epoch IS NULL)),
 FOREIGN KEY(local_grant_id) REFERENCES offline_local_grants(grant_id) ON DELETE RESTRICT,
 UNIQUE(subject_key,observation_epoch,observation_id),
 UNIQUE(observation_id,snapshot_id,observation_epoch),
 CHECK((installation_id IS NULL)=(installation_revision IS NULL)),
 FOREIGN KEY(installation_id,installation_revision) REFERENCES offline_installation_versions(installation_id,installation_revision) ON DELETE RESTRICT,
 FOREIGN KEY(snapshot_id) REFERENCES offline_resource_snapshots(snapshot_id) ON DELETE RESTRICT,
 CHECK(state<>'installed' OR snapshot_id IS NOT NULL)
) STRICT;

CREATE TABLE offline_observation_heads (
 subject_key TEXT NOT NULL CHECK(length(CAST(subject_key AS BLOB)) BETWEEN 1 AND 512 AND instr(subject_key,char(0))=0),
 observation_epoch INTEGER NOT NULL CHECK(observation_epoch BETWEEN 1 AND 4294967295),
 observation_id TEXT NOT NULL CHECK(length(observation_id)>0 AND instr(observation_id,char(0))=0),
 row_revision INTEGER NOT NULL CHECK(row_revision BETWEEN 1 AND 4294967295),
 PRIMARY KEY(subject_key),
 FOREIGN KEY(subject_key,observation_epoch,observation_id) REFERENCES offline_resource_observations(subject_key,observation_epoch,observation_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE offline_engine_configs (
 revision_id TEXT NOT NULL CHECK(length(revision_id)>0 AND instr(revision_id,char(0))=0),
 engine_config_id TEXT NOT NULL CHECK(length(engine_config_id)>0 AND instr(engine_config_id,char(0))=0),
 engine_id TEXT NOT NULL CHECK(length(engine_id)>0 AND instr(engine_id,char(0))=0),
 engine_contract_revision INTEGER NOT NULL CHECK(engine_contract_revision BETWEEN 1 AND 4294967295),
 backend TEXT NOT NULL CHECK(backend IN('platform_native','local_worker')),
 display_name TEXT NOT NULL CHECK(length(CAST(display_name AS BLOB)) BETWEEN 1 AND 512 AND instr(display_name,char(0))=0),
 source_language TEXT NOT NULL CHECK(length(CAST(source_language AS BLOB)) BETWEEN 1 AND 32 AND instr(source_language,char(0))=0),
 target_language TEXT NOT NULL CHECK(length(CAST(target_language AS BLOB)) BETWEEN 1 AND 32 AND instr(target_language,char(0))=0),
 route_policy TEXT NOT NULL CHECK(route_policy IN('direct_only','explicit_pivot')),
 consent_revision INTEGER CHECK(consent_revision IS NULL OR (consent_revision BETWEEN 1 AND 4294967295)),
 installation_id TEXT CHECK(installation_id IS NULL OR (length(installation_id)>0 AND instr(installation_id,char(0))=0)),
 installation_revision INTEGER CHECK(installation_revision IS NULL OR (installation_revision BETWEEN 1 AND 4294967295)),
 resolution TEXT NOT NULL CHECK(resolution IN('draft','resolved')),
 snapshot_id TEXT CHECK(snapshot_id IS NULL OR (length(snapshot_id)>0 AND instr(snapshot_id,char(0))=0)),
 route_id TEXT CHECK(route_id IS NULL OR (length(route_id)>0 AND instr(route_id,char(0))=0)),
 PRIMARY KEY(revision_id,engine_config_id),
 UNIQUE(revision_id,engine_config_id,snapshot_id,route_id),
 FOREIGN KEY(revision_id) REFERENCES config_revisions(revision_id) ON DELETE RESTRICT,
 FOREIGN KEY(installation_id,installation_revision,engine_id,engine_contract_revision) REFERENCES offline_installation_versions(installation_id,installation_revision,engine_id,engine_contract_revision) ON DELETE RESTRICT,
 FOREIGN KEY(snapshot_id,engine_id,engine_contract_revision,backend) REFERENCES offline_resource_snapshots(snapshot_id,engine_id,engine_contract_revision,backend) ON DELETE RESTRICT,
 FOREIGN KEY(route_id,engine_id,engine_contract_revision,backend) REFERENCES offline_translation_routes(route_id,engine_id,engine_contract_revision,backend) ON DELETE RESTRICT,
 CHECK((installation_id IS NULL)=(installation_revision IS NULL)),
 CHECK((route_policy='direct_only' AND consent_revision IS NULL) OR (route_policy='explicit_pivot' AND consent_revision IS NOT NULL)),
 CHECK(resolution<>'resolved' OR (snapshot_id IS NOT NULL AND route_id IS NOT NULL AND ((backend='platform_native' AND installation_id IS NULL) OR (backend='local_worker' AND installation_id IS NOT NULL))))
) STRICT;

CREATE TABLE offline_config_runtime_states (
 engine_config_id TEXT NOT NULL CHECK(length(engine_config_id)>0 AND instr(engine_config_id,char(0))=0),
 control_revision INTEGER NOT NULL CHECK(control_revision BETWEEN 1 AND 4294967295),
 is_suspended INTEGER NOT NULL CHECK(is_suspended IN(0,1)),
 reason_code TEXT CHECK(reason_code IS NULL OR (length(CAST(reason_code AS BLOB)) BETWEEN 1 AND 160 AND instr(reason_code,char(0))=0)),
 updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms>=0),
 PRIMARY KEY(engine_config_id)
) STRICT;

CREATE TABLE binding_engines_v4 (
    revision_id TEXT NOT NULL, -- 所属不可变整体配置版本
    binding_id TEXT NOT NULL, -- 同版本用途绑定ID
    engine_id TEXT NOT NULL CHECK (length(engine_id) > 0), -- 用途内跨版本稳定的引擎项ID
    purpose TEXT NOT NULL CHECK (purpose IN ('translation','grammar','context_word')), -- 冗余用途并以复合外键保证与父用途一致
    model_id TEXT, -- 同版本模型配置ID
    position INTEGER NOT NULL CHECK (position >= 0), -- 结果显示和调度顺序，按用途唯一
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)), -- 该分配项启用意图；一个翻译引擎失败不关闭其他项
    template_id TEXT, -- 引擎级用途模板；为空继续继承
    target_kind TEXT NOT NULL DEFAULT 'http' CHECK(target_kind IN('http','offline')),
    offline_engine_config_id TEXT,
    PRIMARY KEY (revision_id, binding_id, engine_id),
    UNIQUE (revision_id, binding_id, position),
    UNIQUE (revision_id, binding_id, engine_id, purpose),
    FOREIGN KEY (revision_id, binding_id, purpose) REFERENCES purpose_bindings(revision_id, binding_id, purpose) ON DELETE RESTRICT,
    FOREIGN KEY (revision_id, model_id) REFERENCES models(revision_id, model_id) ON DELETE RESTRICT,
    FOREIGN KEY (revision_id, template_id, purpose) REFERENCES templates(revision_id, template_id, purpose) ON DELETE RESTRICT
,
    UNIQUE(revision_id,binding_id,engine_id,target_kind),
    CHECK((target_kind='http' AND model_id IS NOT NULL AND offline_engine_config_id IS NULL) OR (target_kind='offline' AND purpose='translation' AND model_id IS NULL AND offline_engine_config_id IS NOT NULL AND template_id IS NULL)),
    FOREIGN KEY(revision_id,offline_engine_config_id) REFERENCES offline_engine_configs(revision_id,engine_config_id) ON DELETE RESTRICT
) STRICT;

INSERT INTO binding_engines_v4 ("revision_id","binding_id","engine_id","purpose","model_id","position","enabled","template_id") SELECT "revision_id","binding_id","engine_id","purpose","model_id","position","enabled","template_id" FROM "binding_engines";

DROP TABLE "binding_engines";

ALTER TABLE binding_engines_v4 RENAME TO "binding_engines";

CREATE TABLE service_request_defaults (
 revision_id TEXT NOT NULL CHECK(length(revision_id)>0 AND instr(revision_id,char(0))=0),
 defaults_id TEXT NOT NULL CHECK(length(defaults_id)>0 AND instr(defaults_id,char(0))=0),
 target_kind TEXT NOT NULL CHECK(target_kind IN('http','offline')),
 model_id TEXT CHECK(model_id IS NULL OR (length(model_id)>0 AND instr(model_id,char(0))=0)),
 offline_engine_config_id TEXT CHECK(offline_engine_config_id IS NULL OR (length(offline_engine_config_id)>0 AND instr(offline_engine_config_id,char(0))=0)),
 purpose TEXT NOT NULL CHECK(purpose IN('translation','grammar','context_word')),
 source_mode TEXT NOT NULL CHECK(source_mode IN('inherit_source','explicit','config_pair')),
 source_language TEXT CHECK(source_language IS NULL OR (length(CAST(source_language AS BLOB)) BETWEEN 1 AND 32 AND instr(source_language,char(0))=0)),
 target_mode TEXT NOT NULL CHECK(target_mode IN('inherit_preference','explicit','config_pair')),
 target_language TEXT CHECK(target_language IS NULL OR (length(CAST(target_language AS BLOB)) BETWEEN 1 AND 32 AND instr(target_language,char(0))=0)),
 context_enabled INTEGER NOT NULL CHECK(context_enabled IN(0,1)),
 context_sentences INTEGER NOT NULL CHECK(context_sentences BETWEEN 0 AND 50),
 context_mode TEXT NOT NULL CHECK(context_mode IN('original','translation','both')),
 timeout_ms INTEGER NOT NULL CHECK(timeout_ms BETWEEN 1000 AND 300000),
 concurrency INTEGER NOT NULL CHECK(concurrency BETWEEN 1 AND 8),
 retries INTEGER NOT NULL CHECK(retries BETWEEN 0 AND 3),
 cache_enabled INTEGER NOT NULL CHECK(cache_enabled IN(0,1)),
 PRIMARY KEY(revision_id,defaults_id),
 UNIQUE(revision_id,defaults_id,purpose),
 FOREIGN KEY(revision_id,model_id) REFERENCES models(revision_id,model_id) ON DELETE RESTRICT,
 FOREIGN KEY(revision_id,offline_engine_config_id) REFERENCES offline_engine_configs(revision_id,engine_config_id) ON DELETE RESTRICT,
 CHECK((target_kind='http' AND model_id IS NOT NULL AND offline_engine_config_id IS NULL AND source_mode<>'config_pair' AND target_mode<>'config_pair') OR (target_kind='offline' AND purpose='translation' AND model_id IS NULL AND offline_engine_config_id IS NOT NULL AND source_mode='config_pair' AND target_mode='config_pair' AND source_language IS NULL AND target_language IS NULL AND context_enabled=0 AND context_sentences=0)),
 CHECK((source_mode='explicit' AND source_language IS NOT NULL) OR (source_mode<>'explicit' AND source_language IS NULL)),
 CHECK((target_mode='explicit' AND target_language IS NOT NULL) OR (target_mode<>'explicit' AND target_language IS NULL))
) STRICT;

CREATE UNIQUE INDEX service_defaults_http ON service_request_defaults(revision_id,model_id,purpose) WHERE target_kind='http';

CREATE UNIQUE INDEX service_defaults_offline ON service_request_defaults(revision_id,offline_engine_config_id) WHERE target_kind='offline';

CREATE TABLE service_default_fallbacks (
 revision_id TEXT NOT NULL CHECK(length(revision_id)>0 AND instr(revision_id,char(0))=0),
 defaults_id TEXT NOT NULL CHECK(length(defaults_id)>0 AND instr(defaults_id,char(0))=0),
 position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 127),
 target_kind TEXT NOT NULL CHECK(target_kind IN('http','offline')),
 source_revision_id TEXT NOT NULL CHECK(length(source_revision_id)>0 AND instr(source_revision_id,char(0))=0),
 source_model_id TEXT CHECK(source_model_id IS NULL OR (length(source_model_id)>0 AND instr(source_model_id,char(0))=0)),
 source_offline_config_id TEXT CHECK(source_offline_config_id IS NULL OR (length(source_offline_config_id)>0 AND instr(source_offline_config_id,char(0))=0)),
 PRIMARY KEY(revision_id,defaults_id,position),
 FOREIGN KEY(revision_id,defaults_id) REFERENCES service_request_defaults(revision_id,defaults_id) ON DELETE RESTRICT,
 FOREIGN KEY(source_revision_id,source_model_id) REFERENCES models(revision_id,model_id) ON DELETE RESTRICT,
 FOREIGN KEY(source_revision_id,source_offline_config_id) REFERENCES offline_engine_configs(revision_id,engine_config_id) ON DELETE RESTRICT,
 CHECK((target_kind='http' AND source_model_id IS NOT NULL AND source_offline_config_id IS NULL) OR (target_kind='offline' AND source_model_id IS NULL AND source_offline_config_id IS NOT NULL))
) STRICT;

CREATE TABLE activation_resource_materializations (
 materialization_id TEXT NOT NULL CHECK(length(materialization_id)>0 AND instr(materialization_id,char(0))=0),
 effective_revision_id TEXT NOT NULL CHECK(length(effective_revision_id)>0 AND instr(effective_revision_id,char(0))=0),
 target_kind TEXT NOT NULL CHECK(target_kind IN('http','offline','connection')),
 effective_model_id TEXT CHECK(effective_model_id IS NULL OR (length(effective_model_id)>0 AND instr(effective_model_id,char(0))=0)),
 effective_offline_config_id TEXT CHECK(effective_offline_config_id IS NULL OR (length(effective_offline_config_id)>0 AND instr(effective_offline_config_id,char(0))=0)),
 source_revision_id TEXT NOT NULL CHECK(length(source_revision_id)>0 AND instr(source_revision_id,char(0))=0),
 source_model_id TEXT CHECK(source_model_id IS NULL OR (length(source_model_id)>0 AND instr(source_model_id,char(0))=0)),
 source_offline_config_id TEXT CHECK(source_offline_config_id IS NULL OR (length(source_offline_config_id)>0 AND instr(source_offline_config_id,char(0))=0)),
 reason TEXT NOT NULL CHECK(reason IN('selected_closure','activation_collision','legacy_materialization')),
 effective_connection_id TEXT, source_connection_id TEXT, control_connection_id TEXT, control_config_id TEXT,
 PRIMARY KEY(materialization_id),
 FOREIGN KEY(effective_revision_id,effective_model_id,effective_connection_id) REFERENCES models(revision_id,model_id,connection_id) ON DELETE RESTRICT,
 FOREIGN KEY(source_revision_id,source_model_id,source_connection_id) REFERENCES models(revision_id,model_id,connection_id) ON DELETE RESTRICT,
 FOREIGN KEY(effective_revision_id,effective_connection_id) REFERENCES connections(revision_id,connection_id) ON DELETE RESTRICT,
 FOREIGN KEY(source_revision_id,source_connection_id) REFERENCES connections(revision_id,connection_id) ON DELETE RESTRICT,
 FOREIGN KEY(control_connection_id) REFERENCES connection_runtime_states(connection_id) ON DELETE RESTRICT,
 FOREIGN KEY(control_config_id) REFERENCES offline_config_runtime_states(engine_config_id) ON DELETE RESTRICT,
 CHECK((target_kind IN('http','connection') AND effective_connection_id IS NOT NULL AND source_connection_id IS NOT NULL AND control_connection_id IS NOT NULL AND control_config_id IS NULL) OR (target_kind='offline' AND effective_connection_id IS NULL AND source_connection_id IS NULL AND control_connection_id IS NULL AND control_config_id IS NOT NULL)),
 FOREIGN KEY(effective_revision_id,effective_model_id) REFERENCES models(revision_id,model_id) ON DELETE RESTRICT,
 FOREIGN KEY(effective_revision_id,effective_offline_config_id) REFERENCES offline_engine_configs(revision_id,engine_config_id) ON DELETE RESTRICT,
 FOREIGN KEY(source_revision_id,source_model_id) REFERENCES models(revision_id,model_id) ON DELETE RESTRICT,
 FOREIGN KEY(source_revision_id,source_offline_config_id) REFERENCES offline_engine_configs(revision_id,engine_config_id) ON DELETE RESTRICT,
 CHECK((target_kind='http' AND effective_model_id IS NOT NULL AND source_model_id IS NOT NULL AND effective_offline_config_id IS NULL AND source_offline_config_id IS NULL) OR (target_kind='offline' AND effective_model_id IS NULL AND source_model_id IS NULL AND effective_offline_config_id IS NOT NULL AND source_offline_config_id IS NOT NULL) OR (target_kind='connection' AND effective_model_id IS NULL AND source_model_id IS NULL AND effective_offline_config_id IS NULL AND source_offline_config_id IS NULL AND reason='legacy_materialization'))
) STRICT;

CREATE UNIQUE INDEX activation_http_alias ON activation_resource_materializations(effective_revision_id,effective_model_id) WHERE target_kind='http';

CREATE UNIQUE INDEX activation_offline_alias ON activation_resource_materializations(effective_revision_id,effective_offline_config_id) WHERE target_kind='offline';

CREATE UNIQUE INDEX activation_connection_alias ON activation_resource_materializations(effective_revision_id,effective_connection_id) WHERE target_kind='connection';

CREATE TABLE task_execution_targets (
 task_id TEXT NOT NULL CHECK(length(task_id)>0 AND instr(task_id,char(0))=0),
 config_revision_id TEXT NOT NULL CHECK(length(config_revision_id)>0 AND instr(config_revision_id,char(0))=0),
 binding_id TEXT NOT NULL CHECK(length(binding_id)>0 AND instr(binding_id,char(0))=0),
 engine_id TEXT NOT NULL CHECK(length(engine_id)>0 AND instr(engine_id,char(0))=0),
 purpose TEXT NOT NULL CHECK(purpose IN('translation','grammar','context_word')),
 target_kind TEXT NOT NULL CHECK(target_kind IN('http','offline')),
 http_model_id TEXT CHECK(http_model_id IS NULL OR (length(http_model_id)>0 AND instr(http_model_id,char(0))=0)),
 http_connection_id TEXT CHECK(http_connection_id IS NULL OR (length(http_connection_id)>0 AND instr(http_connection_id,char(0))=0)),
 offline_engine_config_id TEXT CHECK(offline_engine_config_id IS NULL OR (length(offline_engine_config_id)>0 AND instr(offline_engine_config_id,char(0))=0)),
 snapshot_id TEXT CHECK(snapshot_id IS NULL OR (length(snapshot_id)>0 AND instr(snapshot_id,char(0))=0)),
 route_id TEXT CHECK(route_id IS NULL OR (length(route_id)>0 AND instr(route_id,char(0))=0)),
 observation_id TEXT CHECK(observation_id IS NULL OR (length(observation_id)>0 AND instr(observation_id,char(0))=0)),
 observation_epoch INTEGER CHECK(observation_epoch IS NULL OR (observation_epoch BETWEEN 1 AND 4294967295)),
 selection_scope TEXT CHECK(selection_scope IS NULL OR (length(CAST(selection_scope AS BLOB)) BETWEEN 1 AND 512 AND instr(selection_scope,char(0))=0)),
 selection_revision INTEGER CHECK(selection_revision IS NULL OR (selection_revision BETWEEN 1 AND 4294967295)),
 effective_scope TEXT CHECK(effective_scope IS NULL OR (length(CAST(effective_scope AS BLOB)) BETWEEN 1 AND 512 AND instr(effective_scope,char(0))=0)),
 effective_selection_revision INTEGER CHECK(effective_selection_revision IS NULL OR (effective_selection_revision BETWEEN 1 AND 4294967295)),
 target_sha256 TEXT NOT NULL CHECK(length(CAST(target_sha256 AS BLOB))=64 AND target_sha256 NOT GLOB '*[^0-9a-f]*' AND instr(target_sha256,char(0))=0),
 PRIMARY KEY(task_id),
 UNIQUE(task_id,target_kind),
 UNIQUE(task_id,config_revision_id),
 FOREIGN KEY(task_id,config_revision_id,binding_id,purpose) REFERENCES request_tasks(task_id,config_revision_id,binding_id,purpose) ON DELETE CASCADE,
 FOREIGN KEY(config_revision_id,binding_id,engine_id,target_kind) REFERENCES binding_engines(revision_id,binding_id,engine_id,target_kind) ON DELETE RESTRICT,
 FOREIGN KEY(config_revision_id,http_model_id,http_connection_id) REFERENCES models(revision_id,model_id,connection_id) ON DELETE RESTRICT,
 FOREIGN KEY(config_revision_id,offline_engine_config_id,snapshot_id,route_id) REFERENCES offline_engine_configs(revision_id,engine_config_id,snapshot_id,route_id) ON DELETE RESTRICT,
 FOREIGN KEY(observation_id,snapshot_id,observation_epoch) REFERENCES offline_resource_observations(observation_id,snapshot_id,observation_epoch) ON DELETE RESTRICT,
 FOREIGN KEY(config_revision_id,selection_scope,purpose,selection_revision) REFERENCES purpose_bindings(revision_id,scope_key,purpose,selection_revision) ON DELETE RESTRICT,
 FOREIGN KEY(config_revision_id,effective_scope,purpose,effective_selection_revision) REFERENCES purpose_bindings(revision_id,scope_key,purpose,selection_revision) ON DELETE RESTRICT,
 CHECK((selection_scope IS NULL AND selection_revision IS NULL AND effective_scope IS NULL AND effective_selection_revision IS NULL) OR (selection_scope IS NOT NULL AND selection_revision IS NOT NULL AND effective_scope IS NOT NULL AND effective_selection_revision IS NOT NULL)),
 CHECK((target_kind='http' AND http_model_id IS NOT NULL AND http_connection_id IS NOT NULL AND offline_engine_config_id IS NULL AND snapshot_id IS NULL AND route_id IS NULL AND observation_id IS NULL AND observation_epoch IS NULL) OR (target_kind='offline' AND purpose='translation' AND http_model_id IS NULL AND http_connection_id IS NULL AND offline_engine_config_id IS NOT NULL AND snapshot_id IS NOT NULL AND route_id IS NOT NULL AND observation_id IS NOT NULL AND observation_epoch IS NOT NULL))
) STRICT;

CREATE TABLE test_records_v4 (
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
    target_kind TEXT NOT NULL DEFAULT 'http' CHECK(target_kind IN('http','offline')),
    FOREIGN KEY (config_revision_id, connection_id) REFERENCES connections(revision_id, connection_id) ON DELETE RESTRICT,
    FOREIGN KEY (config_revision_id, model_id, connection_id) REFERENCES models(revision_id, model_id, connection_id) ON DELETE RESTRICT,
    FOREIGN KEY (config_revision_id, binding_id, purpose) REFERENCES purpose_bindings(revision_id, binding_id, purpose) ON DELETE RESTRICT,
    FOREIGN KEY (unit_id, text_revision) REFERENCES reading_unit_revisions(unit_id, text_revision) ON DELETE SET NULL,
    CHECK ((unit_id IS NULL) = (text_revision IS NULL)),
    CHECK (target_kind='offline' OR stage = 'local' OR connection_id IS NOT NULL),
    CHECK (target_kind='offline' OR stage NOT IN ('model','purpose') OR model_id IS NOT NULL),
    CHECK (stage <> 'purpose' OR (binding_id IS NOT NULL AND purpose IS NOT NULL)),
    CHECK ((binding_id IS NULL) = (purpose IS NULL)),
    CHECK (model_id IS NULL OR connection_id IS NOT NULL),
    CHECK ((state = 'passed' AND error_code IS NULL) OR state <> 'passed')
) STRICT;

INSERT INTO test_records_v4 ("test_id","config_revision_id","connection_id","model_id","binding_id","purpose","stage","request_semantics_hash","input_scope","unit_id","text_revision","input_summary_json","response_summary_json","output_validation_json","state","started_at_ms","completed_at_ms","error_code","error_stage") SELECT "test_id","config_revision_id","connection_id","model_id","binding_id","purpose","stage","request_semantics_hash","input_scope","unit_id","text_revision","input_summary_json","response_summary_json","output_validation_json","state","started_at_ms","completed_at_ms","error_code","error_stage" FROM "test_records";

DROP TABLE "test_records";

ALTER TABLE test_records_v4 RENAME TO "test_records";

CREATE TABLE offline_test_targets (
 test_id TEXT NOT NULL CHECK(length(test_id)>0 AND instr(test_id,char(0))=0),
 config_revision_id TEXT NOT NULL CHECK(length(config_revision_id)>0 AND instr(config_revision_id,char(0))=0),
 engine_config_id TEXT NOT NULL CHECK(length(engine_config_id)>0 AND instr(engine_config_id,char(0))=0),
 phase TEXT NOT NULL CHECK(phase IN('probe','translation')),
 snapshot_id TEXT CHECK(snapshot_id IS NULL OR (length(snapshot_id)>0 AND instr(snapshot_id,char(0))=0)),
 route_id TEXT CHECK(route_id IS NULL OR (length(route_id)>0 AND instr(route_id,char(0))=0)),
 observation_id TEXT CHECK(observation_id IS NULL OR (length(observation_id)>0 AND instr(observation_id,char(0))=0)),
 observation_epoch INTEGER CHECK(observation_epoch IS NULL OR (observation_epoch BETWEEN 1 AND 4294967295)),
 runtime_build_sha256 TEXT CHECK(runtime_build_sha256 IS NULL OR (length(CAST(runtime_build_sha256 AS BLOB))=64 AND runtime_build_sha256 NOT GLOB '*[^0-9a-f]*' AND instr(runtime_build_sha256,char(0))=0)),
 PRIMARY KEY(test_id),
 FOREIGN KEY(test_id) REFERENCES test_records(test_id) ON DELETE CASCADE,
 FOREIGN KEY(config_revision_id,engine_config_id) REFERENCES offline_engine_configs(revision_id,engine_config_id) ON DELETE RESTRICT,
 FOREIGN KEY(snapshot_id) REFERENCES offline_resource_snapshots(snapshot_id) ON DELETE RESTRICT,
 FOREIGN KEY(route_id) REFERENCES offline_translation_routes(route_id) ON DELETE RESTRICT,
 FOREIGN KEY(observation_id,snapshot_id,observation_epoch) REFERENCES offline_resource_observations(observation_id,snapshot_id,observation_epoch) ON DELETE RESTRICT,
 CHECK(phase='probe' OR (snapshot_id IS NOT NULL AND route_id IS NOT NULL AND observation_id IS NOT NULL AND observation_epoch IS NOT NULL AND runtime_build_sha256 IS NOT NULL)),
 CHECK((observation_id IS NULL)=(observation_epoch IS NULL))
) STRICT;

CREATE TABLE task_allowed_fallback_targets (
 task_id TEXT NOT NULL CHECK(length(task_id)>0 AND instr(task_id,char(0))=0),
 fallback_ordinal INTEGER NOT NULL CHECK(fallback_ordinal BETWEEN 0 AND 127),
 config_revision_id TEXT NOT NULL CHECK(length(config_revision_id)>0 AND instr(config_revision_id,char(0))=0),
 binding_id TEXT NOT NULL CHECK(length(binding_id)>0 AND instr(binding_id,char(0))=0),
 engine_id TEXT NOT NULL CHECK(length(engine_id)>0 AND instr(engine_id,char(0))=0),
 target_kind TEXT NOT NULL CHECK(target_kind IN('http','offline')),
 PRIMARY KEY(task_id,fallback_ordinal),
 UNIQUE(task_id,config_revision_id,binding_id,engine_id,target_kind),
 FOREIGN KEY(task_id,config_revision_id) REFERENCES task_execution_targets(task_id,config_revision_id) ON DELETE CASCADE,
 FOREIGN KEY(config_revision_id,binding_id,engine_id,target_kind) REFERENCES binding_engines(revision_id,binding_id,engine_id,target_kind) ON DELETE RESTRICT
) STRICT;

CREATE TABLE attempt_execution_targets (
 attempt_id TEXT NOT NULL CHECK(length(attempt_id)>0 AND instr(attempt_id,char(0))=0),
 task_id TEXT NOT NULL CHECK(length(task_id)>0 AND instr(task_id,char(0))=0),
 config_revision_id TEXT NOT NULL CHECK(length(config_revision_id)>0 AND instr(config_revision_id,char(0))=0),
 binding_id TEXT NOT NULL CHECK(length(binding_id)>0 AND instr(binding_id,char(0))=0),
 engine_id TEXT NOT NULL CHECK(length(engine_id)>0 AND instr(engine_id,char(0))=0),
 target_kind TEXT NOT NULL CHECK(target_kind IN('http','offline')),
 http_model_id TEXT CHECK(http_model_id IS NULL OR (length(http_model_id)>0 AND instr(http_model_id,char(0))=0)),
 http_connection_id TEXT CHECK(http_connection_id IS NULL OR (length(http_connection_id)>0 AND instr(http_connection_id,char(0))=0)),
 offline_engine_config_id TEXT CHECK(offline_engine_config_id IS NULL OR (length(offline_engine_config_id)>0 AND instr(offline_engine_config_id,char(0))=0)),
 snapshot_id TEXT CHECK(snapshot_id IS NULL OR (length(snapshot_id)>0 AND instr(snapshot_id,char(0))=0)),
 route_id TEXT CHECK(route_id IS NULL OR (length(route_id)>0 AND instr(route_id,char(0))=0)),
 observation_id TEXT CHECK(observation_id IS NULL OR (length(observation_id)>0 AND instr(observation_id,char(0))=0)),
 observation_epoch INTEGER CHECK(observation_epoch IS NULL OR (observation_epoch BETWEEN 1 AND 4294967295)),
 runtime_build_sha256 TEXT, worker_instance_id TEXT, worker_protocol_revision INTEGER CHECK(worker_protocol_revision IS NULL OR worker_protocol_revision BETWEEN 1 AND 4294967295), provenance_sha256 TEXT,
 CHECK((target_kind='http' AND runtime_build_sha256 IS NULL AND worker_instance_id IS NULL AND worker_protocol_revision IS NULL) OR (target_kind='offline' AND runtime_build_sha256 IS NOT NULL AND length(runtime_build_sha256)=64 AND runtime_build_sha256 NOT GLOB '*[^0-9a-f]*' AND provenance_sha256 IS NOT NULL AND length(provenance_sha256)=64 AND provenance_sha256 NOT GLOB '*[^0-9a-f]*' AND ((worker_instance_id IS NULL)=(worker_protocol_revision IS NULL)))),
 PRIMARY KEY(attempt_id),
 UNIQUE(attempt_id,task_id,target_kind),
 FOREIGN KEY(attempt_id,task_id) REFERENCES request_attempts(attempt_id,task_id) ON DELETE CASCADE,
 FOREIGN KEY(task_id,config_revision_id) REFERENCES task_execution_targets(task_id,config_revision_id) ON DELETE RESTRICT,
 FOREIGN KEY(config_revision_id,binding_id,engine_id,target_kind) REFERENCES binding_engines(revision_id,binding_id,engine_id,target_kind) ON DELETE RESTRICT,
 FOREIGN KEY(config_revision_id,http_model_id,http_connection_id) REFERENCES models(revision_id,model_id,connection_id) ON DELETE RESTRICT,
 FOREIGN KEY(config_revision_id,offline_engine_config_id,snapshot_id,route_id) REFERENCES offline_engine_configs(revision_id,engine_config_id,snapshot_id,route_id) ON DELETE RESTRICT,
 FOREIGN KEY(observation_id,snapshot_id,observation_epoch) REFERENCES offline_resource_observations(observation_id,snapshot_id,observation_epoch) ON DELETE RESTRICT,
 CHECK((target_kind='http' AND http_model_id IS NOT NULL AND http_connection_id IS NOT NULL AND offline_engine_config_id IS NULL AND snapshot_id IS NULL AND route_id IS NULL AND observation_id IS NULL AND observation_epoch IS NULL) OR (target_kind='offline' AND http_model_id IS NULL AND http_connection_id IS NULL AND offline_engine_config_id IS NOT NULL AND snapshot_id IS NOT NULL AND route_id IS NOT NULL AND observation_id IS NOT NULL AND observation_epoch IS NOT NULL))
) STRICT;

CREATE UNIQUE INDEX v4_request_attempt_exact_target ON request_attempts(attempt_id,task_id,config_revision_id,binding_id,actual_engine_id);

CREATE UNIQUE INDEX binding_engines_one_study_engine ON binding_engines(revision_id, binding_id) WHERE purpose <> 'translation';

CREATE INDEX binding_engines_by_model ON binding_engines(revision_id, model_id);

CREATE INDEX test_records_by_semantics ON test_records(request_semantics_hash, completed_at_ms DESC, test_id);

CREATE INDEX test_records_by_connection ON test_records(config_revision_id, connection_id, stage, completed_at_ms DESC);

CREATE INDEX test_records_by_model ON test_records(config_revision_id, model_id, stage, completed_at_ms DESC);

CREATE INDEX test_records_by_purpose ON test_records(config_revision_id, binding_id, purpose, completed_at_ms DESC);

CREATE INDEX test_records_by_source ON test_records(unit_id, text_revision);

CREATE INDEX connections_by_credential_pool ON connections(credential_pool_id, credential_pool_revision);

CREATE TRIGGER config_revisions_no_update BEFORE UPDATE ON config_revisions BEGIN SELECT RAISE(ABORT, 'immutable_config_revision'); END;

CREATE TRIGGER binding_engines_no_update BEFORE UPDATE ON binding_engines BEGIN SELECT RAISE(ABORT, 'immutable_engine_snapshot'); END;

CREATE TRIGGER binding_engines_sealed_insert BEFORE INSERT ON binding_engines WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;

CREATE TRIGGER binding_engines_sealed_delete BEFORE DELETE ON binding_engines WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;

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

CREATE TRIGGER pool_gc_unreferenced BEFORE INSERT ON credential_pool_gc_marks
WHEN EXISTS(SELECT 1 FROM connections WHERE credential_pool_id=NEW.pool_id AND credential_pool_revision=NEW.pool_revision)
 OR EXISTS(SELECT 1 FROM test_record_credentials WHERE pool_id=NEW.pool_id AND pool_revision=NEW.pool_revision)
BEGIN SELECT RAISE(ABORT,'pool_gc_still_referenced'); END;

CREATE TRIGGER study_results_connection_authority BEFORE INSERT ON study_results WHEN (SELECT c.codec_version FROM request_tasks t JOIN config_revisions c ON c.revision_id=t.config_revision_id WHERE t.task_id=NEW.task_id)<3 BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM request_tasks t JOIN request_attempts a ON a.task_id=t.task_id AND a.attempt_id=NEW.attempt_id JOIN binding_engines e ON e.revision_id=a.config_revision_id AND e.binding_id=a.binding_id AND e.engine_id=a.actual_engine_id AND e.purpose=a.purpose JOIN purpose_bindings b ON b.revision_id=e.revision_id AND b.binding_id=e.binding_id AND b.purpose=e.purpose JOIN models m ON m.revision_id=e.revision_id AND m.model_id=e.model_id JOIN connections c ON c.revision_id=m.revision_id AND c.connection_id=m.connection_id JOIN connection_runtime_states r ON r.connection_id=c.connection_id WHERE t.task_id=NEW.task_id AND t.purpose IN ('grammar','context_word') AND e.enabled=1 AND b.enabled=1 AND b.binding_mode='override' AND c.availability_intent=1 AND r.is_suspended=0 AND ((c.auth_mode='none' AND a.credential_id IS NULL) OR (c.auth_mode IN ('api_key','baidu_app_secret','youdao_app_secret','google_service_account') AND EXISTS(SELECT 1 FROM credential_pool_members pm JOIN credential_refs cr ON cr.credential_id=pm.credential_id WHERE pm.pool_id=c.credential_pool_id AND pm.pool_revision=c.credential_pool_revision AND pm.credential_id=a.credential_id AND pm.enabled=1 AND cr.state='active' AND cr.auth_kind=c.auth_mode)))) THEN RAISE(ABORT,'study_results_connection_authority') END;
END;

CREATE TRIGGER translations_connection_authority BEFORE INSERT ON translations WHEN (SELECT c.codec_version FROM request_tasks t JOIN config_revisions c ON c.revision_id=t.config_revision_id WHERE t.task_id=NEW.task_id)<3 BEGIN
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

CREATE TRIGGER test_records_immutable_result BEFORE UPDATE ON test_records
WHEN NEW.test_id IS NOT OLD.test_id OR NEW.config_revision_id IS NOT OLD.config_revision_id OR NEW.connection_id IS NOT OLD.connection_id OR NEW.model_id IS NOT OLD.model_id OR NEW.binding_id IS NOT OLD.binding_id OR NEW.purpose IS NOT OLD.purpose OR NEW.stage IS NOT OLD.stage OR NEW.request_semantics_hash IS NOT OLD.request_semantics_hash OR NEW.input_scope IS NOT OLD.input_scope OR NEW.input_summary_json IS NOT OLD.input_summary_json OR NEW.response_summary_json IS NOT OLD.response_summary_json OR NEW.output_validation_json IS NOT OLD.output_validation_json OR NEW.state IS NOT OLD.state OR NEW.started_at_ms IS NOT OLD.started_at_ms OR NEW.completed_at_ms IS NOT OLD.completed_at_ms OR NEW.error_code IS NOT OLD.error_code OR NEW.error_stage IS NOT OLD.error_stage OR ((NEW.unit_id IS NOT OLD.unit_id OR NEW.text_revision IS NOT OLD.text_revision) AND NOT (NEW.unit_id IS NULL AND NEW.text_revision IS NULL))
BEGIN SELECT RAISE(ABORT, 'immutable_test_result'); END;

CREATE TRIGGER config_gc_unreferenced BEFORE INSERT ON config_gc_marks
WHEN EXISTS(SELECT 1 FROM config_heads WHERE revision_id=NEW.revision_id)
 OR EXISTS(SELECT 1 FROM request_tasks WHERE config_revision_id=NEW.revision_id)
 OR EXISTS(SELECT 1 FROM test_records WHERE config_revision_id=NEW.revision_id)
 OR EXISTS(SELECT 1 FROM purpose_runtime_states WHERE config_revision_id=NEW.revision_id)
 OR EXISTS(SELECT 1 FROM connection_runtime_states WHERE config_revision_id=NEW.revision_id)
 OR EXISTS(SELECT 1 FROM activation_resource_materializations a WHERE a.target_kind='connection' AND a.source_revision_id=NEW.revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=a.effective_revision_id))
BEGIN SELECT RAISE(ABORT,'config_gc_still_referenced'); END;

CREATE TRIGGER test_records_requires_seal_insert BEFORE INSERT ON test_records WHEN NOT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=NEW.config_revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id)) BEGIN SELECT RAISE(ABORT,'config_not_sealed_or_purged'); END;

CREATE TRIGGER test_records_requires_seal_update BEFORE UPDATE ON test_records WHEN NOT EXISTS(SELECT 1 FROM config_revision_seals s WHERE s.revision_id=NEW.config_revision_id AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id)) BEGIN SELECT RAISE(ABORT,'config_not_sealed_or_purged'); END;

CREATE TRIGGER v4_http_options_unknown_keys BEFORE INSERT ON connections WHEN NEW.http_options_json IS NOT NULL AND NOT ((NEW.protocol='openai_compatible' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol','organizationId','projectId'))) OR (NEW.protocol='anthropic' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol','workspaceId'))) OR (NEW.protocol='azure_translator' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol','region','resourceType'))) OR (NEW.protocol='openai_responses' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol','organizationId','projectId'))) OR (NEW.protocol='gemini_generate_content' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol'))) OR (NEW.protocol='ollama_native' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol'))) OR (NEW.protocol='lmstudio_native_v1' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol'))) OR (NEW.protocol='deepl_v2' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol','plan'))) OR (NEW.protocol='google_translate_v2' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol'))) OR (NEW.protocol='google_translate_v3' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol','location','projectId'))) OR (NEW.protocol='baidu_vip' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol'))) OR (NEW.protocol='youdao_v3' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol'))) OR (NEW.protocol='tencent_tokenhub_translation' AND NOT EXISTS(SELECT 1 FROM json_each(NEW.http_options_json) WHERE key NOT IN('protocol')))) BEGIN SELECT RAISE(ABORT,'http_options_unknown_fields'); END;

CREATE TRIGGER v4_oauth_protocol BEFORE INSERT ON connections WHEN NEW.auth_mode='google_oauth' AND NEW.protocol<>'google_translate_v3' BEGIN SELECT RAISE(ABORT,'oauth_protocol_mismatch'); END;

CREATE TRIGGER v4_offline_installation_heads_cas BEFORE UPDATE ON offline_installation_heads WHEN NEW.installation_id<>OLD.installation_id OR NEW.row_revision<>OLD.row_revision+1 OR NEW.installation_revision<=OLD.installation_revision BEGIN SELECT RAISE(ABORT,'stale_or_nonmonotonic_head'); END;

CREATE TRIGGER v4_offline_observation_heads_cas BEFORE UPDATE ON offline_observation_heads WHEN NEW.subject_key<>OLD.subject_key OR NEW.row_revision<>OLD.row_revision+1 OR NEW.observation_epoch<=OLD.observation_epoch BEGIN SELECT RAISE(ABORT,'stale_or_nonmonotonic_head'); END;

CREATE TRIGGER v4_local_grant_cas BEFORE UPDATE ON offline_local_grants WHEN NEW.grant_id<>OLD.grant_id OR NEW.machine_fingerprint<>OLD.machine_fingerprint OR NEW.authorization_epoch<>OLD.authorization_epoch+1 BEGIN SELECT RAISE(ABORT,'invalid_local_grant_epoch'); END;

CREATE TRIGGER v4_resource_seal_shape BEFORE INSERT ON offline_resource_seals WHEN NOT EXISTS(SELECT 1 FROM offline_resource_snapshots s WHERE s.snapshot_id=NEW.snapshot_id AND s.content_hash=NEW.content_hash) OR EXISTS(SELECT 1 FROM offline_resource_snapshots s WHERE s.snapshot_id=NEW.snapshot_id AND ((s.identity_kind='file_manifest' AND NOT EXISTS(SELECT 1 FROM offline_resource_snapshot_files f WHERE f.snapshot_id=s.snapshot_id)) OR NOT EXISTS(SELECT 1 FROM offline_resource_pairs p WHERE p.snapshot_id=s.snapshot_id) OR (s.identity_kind='apple_observation' AND (EXISTS(SELECT 1 FROM offline_resource_snapshot_files f WHERE f.snapshot_id=s.snapshot_id) OR (SELECT count(*) FROM offline_resource_pairs p WHERE p.snapshot_id=s.snapshot_id AND p.source_language=s.source_language AND p.target_language=s.target_language)<>1)))) BEGIN SELECT RAISE(ABORT,'invalid_offline_resource_seal'); END;

CREATE TRIGGER v4_route_seal_shape BEFORE INSERT ON offline_route_seals WHEN NOT EXISTS(SELECT 1 FROM offline_translation_routes r WHERE r.route_id=NEW.route_id AND r.content_hash=NEW.content_hash) OR EXISTS(SELECT 1 FROM offline_translation_routes r WHERE r.route_id=NEW.route_id AND ((r.route_kind='direct' AND (SELECT count(*) FROM offline_translation_route_hops h WHERE h.route_id=r.route_id)<>1) OR (r.route_kind='explicit_pivot' AND (SELECT count(*) FROM offline_translation_route_hops h WHERE h.route_id=r.route_id) NOT BETWEEN 2 AND 3) OR NOT EXISTS(SELECT 1 FROM offline_translation_route_hops h WHERE h.route_id=r.route_id AND h.hop_ordinal=0 AND h.source_language=r.source_language) OR NOT EXISTS(SELECT 1 FROM offline_translation_route_hops h WHERE h.route_id=r.route_id AND h.hop_ordinal=(SELECT max(hop_ordinal) FROM offline_translation_route_hops WHERE route_id=r.route_id) AND h.target_language=r.target_language) OR (SELECT max(hop_ordinal)+1 FROM offline_translation_route_hops WHERE route_id=r.route_id)<>(SELECT count(*) FROM offline_translation_route_hops WHERE route_id=r.route_id) OR EXISTS(SELECT 1 FROM offline_translation_route_hops a JOIN offline_translation_route_hops b ON b.route_id=a.route_id AND b.hop_ordinal=a.hop_ordinal+1 WHERE a.route_id=r.route_id AND a.target_language<>b.source_language) OR EXISTS(SELECT 1 FROM offline_translation_route_hops a JOIN offline_translation_route_hops b ON b.route_id=a.route_id AND b.hop_ordinal>a.hop_ordinal WHERE a.route_id=r.route_id AND a.source_language=b.target_language) OR EXISTS(SELECT 1 FROM offline_translation_route_hops h WHERE h.route_id=r.route_id AND NOT EXISTS(SELECT 1 FROM offline_resource_seals s WHERE s.snapshot_id=h.snapshot_id)))) BEGIN SELECT RAISE(ABORT,'invalid_offline_route_seal'); END;

CREATE TRIGGER v4_attempt_exact_target BEFORE INSERT ON attempt_execution_targets WHEN NOT EXISTS(SELECT 1 FROM request_attempts a WHERE a.attempt_id=NEW.attempt_id AND a.task_id=NEW.task_id AND a.config_revision_id=NEW.config_revision_id AND a.binding_id=NEW.binding_id AND a.actual_engine_id=NEW.engine_id AND ((a.dispatch_kind<>'fallback' AND EXISTS(SELECT 1 FROM task_execution_targets t WHERE t.task_id=a.task_id AND t.engine_id=a.actual_engine_id AND t.target_kind=NEW.target_kind)) OR (a.dispatch_kind='fallback' AND EXISTS(SELECT 1 FROM task_allowed_fallback_targets f WHERE f.task_id=a.task_id AND f.config_revision_id=a.config_revision_id AND f.binding_id=a.binding_id AND f.engine_id=a.actual_engine_id AND f.target_kind=NEW.target_kind)))) BEGIN SELECT RAISE(ABORT,'attempt_target_mismatch'); END;

CREATE TRIGGER v4_translations_target_authority BEFORE INSERT ON translations WHEN EXISTS(SELECT 1 FROM request_tasks t JOIN config_revisions c ON c.revision_id=t.config_revision_id WHERE t.task_id=NEW.task_id AND c.codec_version=3) AND NOT EXISTS(SELECT 1 FROM v4_attempt_target_authority a WHERE a.attempt_id=NEW.attempt_id) BEGIN SELECT RAISE(ABORT,'codec3_target_authority'); END;

CREATE TRIGGER v4_study_results_target_authority BEFORE INSERT ON study_results WHEN EXISTS(SELECT 1 FROM request_tasks t JOIN config_revisions c ON c.revision_id=t.config_revision_id WHERE t.task_id=NEW.task_id AND c.codec_version=3) AND NOT EXISTS(SELECT 1 FROM v4_attempt_target_authority a WHERE a.attempt_id=NEW.attempt_id) BEGIN SELECT RAISE(ABORT,'codec3_target_authority'); END;

CREATE TRIGGER v4_offline_engine_configs_sealed_insert BEFORE INSERT ON offline_engine_configs WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;

CREATE TRIGGER v4_offline_engine_configs_no_update BEFORE UPDATE ON offline_engine_configs BEGIN SELECT RAISE(ABORT,'immutable_config_child'); END;

CREATE TRIGGER v4_offline_engine_configs_sealed_delete BEFORE DELETE ON offline_engine_configs WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;

CREATE TRIGGER v4_service_request_defaults_sealed_insert BEFORE INSERT ON service_request_defaults WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;

CREATE TRIGGER v4_service_request_defaults_no_update BEFORE UPDATE ON service_request_defaults BEGIN SELECT RAISE(ABORT,'immutable_config_child'); END;

CREATE TRIGGER v4_service_request_defaults_sealed_delete BEFORE DELETE ON service_request_defaults WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;

CREATE TRIGGER v4_service_default_fallbacks_sealed_insert BEFORE INSERT ON service_default_fallbacks WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;

CREATE TRIGGER v4_service_default_fallbacks_no_update BEFORE UPDATE ON service_default_fallbacks BEGIN SELECT RAISE(ABORT,'immutable_config_child'); END;

CREATE TRIGGER v4_service_default_fallbacks_sealed_delete BEFORE DELETE ON service_default_fallbacks WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;

CREATE TRIGGER v4_activation_resource_materializations_sealed_insert BEFORE INSERT ON activation_resource_materializations WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=NEW.effective_revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;

CREATE TRIGGER v4_activation_resource_materializations_no_update BEFORE UPDATE ON activation_resource_materializations BEGIN SELECT RAISE(ABORT,'immutable_config_child'); END;

CREATE TRIGGER v4_activation_resource_materializations_sealed_delete BEFORE DELETE ON activation_resource_materializations WHEN EXISTS(SELECT 1 FROM config_revision_seals WHERE revision_id=OLD.effective_revision_id) AND NOT EXISTS(SELECT 1 FROM config_gc_marks WHERE revision_id=OLD.effective_revision_id) BEGIN SELECT RAISE(ABORT,'sealed_config_snapshot'); END;

CREATE TRIGGER v4_oauth_client_profiles_no_update BEFORE UPDATE ON oauth_client_profiles BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_oauth_authorization_snapshots_no_update BEFORE UPDATE ON oauth_authorization_snapshots BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_oauth_token_material_revisions_no_update BEFORE UPDATE ON oauth_token_material_revisions BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_runtime_profiles_no_update BEFORE UPDATE ON offline_runtime_profiles BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_execution_environments_no_update BEFORE UPDATE ON offline_execution_environments BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_installation_versions_no_update BEFORE UPDATE ON offline_installation_versions BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_resource_snapshots_no_update BEFORE UPDATE ON offline_resource_snapshots BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_resource_snapshot_files_no_update BEFORE UPDATE ON offline_resource_snapshot_files BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_resource_pairs_no_update BEFORE UPDATE ON offline_resource_pairs BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_resource_seals_no_update BEFORE UPDATE ON offline_resource_seals BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_translation_routes_no_update BEFORE UPDATE ON offline_translation_routes BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_translation_route_hops_no_update BEFORE UPDATE ON offline_translation_route_hops BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_route_seals_no_update BEFORE UPDATE ON offline_route_seals BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_resource_observations_no_update BEFORE UPDATE ON offline_resource_observations BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_task_execution_targets_no_update BEFORE UPDATE ON task_execution_targets BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_test_targets_no_update BEFORE UPDATE ON offline_test_targets BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_task_allowed_fallback_targets_no_update BEFORE UPDATE ON task_allowed_fallback_targets BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_attempt_execution_targets_no_update BEFORE UPDATE ON attempt_execution_targets BEGIN SELECT RAISE(ABORT,'immutable_service_fact'); END;

CREATE TRIGGER v4_offline_resource_snapshot_files_sealed_insert BEFORE INSERT ON offline_resource_snapshot_files WHEN EXISTS(SELECT 1 FROM offline_resource_seals s WHERE s.snapshot_id=NEW.snapshot_id) BEGIN SELECT RAISE(ABORT,'sealed_service_closure'); END;

CREATE TRIGGER v4_offline_resource_snapshot_files_sealed_delete BEFORE DELETE ON offline_resource_snapshot_files WHEN EXISTS(SELECT 1 FROM offline_resource_seals s WHERE s.snapshot_id=OLD.snapshot_id) BEGIN SELECT RAISE(ABORT,'sealed_service_closure'); END;

CREATE TRIGGER v4_offline_resource_pairs_sealed_insert BEFORE INSERT ON offline_resource_pairs WHEN EXISTS(SELECT 1 FROM offline_resource_seals s WHERE s.snapshot_id=NEW.snapshot_id) BEGIN SELECT RAISE(ABORT,'sealed_service_closure'); END;

CREATE TRIGGER v4_offline_resource_pairs_sealed_delete BEFORE DELETE ON offline_resource_pairs WHEN EXISTS(SELECT 1 FROM offline_resource_seals s WHERE s.snapshot_id=OLD.snapshot_id) BEGIN SELECT RAISE(ABORT,'sealed_service_closure'); END;

CREATE TRIGGER v4_offline_translation_route_hops_sealed_insert BEFORE INSERT ON offline_translation_route_hops WHEN EXISTS(SELECT 1 FROM offline_route_seals s WHERE s.route_id=NEW.route_id) BEGIN SELECT RAISE(ABORT,'sealed_service_closure'); END;

CREATE TRIGGER v4_offline_translation_route_hops_sealed_delete BEFORE DELETE ON offline_translation_route_hops WHEN EXISTS(SELECT 1 FROM offline_route_seals s WHERE s.route_id=OLD.route_id) BEGIN SELECT RAISE(ABORT,'sealed_service_closure'); END;

CREATE TRIGGER v4_offline_resource_snapshots_sealed_delete BEFORE DELETE ON offline_resource_snapshots WHEN EXISTS(SELECT 1 FROM offline_resource_seals s WHERE s.snapshot_id=OLD.snapshot_id) BEGIN SELECT RAISE(ABORT,'sealed_service_closure'); END;

CREATE TRIGGER v4_offline_translation_routes_sealed_delete BEFORE DELETE ON offline_translation_routes WHEN EXISTS(SELECT 1 FROM offline_route_seals s WHERE s.route_id=OLD.route_id) BEGIN SELECT RAISE(ABORT,'sealed_service_closure'); END;


ALTER TABLE purpose_bindings ADD COLUMN selection_revision INTEGER NOT NULL DEFAULT 0 CHECK(selection_revision BETWEEN 0 AND 4294967295);
CREATE UNIQUE INDEX v4_binding_selection_identity ON purpose_bindings(revision_id,scope_key,purpose,selection_revision);
CREATE UNIQUE INDEX v4_runtime_profile_engine_identity ON offline_runtime_profiles(runtime_profile_id,runtime_profile_revision,engine_id,engine_contract_revision);
ALTER TABLE purpose_bindings ADD COLUMN policy_overrides_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(policy_overrides_json) AND json_type(policy_overrides_json)='object' AND instr(policy_overrides_json,char(0))=0 AND length(CAST(policy_overrides_json AS BLOB))<=65536);
ALTER TABLE binding_engines ADD COLUMN policy_overrides_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(policy_overrides_json) AND json_type(policy_overrides_json)='object' AND instr(policy_overrides_json,char(0))=0 AND length(CAST(policy_overrides_json AS BLOB))<=65536);
ALTER TABLE cache_entries ADD COLUMN origin_attempt_id TEXT REFERENCES request_attempts(attempt_id) ON DELETE RESTRICT;


CREATE TABLE execution_authorization_leases(
 lease_id TEXT NOT NULL PRIMARY KEY CHECK(length(lease_id)>0),
 attempt_id TEXT UNIQUE, task_id TEXT, test_id TEXT UNIQUE,
 origin_kind TEXT NOT NULL CHECK(origin_kind IN('service_account','user_oauth')),
 grant_id TEXT, authorization_epoch INTEGER, material_revision INTEGER,
 pool_id TEXT, pool_revision INTEGER, member_id TEXT,
 credential_id TEXT NOT NULL, auth_kind TEXT NOT NULL CHECK(auth_kind IN('google_oauth_refresh','google_service_account')),
 lifecycle_revision INTEGER NOT NULL CHECK(lifecycle_revision BETWEEN 0 AND 4294967295),
 scopes_sha256 TEXT NOT NULL CHECK(length(scopes_sha256)=64 AND scopes_sha256 NOT GLOB '*[^0-9a-f]*'),
 client_policy_sha256 TEXT, target_policy_sha256 TEXT NOT NULL CHECK(length(target_policy_sha256)=64 AND target_policy_sha256 NOT GLOB '*[^0-9a-f]*'),
 minted_at_ms INTEGER NOT NULL CHECK(minted_at_ms>=0), expires_at_ms INTEGER NOT NULL CHECK(expires_at_ms>minted_at_ms),
 FOREIGN KEY(attempt_id,task_id) REFERENCES request_attempts(attempt_id,task_id) ON DELETE CASCADE,
 FOREIGN KEY(test_id) REFERENCES test_records(test_id) ON DELETE CASCADE,
 FOREIGN KEY(grant_id,authorization_epoch) REFERENCES oauth_authorization_snapshots(grant_id,authorization_epoch) ON DELETE RESTRICT,
 FOREIGN KEY(grant_id,material_revision,credential_id) REFERENCES oauth_token_material_revisions(grant_id,material_revision,credential_id) ON DELETE RESTRICT,
 FOREIGN KEY(pool_id,pool_revision,member_id,credential_id) REFERENCES credential_pool_members(pool_id,pool_revision,member_id,credential_id) ON DELETE RESTRICT,
 FOREIGN KEY(credential_id,auth_kind) REFERENCES credential_refs(credential_id,auth_kind) ON DELETE RESTRICT,
 CHECK((attempt_id IS NOT NULL AND task_id IS NOT NULL AND test_id IS NULL) OR (attempt_id IS NULL AND task_id IS NULL AND test_id IS NOT NULL)),
 CHECK((origin_kind='service_account' AND auth_kind='google_service_account' AND pool_id IS NOT NULL AND pool_revision IS NOT NULL AND member_id IS NOT NULL AND grant_id IS NULL AND authorization_epoch IS NULL AND material_revision IS NULL AND client_policy_sha256 IS NULL) OR (origin_kind='user_oauth' AND auth_kind='google_oauth_refresh' AND grant_id IS NOT NULL AND authorization_epoch IS NOT NULL AND material_revision IS NOT NULL AND pool_id IS NULL AND pool_revision IS NULL AND member_id IS NULL AND client_policy_sha256 IS NOT NULL AND length(client_policy_sha256)=64 AND client_policy_sha256 NOT GLOB '*[^0-9a-f]*'))
) STRICT;
CREATE TRIGGER v4_execution_authorization_immutable BEFORE UPDATE ON execution_authorization_leases BEGIN SELECT RAISE(ABORT,'immutable_execution_authority'); END;
CREATE TRIGGER v4_oauth_grants_control BEFORE UPDATE ON oauth_grants WHEN NEW.grant_id<>OLD.grant_id OR NEW.client_profile_id<>OLD.client_profile_id OR NEW.client_profile_revision<>OLD.client_profile_revision OR NEW.principal_state<>OLD.principal_state OR NEW.principal_subject IS NOT OLD.principal_subject OR NEW.created_at_ms<>OLD.created_at_ms OR NEW.control_revision<>OLD.control_revision+1 OR (NEW.authorization_epoch IS OLD.authorization_epoch AND NEW.material_revision IS OLD.material_revision) OR (OLD.authorization_epoch IS NOT NULL AND (NEW.authorization_epoch IS NULL OR NEW.authorization_epoch<OLD.authorization_epoch)) OR (OLD.material_revision IS NOT NULL AND (NEW.material_revision IS NULL OR NEW.material_revision<OLD.material_revision)) BEGIN SELECT RAISE(ABORT,'stale_or_changed_grant_identity'); END;
CREATE TRIGGER v4_oauth_grant_ready BEFORE UPDATE ON oauth_grants WHEN EXISTS(SELECT 1 FROM oauth_authorization_snapshots s WHERE s.grant_id=NEW.grant_id AND s.authorization_epoch=NEW.authorization_epoch AND s.state='active') AND (NOT EXISTS(SELECT 1 FROM oauth_client_profiles c WHERE c.client_profile_id=NEW.client_profile_id AND c.client_profile_revision=NEW.client_profile_revision AND c.setup_state='verified') OR NOT EXISTS(SELECT 1 FROM oauth_token_material_revisions m JOIN credential_refs r ON r.credential_id=m.credential_id WHERE m.grant_id=NEW.grant_id AND m.material_revision=NEW.material_revision AND r.state='active')) BEGIN SELECT RAISE(ABORT,'oauth_not_ready'); END;
CREATE TRIGGER v4_runtime_route_closure BEFORE INSERT ON offline_route_seals WHEN (SELECT count(DISTINCT s.runtime_profile_id||':'||s.runtime_profile_revision) FROM offline_translation_route_hops h JOIN offline_resource_snapshots s ON s.snapshot_id=h.snapshot_id WHERE h.route_id=NEW.route_id)<>1 BEGIN SELECT RAISE(ABORT,'mixed_runtime_route'); END;
CREATE TRIGGER v4_config_installation_runtime BEFORE INSERT ON offline_engine_configs WHEN NEW.installation_id IS NOT NULL AND NEW.snapshot_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM offline_installation_versions i JOIN offline_resource_snapshots s ON s.runtime_profile_id=i.runtime_profile_id AND s.runtime_profile_revision=i.runtime_profile_revision WHERE i.installation_id=NEW.installation_id AND i.installation_revision=NEW.installation_revision AND i.engine_id=NEW.engine_id AND i.engine_contract_revision=NEW.engine_contract_revision AND s.snapshot_id=NEW.snapshot_id) BEGIN SELECT RAISE(ABORT,'installation_resource_runtime_mismatch'); END;


CREATE VIEW v4_attempt_target_authority AS
SELECT a.attempt_id FROM request_attempts a
JOIN attempt_execution_targets x ON x.attempt_id=a.attempt_id AND x.task_id=a.task_id
JOIN binding_engines e ON e.revision_id=x.config_revision_id AND e.binding_id=x.binding_id AND e.engine_id=x.engine_id AND e.target_kind=x.target_kind
JOIN purpose_bindings b ON b.revision_id=e.revision_id AND b.binding_id=e.binding_id
JOIN task_execution_targets t ON t.task_id=a.task_id
WHERE e.enabled=1 AND b.enabled=1 AND b.binding_mode='override'
AND (t.selection_scope IS NULL OR (EXISTS(SELECT 1 FROM config_heads head JOIN purpose_bindings current ON current.revision_id=head.revision_id WHERE head.head_kind='applied' AND current.scope_key=t.selection_scope AND current.purpose=b.purpose AND current.selection_revision=t.selection_revision) AND EXISTS(SELECT 1 FROM config_heads head JOIN purpose_bindings current ON current.revision_id=head.revision_id WHERE head.head_kind='applied' AND current.scope_key=t.effective_scope AND current.purpose=b.purpose AND current.selection_revision=t.effective_selection_revision)))
AND ((x.target_kind='http' AND EXISTS(
 SELECT 1 FROM models m JOIN connections c ON c.revision_id=m.revision_id AND c.connection_id=m.connection_id
 LEFT JOIN activation_resource_materializations ac ON ac.effective_revision_id=c.revision_id AND ac.effective_connection_id=c.connection_id
 JOIN connection_runtime_states gate ON gate.connection_id=coalesce(ac.control_connection_id,c.connection_id)
 WHERE m.revision_id=x.config_revision_id AND m.model_id=x.http_model_id AND c.connection_id=x.http_connection_id AND c.availability_intent=1 AND gate.is_suspended=0
 AND ((c.auth_mode='none' AND a.credential_id IS NULL)
 OR (c.auth_mode IN('api_key','baidu_app_secret','youdao_app_secret','google_service_account') AND EXISTS(SELECT 1 FROM credential_pool_members pm JOIN credential_refs cr ON cr.credential_id=pm.credential_id WHERE pm.pool_id=c.credential_pool_id AND pm.pool_revision=c.credential_pool_revision AND pm.credential_id=a.credential_id AND pm.enabled=1 AND cr.state='active' AND cr.auth_kind=c.auth_mode AND (c.auth_mode<>'google_service_account' OR EXISTS(SELECT 1 FROM execution_authorization_leases sl WHERE sl.origin_kind='service_account' AND sl.attempt_id=a.attempt_id AND sl.pool_id=pm.pool_id AND sl.pool_revision=pm.pool_revision AND sl.member_id=pm.member_id AND sl.credential_id=cr.credential_id AND sl.lifecycle_revision=cr.lifecycle_revision))))
 OR (c.auth_mode='google_oauth' AND a.credential_id IS NULL AND EXISTS(SELECT 1 FROM execution_authorization_leases l JOIN oauth_grants h ON h.grant_id=l.grant_id AND h.authorization_epoch=l.authorization_epoch JOIN oauth_authorization_snapshots s ON s.grant_id=h.grant_id AND s.authorization_epoch=h.authorization_epoch JOIN oauth_token_material_revisions material ON material.grant_id=l.grant_id AND material.material_revision=l.material_revision AND material.credential_id=l.credential_id JOIN credential_refs cr ON cr.credential_id=material.credential_id WHERE l.origin_kind='user_oauth' AND l.attempt_id=a.attempt_id AND l.grant_id=c.oauth_grant_id AND l.authorization_epoch=c.oauth_authorization_epoch AND s.state='active' AND cr.state='active' AND cr.lifecycle_revision=l.lifecycle_revision)))))
 OR (x.target_kind='offline' AND a.credential_id IS NULL AND EXISTS(
 SELECT 1 FROM offline_engine_configs oc JOIN offline_resource_seals rs ON rs.snapshot_id=oc.snapshot_id JOIN offline_route_seals routes ON routes.route_id=oc.route_id
 JOIN offline_resource_observations obs ON obs.observation_id=x.observation_id AND obs.snapshot_id=x.snapshot_id AND obs.observation_epoch=x.observation_epoch
 JOIN offline_observation_heads oh ON oh.subject_key=obs.subject_key AND oh.observation_id=obs.observation_id AND oh.observation_epoch=obs.observation_epoch
 LEFT JOIN activation_resource_materializations ac ON ac.effective_revision_id=oc.revision_id AND ac.effective_offline_config_id=oc.engine_config_id
 JOIN offline_config_runtime_states gate ON gate.engine_config_id=coalesce(ac.control_config_id,oc.engine_config_id)
 JOIN offline_resource_snapshots snap ON snap.snapshot_id=oc.snapshot_id JOIN offline_runtime_profiles rp ON rp.runtime_profile_id=snap.runtime_profile_id AND rp.runtime_profile_revision=snap.runtime_profile_revision
 WHERE oc.revision_id=x.config_revision_id AND oc.engine_config_id=x.offline_engine_config_id AND oc.resolution='resolved' AND oc.snapshot_id=x.snapshot_id AND oc.route_id=x.route_id AND obs.state='installed' AND gate.is_suspended=0 AND rp.verification_state='verified'
 AND ((oc.backend='platform_native' AND oc.installation_id IS NULL) OR (oc.backend='local_worker' AND EXISTS(SELECT 1 FROM offline_installation_versions i JOIN offline_installation_heads ih ON ih.installation_id=i.installation_id AND ih.installation_revision=i.installation_revision JOIN offline_local_grants g ON g.grant_id=i.local_grant_id WHERE i.installation_id=oc.installation_id AND i.installation_revision=oc.installation_revision AND ih.enabled=1 AND g.state='active' AND obs.local_grant_id=g.grant_id AND obs.local_grant_epoch=g.authorization_epoch))))));

CREATE TRIGGER v4_typed_no_legacy_policy_shadow BEFORE INSERT ON purpose_bindings WHEN NEW.semantics_version=3 AND (json(NEW.context_json)<>'{}' OR json(NEW.schedule_json)<>'{}') BEGIN SELECT RAISE(ABORT,'typed_policy_has_legacy_shadow'); END;

CREATE TRIGGER v4_http_origin_shape BEFORE INSERT ON connections WHEN NEW.origin_json IS NOT NULL AND (
 (json_type(NEW.origin_json,'$.site') IS NOT NULL AND (json_type(NEW.origin_json,'$.site') IS NOT 'text' OR instr(json_extract(NEW.origin_json,'$.site'),char(0))<>0))
 OR  (json_extract(NEW.origin_json,'$.kind')='preset' AND (json_type(NEW.origin_json,'$.key') IS NOT 'object' OR json_extract(NEW.origin_json,'$.key.presetId') IS NOT NEW.provider_preset_id OR json_extract(NEW.origin_json,'$.key.presetRevision') IS NOT NEW.provider_preset_revision OR NEW.provider_preset_id IS NULL OR EXISTS(SELECT 1 FROM json_each(NEW.origin_json) WHERE key NOT IN('kind','key','site')) OR EXISTS(SELECT 1 FROM json_each(NEW.origin_json,'$.key') WHERE key NOT IN('presetId','presetRevision'))))
 OR (json_extract(NEW.origin_json,'$.kind')='custom' AND (EXISTS(SELECT 1 FROM json_each(NEW.origin_json) WHERE key NOT IN('kind','copiedFrom','site')) OR (json_type(NEW.origin_json,'$.copiedFrom') IS NOT NULL AND (json_type(NEW.origin_json,'$.copiedFrom') IS NOT 'object' OR EXISTS(SELECT 1 FROM json_each(NEW.origin_json,'$.copiedFrom') WHERE key NOT IN('presetId','presetRevision')) OR NOT EXISTS(SELECT 1 FROM provider_preset_versions p WHERE p.preset_id=json_extract(NEW.origin_json,'$.copiedFrom.presetId') AND p.preset_revision=json_extract(NEW.origin_json,'$.copiedFrom.presetRevision')))))))
 BEGIN SELECT RAISE(ABORT,'http_origin_mismatch'); END;


CREATE TRIGGER v4_lease_actual_connection BEFORE INSERT ON execution_authorization_leases WHEN ((NEW.attempt_id IS NOT NULL AND EXISTS(SELECT 1 FROM request_attempts WHERE attempt_id=NEW.attempt_id)) OR (NEW.test_id IS NOT NULL AND EXISTS(SELECT 1 FROM test_records WHERE test_id=NEW.test_id))) AND NOT EXISTS(
 SELECT 1 FROM connections c WHERE
 ((NEW.attempt_id IS NOT NULL AND EXISTS(SELECT 1 FROM attempt_execution_targets x WHERE x.attempt_id=NEW.attempt_id AND x.task_id=NEW.task_id AND x.target_kind='http' AND x.config_revision_id=c.revision_id AND x.http_connection_id=c.connection_id))
 OR (NEW.test_id IS NOT NULL AND EXISTS(SELECT 1 FROM test_records t WHERE t.test_id=NEW.test_id AND t.config_revision_id=c.revision_id AND t.connection_id=c.connection_id)))
 AND ((NEW.origin_kind='user_oauth' AND c.auth_mode='google_oauth' AND c.oauth_grant_id=NEW.grant_id AND c.oauth_authorization_epoch=NEW.authorization_epoch)
 OR (NEW.origin_kind='service_account' AND c.auth_mode='google_service_account' AND c.credential_pool_id=NEW.pool_id AND c.credential_pool_revision=NEW.pool_revision)))
 BEGIN SELECT RAISE(ABORT,'lease_actual_target_mismatch'); END;
CREATE TRIGGER v4_config_no_duplicate_logical_service BEFORE INSERT ON config_revision_seals WHEN EXISTS(
 SELECT 1 FROM binding_engines e LEFT JOIN activation_resource_materializations a ON a.effective_revision_id=e.revision_id AND ((e.target_kind='http' AND a.effective_model_id=e.model_id) OR (e.target_kind='offline' AND a.effective_offline_config_id=e.offline_engine_config_id))
 WHERE e.revision_id=NEW.revision_id AND EXISTS(SELECT 1 FROM purpose_bindings b JOIN config_revisions c ON c.revision_id=b.revision_id WHERE b.revision_id=e.revision_id AND b.binding_id=e.binding_id AND b.semantics_version=3 AND c.codec_version=3) GROUP BY e.binding_id,e.target_kind,coalesce(a.source_model_id,a.source_offline_config_id,e.model_id,e.offline_engine_config_id) HAVING count(*)>1)
 BEGIN SELECT RAISE(ABORT,'duplicate_logical_service'); END;


CREATE TRIGGER v4_oauth_grant_initial_control BEFORE INSERT ON oauth_grants WHEN NEW.authorization_epoch IS NOT NULL OR NEW.material_revision IS NOT NULL OR NEW.control_revision<>0 BEGIN SELECT RAISE(ABORT,'grant_initial_control_must_be_pending'); END;
CREATE TRIGGER v4_resource_gc_references BEFORE DELETE ON offline_resource_seals WHEN EXISTS(SELECT 1 FROM offline_engine_configs WHERE snapshot_id=OLD.snapshot_id) OR EXISTS(SELECT 1 FROM offline_translation_route_hops WHERE snapshot_id=OLD.snapshot_id) OR EXISTS(SELECT 1 FROM offline_resource_observations WHERE snapshot_id=OLD.snapshot_id) BEGIN SELECT RAISE(ABORT,'resource_still_referenced'); END;
CREATE TRIGGER v4_route_gc_references BEFORE DELETE ON offline_route_seals WHEN EXISTS(SELECT 1 FROM offline_engine_configs WHERE route_id=OLD.route_id) BEGIN SELECT RAISE(ABORT,'route_still_referenced'); END;
CREATE TRIGGER v4_config_selection_shape BEFORE INSERT ON config_revision_seals WHEN EXISTS(SELECT 1 FROM purpose_bindings b WHERE b.revision_id=NEW.revision_id AND b.semantics_version=3 AND b.binding_mode='override' AND EXISTS(SELECT 1 FROM binding_engines e WHERE e.revision_id=b.revision_id AND e.binding_id=b.binding_id) AND ((SELECT min(position) FROM binding_engines e WHERE e.revision_id=b.revision_id AND e.binding_id=b.binding_id)<>0 OR (SELECT max(position)+1 FROM binding_engines e WHERE e.revision_id=b.revision_id AND e.binding_id=b.binding_id)<>(SELECT count(*) FROM binding_engines e WHERE e.revision_id=b.revision_id AND e.binding_id=b.binding_id))) BEGIN SELECT RAISE(ABORT,'selection_order_gap'); END;
CREATE TRIGGER v4_config_typed_graph_seal BEFORE INSERT ON config_revision_seals WHEN EXISTS(SELECT 1 FROM config_revisions c WHERE c.revision_id=NEW.revision_id AND c.codec_version=3) AND (
 EXISTS(SELECT 1 FROM models m WHERE m.revision_id=NEW.revision_id AND m.template_purpose IS NULL AND NOT EXISTS(SELECT 1 FROM activation_resource_materializations o JOIN config_revisions s ON s.revision_id=o.source_revision_id WHERE o.effective_revision_id=m.revision_id AND o.effective_model_id=m.model_id AND o.reason='legacy_materialization' AND s.codec_version<3))
 OR EXISTS(SELECT 1 FROM connections c WHERE c.revision_id=NEW.revision_id AND (c.adapter_binding_schema IS NULL OR c.http_options_schema IS NULL OR c.origin_schema IS NULL) AND NOT EXISTS(SELECT 1 FROM activation_resource_materializations a JOIN config_revisions s ON s.revision_id=a.source_revision_id WHERE a.effective_revision_id=c.revision_id AND a.effective_connection_id=c.connection_id AND a.reason='legacy_materialization' AND s.codec_version<3))
 OR EXISTS(SELECT 1 FROM binding_engines e JOIN models m ON m.revision_id=e.revision_id AND m.model_id=e.model_id WHERE e.revision_id=NEW.revision_id AND e.target_kind='http' AND m.template_purpose IS NOT NULL AND m.template_purpose<>e.purpose)
 OR EXISTS(SELECT 1 FROM offline_engine_configs c WHERE c.revision_id=NEW.revision_id AND c.resolution='resolved' AND (NOT EXISTS(SELECT 1 FROM offline_resource_seals s WHERE s.snapshot_id=c.snapshot_id) OR NOT EXISTS(SELECT 1 FROM offline_route_seals r WHERE r.route_id=c.route_id)))
 OR EXISTS(SELECT 1 FROM activation_resource_materializations a JOIN models m ON m.revision_id=a.effective_revision_id AND m.connection_id=a.effective_connection_id WHERE a.effective_revision_id=NEW.revision_id AND a.target_kind='connection')
) BEGIN SELECT RAISE(ABORT,'invalid_codec3_graph'); END;
CREATE TRIGGER v4_materialized_control BEFORE INSERT ON activation_resource_materializations WHEN
 (NEW.target_kind IN('http','connection') AND NEW.control_connection_id<>coalesce((SELECT control_connection_id FROM activation_resource_materializations WHERE effective_revision_id=NEW.source_revision_id AND effective_connection_id=NEW.source_connection_id LIMIT 1),NEW.source_connection_id))
 OR (NEW.target_kind='offline' AND NEW.control_config_id<>coalesce((SELECT control_config_id FROM activation_resource_materializations WHERE effective_revision_id=NEW.source_revision_id AND effective_offline_config_id=NEW.source_offline_config_id LIMIT 1),NEW.source_offline_config_id))
 OR (NEW.target_kind='connection' AND (
   NEW.effective_revision_id=NEW.source_revision_id
   OR NOT EXISTS(SELECT 1 FROM config_revisions e WHERE e.revision_id=NEW.effective_revision_id AND e.codec_version=3)
   OR NOT EXISTS(SELECT 1 FROM config_revisions s JOIN config_revision_seals z ON z.revision_id=s.revision_id AND z.content_hash=s.content_hash WHERE s.revision_id=NEW.source_revision_id AND s.codec_version<3 AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=s.revision_id))
   OR EXISTS(SELECT 1 FROM models WHERE revision_id=NEW.source_revision_id AND connection_id=NEW.source_connection_id)
   OR EXISTS(SELECT 1 FROM models WHERE revision_id=NEW.effective_revision_id AND connection_id=NEW.effective_connection_id)
   OR NEW.control_connection_id IS NOT NEW.source_connection_id
 ))
 OR (NEW.target_kind IN('http','connection') AND EXISTS(
   SELECT 1 FROM activation_resource_materializations a WHERE a.effective_revision_id=NEW.effective_revision_id AND a.effective_connection_id=NEW.effective_connection_id AND a.target_kind IN('http','connection')
   AND (NEW.target_kind='connection' OR a.target_kind='connection' OR a.source_revision_id IS NOT NEW.source_revision_id OR a.source_connection_id IS NOT NEW.source_connection_id OR a.control_connection_id IS NOT NEW.control_connection_id)
 ))
 BEGIN SELECT RAISE(ABORT,'materialization_control_bypass'); END;
CREATE TRIGGER v4_engine_policy_fields BEFORE INSERT ON binding_engines WHEN EXISTS(SELECT 1 FROM json_each(NEW.policy_overrides_json) WHERE key NOT IN('context','execution','cache')) BEGIN SELECT RAISE(ABORT,'unknown_policy_override'); END;
CREATE TRIGGER v4_scope_policy_fields BEFORE INSERT ON purpose_bindings WHEN EXISTS(SELECT 1 FROM json_each(NEW.policy_overrides_json) WHERE key NOT IN('context','execution','cache')) BEGIN SELECT RAISE(ABORT,'unknown_policy_override'); END;


CREATE TRIGGER v4_installation_environment_runtime BEFORE INSERT ON offline_installation_versions WHEN NOT EXISTS(
 SELECT 1 FROM offline_execution_environments e JOIN offline_runtime_profiles r ON r.runtime_profile_id=NEW.runtime_profile_id AND r.runtime_profile_revision=NEW.runtime_profile_revision JOIN offline_local_grants g ON g.grant_id=NEW.local_grant_id
 WHERE e.environment_id=NEW.environment_id AND e.environment_revision=NEW.environment_revision AND r.architecture=e.architecture AND r.platform=CASE WHEN e.platform='crossover' THEN 'windows' ELSE e.platform END AND g.machine_fingerprint=e.machine_fingerprint)
 BEGIN SELECT RAISE(ABORT,'installation_environment_runtime_mismatch'); END;
