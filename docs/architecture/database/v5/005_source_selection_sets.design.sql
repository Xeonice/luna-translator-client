-- INDEPENDENT SCHEMA5 TECHNICAL DESIGN CANDIDATE; NOT A PRODUCTION MIGRATION.
-- Apply after exact schema4. Never rewrite migrations 001-004 or historical lineage.
-- One SQLite writer transaction must apply migration/backfill, validate FKs and record ledger.
-- MAX_SELECTION_MEMBERS=64 is a bounded submission policy, not a test-only exception.

-- @table source_selection_sets | AP | FP-03,FP-04,FP-05,FP-09 | One immutable user-confirmed set; sealed only after complete members
CREATE TABLE source_selection_sets (
    session_id TEXT NOT NULL CHECK(length(session_id)>0 AND instr(session_id,char(0))=0), -- Owning actual Hook session
    selection_set_revision INTEGER NOT NULL CHECK(selection_set_revision>0), -- Monotonic set version; canonical decimal string in new IPC
    operation_id TEXT UNIQUE CHECK(operation_id IS NULL OR (length(operation_id)>0 AND instr(operation_id,char(0))=0)), -- Existing operation_receipts authority; NULL only for legacy backfill
    origin_kind TEXT NOT NULL CHECK(origin_kind IN ('explicit_set','legacy_singleton')), -- Distinguishes real command and lossless migration
    persistence_kind TEXT NOT NULL CHECK(persistence_kind IN ('temporary','saved_snapshot')), -- Temporary selection never deletes last saved set
    match_snapshot_json TEXT NOT NULL CHECK(instr(match_snapshot_json,char(0))=0 AND json_valid(match_snapshot_json) AND json_type(match_snapshot_json)='object'), -- Versioned matching explanation; Rust validates inner types/portable proof
    selected_at_ms INTEGER NOT NULL CHECK(selected_at_ms>=0), -- Preserves original selection time for legacy rows
    member_count INTEGER NOT NULL CHECK(member_count BETWEEN 1 AND 64), -- Complete declared set cardinality
    state TEXT NOT NULL CHECK(state IN ('building','sealed')), -- No reader consumes a partial building set
    PRIMARY KEY(session_id,selection_set_revision),
    FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE RESTRICT,
    FOREIGN KEY(operation_id) REFERENCES operation_receipts(operation_id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    CHECK((origin_kind='explicit_set' AND operation_id IS NOT NULL) OR (origin_kind='legacy_singleton' AND operation_id IS NULL AND member_count=1))
) STRICT;
CREATE INDEX ix_selection_sets_saved ON source_selection_sets(persistence_kind,state,selected_at_ms DESC,session_id,selection_set_revision DESC);

-- @table source_selection_members | AP | FP-03,FP-04,FP-05 | Retains each member's existing single-source lineage revision
CREATE TABLE source_selection_members (
    session_id TEXT NOT NULL CHECK(length(session_id)>0 AND instr(session_id,char(0))=0), -- Same actual session as header and source
    selection_set_revision INTEGER NOT NULL CHECK(selection_set_revision>0), -- Owning immutable set
    source_id TEXT NOT NULL CHECK(length(source_id)>0 AND instr(source_id,char(0))=0), -- Existing complete source identity; never Hook display name
    selection_revision INTEGER NOT NULL CHECK(selection_revision>0), -- Existing source_selections per-member revision; never a representative source
    member_ordinal INTEGER NOT NULL CHECK(member_ordinal BETWEEN 0 AND 63), -- Stable confirmed display order, contiguous from zero
    PRIMARY KEY(session_id,selection_set_revision,source_id),
    UNIQUE(session_id,selection_set_revision,member_ordinal),
    UNIQUE(session_id,selection_revision),
    FOREIGN KEY(session_id,selection_set_revision) REFERENCES source_selection_sets(session_id,selection_set_revision) ON DELETE RESTRICT,
    FOREIGN KEY(session_id,selection_revision) REFERENCES source_selections(session_id,selection_revision) ON DELETE RESTRICT,
    FOREIGN KEY(source_id,session_id) REFERENCES capture_sources(source_id,session_id) ON DELETE RESTRICT
) STRICT;

CREATE TRIGGER source_selection_set_building_insert BEFORE INSERT ON source_selection_sets
WHEN NEW.state<>'building'
 OR (NEW.origin_kind='explicit_set' AND NEW.selection_set_revision<>(SELECT coalesce(max(selection_set_revision),0)+1 FROM source_selection_sets WHERE session_id=NEW.session_id))
 OR (NEW.origin_kind='explicit_set' AND NOT EXISTS(SELECT 1 FROM sessions WHERE session_id=NEW.session_id AND source_kind='hook' AND state IN ('opening','active')))
 OR (NEW.origin_kind='explicit_set' AND NEW.persistence_kind='saved_snapshot' AND NOT EXISTS(SELECT 1 FROM sessions WHERE session_id=NEW.session_id AND game_id IS NOT NULL))
BEGIN SELECT RAISE(ABORT,'invalid_source_selection_set'); END;

CREATE TRIGGER source_selection_member_building_insert BEFORE INSERT ON source_selection_members
WHEN NOT EXISTS(
 SELECT 1 FROM source_selection_sets s JOIN source_selections old
 ON old.session_id=s.session_id AND old.selection_revision=NEW.selection_revision
 WHERE s.session_id=NEW.session_id AND s.selection_set_revision=NEW.selection_set_revision
 AND s.state='building' AND NEW.member_ordinal<s.member_count
 AND old.source_id=NEW.source_id AND old.persistence_kind=s.persistence_kind
)
BEGIN SELECT RAISE(ABORT,'invalid_source_selection_member'); END;

CREATE TRIGGER source_selection_set_seal BEFORE UPDATE ON source_selection_sets
WHEN OLD.state<>'building' OR NEW.state<>'sealed'
 OR NEW.session_id IS NOT OLD.session_id
 OR NEW.selection_set_revision IS NOT OLD.selection_set_revision
 OR NEW.operation_id IS NOT OLD.operation_id
 OR NEW.origin_kind IS NOT OLD.origin_kind
 OR NEW.persistence_kind IS NOT OLD.persistence_kind
 OR NEW.match_snapshot_json IS NOT OLD.match_snapshot_json
 OR NEW.selected_at_ms IS NOT OLD.selected_at_ms
 OR NEW.member_count IS NOT OLD.member_count
 OR NEW.member_count<>(SELECT count(*) FROM source_selection_members m WHERE m.session_id=OLD.session_id AND m.selection_set_revision=OLD.selection_set_revision)
 OR (SELECT min(member_ordinal) FROM source_selection_members m WHERE m.session_id=OLD.session_id AND m.selection_set_revision=OLD.selection_set_revision)<>0
 OR (SELECT max(member_ordinal) FROM source_selection_members m WHERE m.session_id=OLD.session_id AND m.selection_set_revision=OLD.selection_set_revision)<>NEW.member_count-1
BEGIN SELECT RAISE(ABORT,'invalid_source_selection_set_seal'); END;

CREATE TRIGGER immutable_source_selection_members_update BEFORE UPDATE ON source_selection_members
BEGIN SELECT RAISE(ABORT,'immutable_source_selection_members'); END;
CREATE TRIGGER immutable_source_selection_members_delete BEFORE DELETE ON source_selection_members
BEGIN SELECT RAISE(ABORT,'immutable_source_selection_members'); END;
CREATE TRIGGER immutable_source_selection_sets_delete BEFORE DELETE ON source_selection_sets
BEGIN SELECT RAISE(ABORT,'immutable_source_selection_sets'); END;

-- Preserve every old per-source selection and its original FK/trigger relationships.
-- No fake operation or portable matching proof is invented for historical rows.
INSERT INTO source_selection_sets(session_id,selection_set_revision,operation_id,origin_kind,persistence_kind,match_snapshot_json,selected_at_ms,member_count,state)
 SELECT session_id,selection_revision,NULL,'legacy_singleton',persistence_kind,match_snapshot_json,selected_at_ms,1,'building'
 FROM source_selections ORDER BY session_id,selection_revision;
INSERT INTO source_selection_members(session_id,selection_set_revision,source_id,selection_revision,member_ordinal)
 SELECT session_id,selection_revision,source_id,selection_revision,0 FROM source_selections ORDER BY session_id,selection_revision;
UPDATE source_selection_sets SET state='sealed' WHERE origin_kind='legacy_singleton' AND state='building';

-- Capture consumption and both automatic-task authority paths use the SAME membership predicate.
-- A later building set is invisible. There is no cross-source concatenation or max(member_revision) shortcut.
CREATE VIEW current_source_selection_members AS
 SELECT s.session_id,s.selection_set_revision,m.source_id,m.selection_revision,m.member_ordinal,s.persistence_kind,s.selected_at_ms
 FROM source_selection_sets s JOIN source_selection_members m
 ON m.session_id=s.session_id AND m.selection_set_revision=s.selection_set_revision
 WHERE s.state='sealed' AND NOT EXISTS(
   SELECT 1 FROM source_selection_sets newer WHERE newer.session_id=s.session_id
   AND newer.state='sealed' AND newer.selection_set_revision>s.selection_set_revision
 );
