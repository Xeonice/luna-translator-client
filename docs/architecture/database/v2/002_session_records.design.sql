-- REVIEW CANDIDATE ONLY. Applies after immutable 001_profile + 001_config.
-- No ALTER/UPDATE/DELETE of old schema, no seeded product successes.
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
CREATE INDEX ix_capture_operations_state ON capture_operations(state,started_at_ms);
CREATE INDEX ix_capture_operations_session ON capture_operations(session_id,started_at_ms,operation_id);
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
CREATE INDEX ix_hook_events_operation_order ON hook_events(operation_id,event_sequence);
CREATE INDEX ix_hook_events_session_order ON hook_events(session_id,event_sequence);
CREATE INDEX ix_hook_events_filter ON hook_events(operation_id,level,stage,event_sequence);
CREATE TABLE body_record_positions (
    record_sequence INTEGER PRIMARY KEY AUTOINCREMENT CHECK(record_sequence>0),
    session_id TEXT NOT NULL,
    unit_id TEXT NOT NULL,
    text_revision INTEGER NOT NULL CHECK(text_revision>0),
    FOREIGN KEY(unit_id,text_revision,session_id) REFERENCES reading_unit_revisions(unit_id,text_revision,session_id) ON DELETE CASCADE,
    UNIQUE(unit_id,text_revision)
) STRICT;
CREATE INDEX ix_body_records_session_order ON body_record_positions(session_id,record_sequence);
CREATE TABLE reading_preference_settings (
    preference_revision_id TEXT NOT NULL PRIMARY KEY,
    segmentation_enabled INTEGER NOT NULL DEFAULT 1 CHECK(segmentation_enabled IN (0,1)),
    reading_display_mode TEXT NOT NULL DEFAULT 'both' CHECK(reading_display_mode IN ('both','source','translation') AND instr(reading_display_mode,char(0))=0),
    hud_background_opacity REAL NOT NULL DEFAULT 0.70 CHECK(hud_background_opacity>=0.0 AND hud_background_opacity<=1.0),
    FOREIGN KEY(preference_revision_id) REFERENCES preference_revisions(preference_revision_id) ON DELETE CASCADE
) STRICT;
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
-- Backfill existing immutable source revisions in deterministic prior reading order.
INSERT INTO body_record_positions(session_id,unit_id,text_revision)
SELECT r.session_id,r.unit_id,r.text_revision FROM reading_unit_revisions r JOIN reading_units u USING(unit_id)
ORDER BY r.session_id,u.unit_ordinal,r.text_revision;
-- Historical sessions receive identities, never invented historical event rows.
INSERT INTO capture_operations(operation_id,request_sha256,session_id,state,started_at_ms,ended_at_ms,legacy_unknown_start)
SELECT 'legacy-session:'||session_id, printf('%064d',0),session_id,
 CASE WHEN state IN ('opening','active') THEN 'active' ELSE 'recovered' END, started_at_ms,ended_at_ms,1
FROM sessions WHERE source_kind='hook';
-- Preserve explicit legacy segmentation value, including false; missing means true.
INSERT INTO reading_preference_settings(preference_revision_id,segmentation_enabled,reading_display_mode,hud_background_opacity)
SELECT preference_revision_id,CASE WHEN json_type(overrides_json,'$."reading.segmentationEnabled"') IN ('true','false') THEN json_extract(overrides_json,'$."reading.segmentationEnabled"') ELSE 1 END,'both',0.70
FROM preference_revisions;
