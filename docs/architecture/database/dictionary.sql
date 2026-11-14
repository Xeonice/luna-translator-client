-- 随包只读辞书索引的构建设计，不是可写用户库迁移；运行以只读连接打开。
PRAGMA foreign_keys = ON;
-- @table dictionary_meta | JA | FP-07,FP-11 | 单个固定辞书索引及规范化算法的发布身份
CREATE TABLE dictionary_meta (
    singleton INTEGER NOT NULL PRIMARY KEY CHECK(singleton=1), -- 唯一元数据行
    resource_version TEXT NOT NULL, -- 发布清单中的资源版本
    manifest_sha256 TEXT NOT NULL CHECK(instr(manifest_sha256,char(0))=0 AND (length(manifest_sha256)=64 AND manifest_sha256 NOT GLOB '*[^0-9a-f]*')), -- 实际资源manifest摘要
    index_schema_version INTEGER NOT NULL CHECK(index_schema_version>0), -- 查询索引格式版本
    normalization_version TEXT NOT NULL, -- 词形/读音键规范化规则版本
    built_at_ms INTEGER NOT NULL CHECK(built_at_ms>=0) -- 离线构建时间UTC毫秒
) STRICT;
-- @table dictionary_sources | JA | FP-07,FP-11 | 每个辞书来源、快照与必须显示的归属许可
CREATE TABLE dictionary_sources (
    source_id TEXT NOT NULL PRIMARY KEY, -- 稳定来源标识
    display_name TEXT NOT NULL, -- 用户可见来源名
    snapshot_version TEXT NOT NULL, -- 数据快照版本
    source_sha256 TEXT NOT NULL CHECK(instr(source_sha256,char(0))=0 AND (length(source_sha256)=64 AND source_sha256 NOT GLOB '*[^0-9a-f]*')), -- 原数据身份
    source_url TEXT NOT NULL, -- 原来源定位，不在运行时自动联网
    license_ref TEXT NOT NULL, -- 包内许可文件引用
    attribution_text TEXT NOT NULL -- 必要归属文字
) STRICT;
-- @table dictionary_entries | JA | FP-07 | 日语词条身份与中文基础/英语补充的明确区分
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
-- @table dictionary_forms | JA | FP-07 | 词面/词元/读音查询的B-tree键，未知词不伪造命中
CREATE TABLE dictionary_forms (
    form_id TEXT NOT NULL PRIMARY KEY, -- 查询词形ID
    entry_id TEXT NOT NULL REFERENCES dictionary_entries(entry_id) ON DELETE CASCADE, -- 对应词条
    form_text TEXT NOT NULL, -- 原词形
    normalized_key TEXT NOT NULL, -- 按dictionary_meta规则生成的精确查询键
    form_kind TEXT NOT NULL CHECK(form_kind IN ('surface','lemma','reading')), -- 查询形态类别
    rank_weight INTEGER NOT NULL CHECK(rank_weight>=0), -- 资源构建确定的排序值
    UNIQUE(entry_id,normalized_key,form_kind)
) STRICT;
CREATE INDEX dictionary_forms_lookup ON dictionary_forms(normalized_key,form_kind,rank_weight,entry_id);
-- @table dictionary_senses | JA | FP-07 | 释义正文、语言与来源定位，不把英语补充伪称中文释义
CREATE TABLE dictionary_senses (
    entry_id TEXT NOT NULL REFERENCES dictionary_entries(entry_id) ON DELETE CASCADE, -- 对应词条
    sense_index INTEGER NOT NULL CHECK(sense_index>=0), -- 词条内释义顺序
    gloss_language TEXT NOT NULL CHECK(gloss_language IN ('zh','en')), -- 释义语言
    gloss_text TEXT NOT NULL, -- 实际离线释义
    usage_notes TEXT, -- 用法说明
    source_locator TEXT NOT NULL, -- 来源页/条目定位，遵循原数据许可
    PRIMARY KEY(entry_id,sense_index)
) STRICT;
CREATE TRIGGER dictionary_gloss_language BEFORE INSERT ON dictionary_senses
WHEN NOT EXISTS(SELECT 1 FROM dictionary_entries WHERE entry_id=NEW.entry_id AND ((content_kind='zh_base' AND NEW.gloss_language='zh') OR (content_kind='en_supplement' AND NEW.gloss_language='en')))
BEGIN SELECT RAISE(ABORT,'dictionary_gloss_language'); END;

CREATE TRIGGER dictionary_gloss_language_update BEFORE UPDATE ON dictionary_senses
WHEN NOT EXISTS(SELECT 1 FROM dictionary_entries WHERE entry_id=NEW.entry_id AND ((content_kind='zh_base' AND NEW.gloss_language='zh') OR (content_kind='en_supplement' AND NEW.gloss_language='en')))
BEGIN SELECT RAISE(ABORT,'dictionary_gloss_language'); END;
CREATE TRIGGER dictionary_entry_language_update BEFORE UPDATE OF content_kind ON dictionary_entries
WHEN EXISTS(SELECT 1 FROM dictionary_senses WHERE entry_id=OLD.entry_id AND ((NEW.content_kind='zh_base' AND gloss_language<>'zh') OR (NEW.content_kind='en_supplement' AND gloss_language<>'en')))
BEGIN SELECT RAISE(ABORT,'dictionary_gloss_language'); END;
