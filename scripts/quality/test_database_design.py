"""Execute architecture DDL against isolated SQLite databases, never a product DB.

These checks establish schema-design properties only. They do not establish a
working repository adapter, migration, OS credential vault, or packaged product.
"""
from __future__ import annotations

import hashlib
from pathlib import Path
import sqlite3
import shutil

from database_design import BASE, OUTPUTS, SOURCES, build_metadata, generated_outputs, validate_design, _authorizer
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
DDL = ROOT / "docs" / "architecture" / "database"


def digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


class DatabaseDesignTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.addCleanup(self.db.close)
        self.db.row_factory = sqlite3.Row
        self.db.set_authorizer(_authorizer)
        self.db.execute("PRAGMA foreign_keys=ON")
        for name in ("profile.sql", "config.sql"):
            self.db.executescript((DDL / name).read_text(encoding="utf-8"))
        self.insert("profile_meta", singleton=1, profile_id="test-profile",
                    commit_revision=0, runtime_generation=1,
                    server_instance_id="test-server", runtime_state="active",
                    created_at_ms=1, updated_at_ms=1)

    def insert(self, table, **fields):
        # All identifiers originate in this test module; values stay parameterized.
        columns = ",".join(f'"{column}"' for column in fields)
        markers = ",".join("?" for _ in fields)
        return self.db.execute(f'INSERT INTO "{table}" ({columns}) VALUES ({markers})',
                               tuple(fields.values()))

    def rejected(self, statement, parameters=()):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(statement, parameters)

    def seed_resource(self, key, kind):
        self.insert("resource_snapshots", resource_snapshot_id=key, kind=kind,
                    version_label="synthetic-test-resource", manifest_sha256=digest(key),
                    contract_version="test-contract-1", metadata_json="{}", recorded_at_ms=1)

    def seed_unit(self, key="u1", text="図書館へ。", hook=False):
        session = f"s-{key}"
        fields = dict(session_id=session, source_kind="manual", state="active", started_at_ms=1)
        selection = None
        if hook:
            resource = f"core-{key}"
            self.seed_resource(resource, "core")
            fields.update(source_kind="hook", environment_key="test-bottle",
                          target_exe_path="C:/test/game.exe", windows_pid=123,
                          process_created_u64="18446744073709551615", architecture="x86",
                          broker_instance_id=f"broker-{key}", core_resource_snapshot_id=resource,
                          target_identity_sha256=digest(f"target-{key}"))
        self.insert("sessions", **fields)
        if hook:
            self.insert("capture_sources", source_id=f"src-{key}", session_id=session,
                        host_instance_id=f"host-{key}", hook_address_u64="18446744073709551615",
                        context_u64="0", subcontext_u64="0", hook_code="test-hook",
                        hook_name="synthetic", encoding="UTF-8",
                        stream_identity_sha256=digest(f"stream-{key}"),
                        identity_extensions_json="{}", discovered_at_ms=1)
            self.insert("source_selections", session_id=session, selection_revision=1,
                        source_id=f"src-{key}", persistence_kind="temporary",
                        match_snapshot_json="{}", selected_at_ms=1)
            selection = 1
        self.insert("reading_units", unit_id=key, session_id=session,
                    unit_ordinal=1, state="finalized", created_at_ms=1)
        self.insert("reading_unit_revisions", unit_id=key, text_revision=1,
                    session_id=session, source_text=text,
                    source_byte_length=len(text.encode("utf-8")), source_sha256=digest(text),
                    source_language="ja", processing_revision="test-processing-1",
                    processing_snapshot_json="{}", selection_revision=selection, created_at_ms=1)
        return dict(unit_id=key, text_revision=1, session_id=session, text=text)

    def seed_credential(self, key="key1"):
        self.insert("credential_refs", credential_id=key, vault_ref=f"test-vault-ref:{key}",
                    masked_label="test-key-ending", auth_kind="api_key", state="active",
                    created_at_ms=1, activated_at_ms=2, lifecycle_revision=0)
        return key

    def seed_config(self, key="cfg1", purpose="translation", authenticated=False, seal=True,
                    engine_enabled=True):
        self.insert("config_revisions", revision_id=key, content_hash=digest(key),
                    created_at_ms=1, save_reason="create", local_validation_json="{}")
        credential = None
        pool = None
        if authenticated:
            credential = self.seed_credential(f"key-{key}")
            pool = f"pool-{key}"
            self.insert("credential_pool_revisions", pool_id=pool, pool_revision=1,
                        policy="selected", policy_metadata_json="{}", created_at_ms=1,
                        content_hash=digest(pool))
            self.insert("credential_pool_members", pool_id=pool, pool_revision=1,
                        member_id="member-1", credential_id=credential, position=0, enabled=1, weight=1)
            self.insert("credential_pool_seals", pool_id=pool, pool_revision=1,
                        content_hash=digest(pool), sealed_at_ms=2)
        connection = f"connection-{key}"
        model = f"model-{key}"
        binding = f"binding-{key}"
        engine = f"engine-{key}"
        self.insert("connections", revision_id=key, connection_id=connection,
                    display_name="Synthetic service", protocol="openai_compatible",
                    deployment="local", auth_mode="api_key" if authenticated else "none",
                    endpoint="http://127.0.0.1:1/v1", credential_pool_id=pool,
                    credential_pool_revision=1 if pool else None, availability_intent=1)
        self.insert("models", revision_id=key, model_id=model, connection_id=connection,
                    actual_model_id="synthetic-model", display_name="Test model")
        self.insert("purpose_bindings", revision_id=key, binding_id=binding,
                    scope_key="global", purpose=purpose, enabled=1,
                    output_mode="text" if purpose == "translation" else "plain_text",
                    source_language="ja", target_language="zh")
        self.insert("binding_engines", revision_id=key, binding_id=binding,
                    engine_id=engine, purpose=purpose, model_id=model, position=0,
                    enabled=int(engine_enabled))
        self.insert("model_parameters", revision_id=key, model_id=model,
                    parameter_key="temperature", mode="set", value_json="0")
        if seal:
            self.insert("config_revision_seals", revision_id=key, content_hash=digest(key), sealed_at_ms=2)
        return dict(revision_id=key, connection_id=connection, model_id=model,
                    binding_id=binding, engine_id=engine, purpose=purpose,
                    credential_id=credential, pool_id=pool)

    def seed_request(self, key="task1", *, unit=None, config=None, authenticated=False):
        unit = unit or self.seed_unit(f"u-{key}")
        config = config or self.seed_config(f"cfg-{key}", authenticated=authenticated)
        if not self.db.execute("SELECT 1 FROM purpose_runtime_states WHERE scope_key='global' AND purpose=?",
                               (config["purpose"],)).fetchone():
            self.insert("purpose_runtime_states", scope_key="global", purpose=config["purpose"],
                        binding_id=config["binding_id"], config_revision_id=config["revision_id"],
                        is_enabled=1, authorization_epoch=1,
                        config_semantics_sha256=digest("effective-config"), updated_at_ms=2)
        if not self.db.execute("SELECT 1 FROM connection_runtime_states WHERE connection_id=?",
                               (config["connection_id"],)).fetchone():
            self.insert("connection_runtime_states", connection_id=config["connection_id"],
                        config_revision_id=config["revision_id"], is_suspended=0,
                        control_revision=1, updated_at_ms=2)
        self.insert("request_tasks", task_id=key, unit_id=unit["unit_id"],
                    text_revision=unit["text_revision"], session_id=unit["session_id"],
                    scope_key="global", purpose=config["purpose"],
                    config_revision_id=config["revision_id"], binding_id=config["binding_id"],
                    engine_id=config["engine_id"], profile_generation=1,
                    server_instance_id="test-server", authorization_epoch=1,
                    request_semantics_sha256=digest(key), input_snapshot_json="{}",
                    state="queued", created_at_ms=3)
        self.insert("request_attempts", attempt_id=f"attempt-{key}", task_id=key,
                    attempt_number=1, config_revision_id=config["revision_id"],
                    binding_id=config["binding_id"], purpose=config["purpose"],
                    actual_engine_id=config["engine_id"], credential_id=config["credential_id"],
                    dispatch_kind="initial", state="started", usage_json='{"availability":"unknown"}',
                    started_at_ms=4)
        return dict(task_id=key, attempt_id=f"attempt-{key}", config=config, unit=unit)

    def complete_request(self, request):
        self.db.execute("UPDATE request_tasks SET state='running' WHERE task_id=?", (request["task_id"],))
        self.db.execute("UPDATE request_attempts SET state='completed',finished_at_ms=5 WHERE attempt_id=?",
                        (request["attempt_id"],))
        self.db.execute("UPDATE request_tasks SET state='completed',finished_at_ms=5 WHERE task_id=?",
                        (request["task_id"],))

    def insert_translation(self, request, key="translation1", text="去图书馆。"):
        self.insert("translations", translation_id=key, task_id=request["task_id"],
                    attempt_id=request["attempt_id"], translated_text=text,
                    target_language="zh", provider_label_snapshot="Synthetic service", created_at_ms=6)

    def insert_gc_receipt(self, operation):
        self.db.execute("UPDATE profile_meta SET commit_revision=commit_revision+1 WHERE singleton=1")
        revision = self.db.execute("SELECT commit_revision FROM profile_meta WHERE singleton=1").fetchone()[0]
        self.insert("operation_receipts", operation_id=operation, request_sha256=digest(operation),
                    command_kind="retention_gc", commit_revision=revision,
                    result_refs_json="{}", committed_at_ms=10)

    def test_strict_types_and_nonnull_identity(self):
        self.rejected("INSERT INTO games(game_id,display_name,row_revision,created_at_ms,updated_at_ms) VALUES(NULL,'x',1,1,1)")
        self.rejected("INSERT INTO games(game_id,display_name,row_revision,created_at_ms,updated_at_ms) VALUES('g','x',1.5,1,1)")
        self.assertEqual(1, self.db.execute("PRAGMA foreign_keys").fetchone()[0])

    def test_json_shapes_and_lowercase_hash(self):
        for bad_hash, bad_json in (("A" * 64, "{}"), (digest("x"), "[]"), (digest("x"), "broken")):
            with self.subTest(hash=bad_hash, validation=bad_json):
                self.rejected("INSERT INTO config_revisions VALUES('bad',NULL,?,1,'create',?)", (bad_hash, bad_json))
        self.assertEqual(0, self.db.execute("SELECT count(*) FROM config_revisions").fetchone()[0])

    def test_incomplete_draft_is_saveable_without_network_or_fake_model(self):
        config = self.seed_config(seal=False)
        # A new draft may omit the endpoint and actual model; it is not a runnable promise.
        self.insert("connections", revision_id=config["revision_id"], connection_id="unfinished",
                    display_name="", protocol="unsupported", deployment="remote", auth_mode="api_key")
        self.insert("models", revision_id=config["revision_id"], model_id="unfinished-model",
                    connection_id="unfinished", display_name="")
        row = self.db.execute("SELECT endpoint,actual_model_id FROM connections JOIN models USING(revision_id,connection_id) WHERE model_id='unfinished-model'").fetchone()
        self.assertEqual((None, None), tuple(row))

    def test_model_cannot_reference_connection_from_another_revision(self):
        self.seed_config(seal=False)
        self.insert("config_revisions", revision_id="cfg2", content_hash=digest("cfg2"),
                    created_at_ms=2, save_reason="edit", local_validation_json="{}")
        self.rejected("INSERT INTO models(revision_id,model_id,connection_id,display_name) VALUES('cfg2','m2','connection-cfg1','x')")

    def test_explicit_scope_prevents_duplicate_global_and_mismatched_game(self):
        self.seed_config(seal=False)
        self.rejected("INSERT INTO purpose_bindings(revision_id,binding_id,scope_key,purpose) VALUES('cfg1','duplicate','global','translation')")
        self.insert("games", game_id="g", display_name="Game", row_revision=1, created_at_ms=1, updated_at_ms=1)
        self.rejected("INSERT INTO purpose_bindings(revision_id,binding_id,scope_key,game_id,purpose) VALUES('cfg1','wrong','game:other','g','grammar')")
        self.insert("purpose_bindings", revision_id="cfg1", binding_id="game-grammar",
                    scope_key="game:g", game_id="g", purpose="grammar", enabled=0)

    def test_learning_has_single_engine_while_translation_can_have_multiple(self):
        translation = self.seed_config("translation", seal=False)
        self.insert("binding_engines", revision_id="translation", binding_id=translation["binding_id"],
                    engine_id="second", purpose="translation", model_id=translation["model_id"], position=1)
        grammar = self.seed_config("grammar", purpose="grammar", seal=False)
        self.rejected("INSERT INTO binding_engines(revision_id,binding_id,engine_id,purpose,model_id,position) VALUES(?,?,?,?,?,1)",
                      ("grammar", grammar["binding_id"], "second", "grammar", grammar["model_id"]))

    def test_parameters_keep_explicit_zero_distinct_from_omit_and_inherit(self):
        config = self.seed_config(seal=False)
        self.insert("binding_parameters", revision_id="cfg1", binding_id=config["binding_id"],
                    parameter_key="temperature", mode="omit", value_json=None)
        self.insert("engine_parameters", revision_id="cfg1", binding_id=config["binding_id"],
                    engine_id=config["engine_id"], parameter_key="temperature", mode="inherit", value_json=None)
        self.assertEqual("0", self.db.execute("SELECT value_json FROM model_parameters").fetchone()[0])
        self.rejected("INSERT INTO binding_parameters VALUES('cfg1',?,'top_p','omit','0')", (config["binding_id"],))
        self.rejected("INSERT INTO binding_parameters VALUES('cfg1',?,'top_p','set',NULL)", (config["binding_id"],))

    def test_config_head_cas_rejects_stale_writer_and_nonmonotonic_revision(self):
        self.seed_config()
        self.insert("config_heads", head_kind="draft", revision_id="cfg1", head_revision=0, updated_at_ms=2)
        first = self.db.execute("UPDATE config_heads SET head_revision=1,updated_at_ms=3 WHERE head_kind='draft' AND head_revision=0")
        stale = self.db.execute("UPDATE config_heads SET head_revision=1,updated_at_ms=3 WHERE head_kind='draft' AND head_revision=0")
        self.assertEqual((1, 0), (first.rowcount, stale.rowcount))
        self.rejected("UPDATE config_heads SET head_revision=4 WHERE head_kind='draft'")

    def test_sealed_config_blocks_late_insert_delete_and_update(self):
        config = self.seed_config()
        self.rejected("INSERT INTO model_parameters VALUES('cfg1',?,'top_p','set','0.8')", (config["model_id"],))
        self.rejected("DELETE FROM model_parameters WHERE revision_id='cfg1'")
        self.rejected("UPDATE models SET display_name='changed' WHERE revision_id='cfg1'")
        self.assertEqual(1, self.db.execute("SELECT count(*) FROM model_parameters").fetchone()[0])

    def test_sealed_credential_pool_cannot_be_changed_in_place(self):
        config = self.seed_config(authenticated=True)
        extra = self.seed_credential("extra-key")
        self.rejected("INSERT INTO credential_pool_members VALUES(?,1,'extra',?,1,1,1)", (config["pool_id"], extra))
        self.rejected("DELETE FROM credential_pool_members WHERE pool_id=?", (config["pool_id"],))
        self.rejected("UPDATE credential_pool_revisions SET policy='round_robin' WHERE pool_id=?", (config["pool_id"],))

    def test_applied_head_cannot_point_to_unsealed_building_revision(self):
        self.seed_config(seal=False)
        self.rejected("INSERT INTO config_heads VALUES('applied','cfg1',1,2)")
        self.insert("config_revision_seals", revision_id="cfg1", content_hash=digest("cfg1"), sealed_at_ms=2)
        self.insert("config_heads", head_kind="applied", revision_id="cfg1", head_revision=1, updated_at_ms=2)

    def test_gc_mark_rejects_config_with_head_or_task_reference(self):
        config = self.seed_config()
        self.insert("config_heads", head_kind="applied", revision_id="cfg1", head_revision=1, updated_at_ms=2)
        self.rejected("INSERT INTO config_gc_marks VALUES('cfg1','gc-referenced',10)")
        self.db.execute("DELETE FROM config_heads WHERE head_kind='applied'")
        self.seed_request(config=config)
        self.rejected("INSERT INTO config_gc_marks VALUES('cfg1','gc-referenced',10)")
        self.assertEqual(0, self.db.execute("SELECT count(*) FROM config_gc_marks").fetchone()[0])

    def test_gc_requires_same_transaction_receipt_and_purged_config_never_reactivates(self):
        config = self.seed_config()
        self.db.commit()
        with self.assertRaises(sqlite3.IntegrityError), self.db:
            self.insert("config_gc_marks", revision_id="cfg1", operation_id="missing-receipt", requested_at_ms=10)
        self.assertEqual(0, self.db.execute("SELECT count(*) FROM config_gc_marks").fetchone()[0])
        with self.db:
            self.insert("config_gc_marks", revision_id="cfg1", operation_id="gc-config", requested_at_ms=10)
            self.insert_gc_receipt("gc-config")
            self.db.execute("DELETE FROM model_parameters WHERE revision_id='cfg1'")
        self.assertEqual(0, self.db.execute("SELECT count(*) FROM model_parameters").fetchone()[0])
        self.assertEqual(1, self.db.execute("SELECT count(*) FROM config_revision_seals").fetchone()[0])
        self.rejected("INSERT INTO config_heads VALUES('applied','cfg1',1,11)")
        self.rejected("INSERT INTO model_parameters VALUES('cfg1',?,'temperature','set','0')", (config["model_id"],))
        self.rejected("DELETE FROM config_gc_marks WHERE revision_id='cfg1'")
        self.rejected("DELETE FROM config_revision_seals WHERE revision_id='cfg1'")
        self.assertEqual([], self.db.execute("PRAGMA foreign_key_check").fetchall())

    def test_pool_gc_waits_for_connection_references_and_preserves_irrevocable_seal(self):
        config = self.seed_config(authenticated=True)
        self.rejected("INSERT INTO credential_pool_gc_marks VALUES(?,1,'pool-gc',10)", (config["pool_id"],))
        self.db.commit()
        with self.db:
            self.insert("config_gc_marks", revision_id="cfg1", operation_id="gc-config", requested_at_ms=10)
            self.insert_gc_receipt("gc-config")
            for table in ("model_parameters", "binding_engines", "purpose_bindings", "models", "connections"):
                self.db.execute(f'DELETE FROM "{table}" WHERE revision_id=?', ("cfg1",))
            self.insert("credential_pool_gc_marks", pool_id=config["pool_id"], pool_revision=1,
                        operation_id="gc-pool", requested_at_ms=11)
            self.insert_gc_receipt("gc-pool")
            self.db.execute("DELETE FROM credential_pool_members WHERE pool_id=?", (config["pool_id"],))
        self.assertEqual(0, self.db.execute("SELECT count(*) FROM credential_pool_members").fetchone()[0])
        self.assertEqual(1, self.db.execute("SELECT count(*) FROM credential_pool_seals").fetchone()[0])
        self.rejected("DELETE FROM credential_pool_seals WHERE pool_id=?", (config["pool_id"],))
        self.rejected("INSERT INTO credential_pool_members VALUES(?,1,'resurrected',?,0,1,1)",
                      (config["pool_id"], config["credential_id"]))
        self.assertEqual([], self.db.execute("PRAGMA foreign_key_check").fetchall())

    def test_uint64_identity_keeps_full_value_and_rejects_noncanonical_forms(self):
        unit = self.seed_unit(hook=True)
        self.assertEqual("18446744073709551615", self.db.execute("SELECT process_created_u64 FROM sessions WHERE session_id=?", (unit["session_id"],)).fetchone()[0])
        source = dict(self.db.execute("SELECT * FROM capture_sources").fetchone())
        for index, invalid in enumerate(("18446744073709551616", "01", "-1", "1e3", "")):
            with self.subTest(invalid=invalid), self.assertRaises(sqlite3.IntegrityError):
                self.insert("capture_sources", **(source | dict(source_id=f"bad-{index}", hook_address_u64=invalid,
                                                               stream_identity_sha256=digest(invalid))))

    def test_tokens_match_utf8_slice_and_do_not_overlap(self):
        unit = self.seed_unit(text="図😀書")
        self.seed_resource("language", "language")
        self.insert("lexical_snapshots", lexical_id="lex", unit_id=unit["unit_id"], text_revision=1,
                    source_sha256=digest(unit["text"]), language_resource_snapshot_id="language",
                    token_schema_version="test-schema", analysis_revision="1", token_count=3, created_at_ms=2)
        def token(index, start, end, surface):
            self.insert("lexical_tokens", lexical_id="lex", token_index=index, byte_start=start,
                        byte_end=end, surface=surface, lemma=surface, pos_major="test",
                        features_json="{}", token_kind="word", is_known=1)
        token(0, 0, 3, "図")
        token(1, 3, 7, "😀")
        with self.assertRaises(sqlite3.IntegrityError):
            token(2, 7, 10, "図")
        with self.assertRaises(sqlite3.IntegrityError):
            token(2, 0, 3, "図")
        token(2, 7, 10, "書")
        reconstructed = "".join(row[0] for row in self.db.execute("SELECT surface FROM lexical_tokens ORDER BY token_index"))
        self.assertEqual(unit["text"], reconstructed)

    def test_lexical_snapshot_cannot_claim_different_source_hash(self):
        unit = self.seed_unit()
        self.seed_resource("language", "language")
        self.rejected("INSERT INTO lexical_snapshots VALUES('lex',?,1,?,'language','schema','1',0,2)",
                      (unit["unit_id"], digest("another source")))

    def test_raw_payload_retention_does_not_delete_reading_or_provenance(self):
        unit = self.seed_unit(hook=True)
        payload = unit["text"].encode("utf-8")
        self.insert("raw_events", event_id="event", session_id=unit["session_id"], source_id=f"src-{unit['unit_id']}",
                    wire_sequence_u64="18446744073709551615", ingest_sequence=1, event_kind="text",
                    payload_byte_length=len(payload), payload_sha256=digest(unit["text"]),
                    observed_at_ms=2, integrity_state="intact")
        self.insert("raw_event_payloads", event_id="event", payload_bytes=payload, retained_at_ms=2)
        self.insert("source_refs", unit_id=unit["unit_id"], text_revision=1, session_id=unit["session_id"],
                    ref_ordinal=0, event_id="event", byte_start=0, byte_end=len(payload))
        self.db.execute("DELETE FROM raw_event_payloads WHERE event_id='event'")
        row = self.db.execute("SELECT r.source_text,e.payload_byte_length FROM reading_unit_revisions r JOIN source_refs s USING(unit_id,text_revision) JOIN raw_events e USING(event_id)").fetchone()
        self.assertEqual((unit["text"], len(payload)), tuple(row))
        self.assertEqual([], self.db.execute("PRAGMA foreign_key_check").fetchall())

    def test_cross_session_and_manual_fake_capture_are_rejected(self):
        hook = self.seed_unit("hook", hook=True)
        manual = self.seed_unit("manual")
        source = dict(self.db.execute("SELECT * FROM capture_sources WHERE session_id=?", (hook["session_id"],)).fetchone())
        with self.assertRaises(sqlite3.IntegrityError):
            self.insert("capture_sources", **(source | dict(source_id="fake", session_id=manual["session_id"])))
        self.rejected("INSERT INTO source_selections VALUES(?,2,'src-hook','temporary','{}',2)", (manual["session_id"],))

    def test_task_identity_is_frozen_and_terminal_state_cannot_reopen(self):
        request = self.seed_request()
        self.rejected("UPDATE request_tasks SET unit_id='other' WHERE task_id=?", (request["task_id"],))
        self.rejected("UPDATE request_tasks SET state='completed',finished_at_ms=5 WHERE task_id=?", (request["task_id"],))
        self.db.execute("UPDATE request_tasks SET state='cancelled',finished_at_ms=5 WHERE task_id=?", (request["task_id"],))
        self.rejected("UPDATE request_tasks SET state='running',finished_at_ms=NULL WHERE task_id=?", (request["task_id"],))

    def test_cancelled_task_cannot_commit_translation(self):
        request = self.seed_request()
        self.db.execute("UPDATE request_tasks SET state='cancelled',finished_at_ms=5 WHERE task_id=?", (request["task_id"],))
        self.db.execute("UPDATE request_attempts SET state='completed',finished_at_ms=5 WHERE attempt_id=?", (request["attempt_id"],))
        with self.assertRaises(sqlite3.IntegrityError):
            self.insert_translation(request)
        self.assertEqual(0, self.db.execute("SELECT count(*) FROM translations").fetchone()[0])

    def test_stale_profile_generation_or_authorization_epoch_blocks_commit(self):
        request = self.seed_request()
        self.complete_request(request)
        for statement in ("UPDATE profile_meta SET runtime_generation=2 WHERE singleton=1",
                          "UPDATE purpose_runtime_states SET authorization_epoch=2 WHERE scope_key='global'"):
            with self.subTest(statement=statement):
                self.db.execute("SAVEPOINT stale_authority")
                self.db.execute(statement)
                with self.assertRaises(sqlite3.IntegrityError):
                    self.insert_translation(request)
                self.db.execute("ROLLBACK TO stale_authority")
                self.db.execute("RELEASE stale_authority")
        self.insert_translation(request)

    def test_purpose_epoch_advances_only_with_explicit_monotonic_authority_change(self):
        request = self.seed_request()
        self.rejected("UPDATE purpose_runtime_states SET authorization_epoch=0")
        self.rejected("UPDATE purpose_runtime_states SET authorization_epoch=3")
        self.rejected("UPDATE purpose_runtime_states SET is_enabled=0")
        self.rejected("UPDATE purpose_runtime_states SET config_semantics_sha256=?", (digest("changed-semantics"),))
        # A new configuration record can retain identical routing/semantics and epoch.
        old = request["config"]["revision_id"]
        new = "same-semantics-new-record"
        self.insert("config_revisions", revision_id=new, parent_revision_id=old,
                    content_hash=digest(new), created_at_ms=3, save_reason="edit", local_validation_json="{}")
        for table in ("connections", "models", "purpose_bindings", "binding_engines", "model_parameters"):
            for row in self.db.execute(f'SELECT * FROM "{table}" WHERE revision_id=?', (old,)).fetchall():
                self.insert(table, **(dict(row) | {"revision_id": new}))
        self.insert("config_revision_seals", revision_id=new, content_hash=digest(new), sealed_at_ms=4)
        self.db.execute("UPDATE purpose_runtime_states SET config_revision_id=?,updated_at_ms=4", (new,))
        self.assertEqual(1, self.db.execute("SELECT authorization_epoch FROM purpose_runtime_states").fetchone()[0])
        self.db.execute("UPDATE purpose_runtime_states SET is_enabled=0,authorization_epoch=2,updated_at_ms=5")
        self.rejected("UPDATE purpose_runtime_states SET is_enabled=1")
        self.db.execute("UPDATE purpose_runtime_states SET is_enabled=1,authorization_epoch=3,updated_at_ms=6")
        self.db.execute("UPDATE purpose_runtime_states SET authorization_epoch=4,updated_at_ms=7")
        self.assertEqual(4, self.db.execute("SELECT authorization_epoch FROM purpose_runtime_states").fetchone()[0])

    def test_immediate_connection_suspend_blocks_result_without_changing_snapshot(self):
        request = self.seed_request()
        self.complete_request(request)
        connection = request["config"]["connection_id"]
        self.db.execute("UPDATE connection_runtime_states SET is_suspended=1,control_revision=2,updated_at_ms=6 WHERE connection_id=?", (connection,))
        with self.assertRaises(sqlite3.IntegrityError):
            self.insert_translation(request)
        self.assertEqual(1, self.db.execute("SELECT availability_intent FROM connections WHERE connection_id=?", (connection,)).fetchone()[0])

    def test_disabled_engine_cannot_commit_even_when_connection_and_purpose_allow(self):
        config = self.seed_config(engine_enabled=False)
        request = self.seed_request(config=config)
        self.complete_request(request)
        self.assertEqual(0, self.db.execute("SELECT enabled FROM binding_engines").fetchone()[0])
        self.assertEqual(1, self.db.execute("SELECT is_enabled FROM purpose_runtime_states").fetchone()[0])
        self.assertEqual(0, self.db.execute("SELECT is_suspended FROM connection_runtime_states").fetchone()[0])
        with self.assertRaises(sqlite3.IntegrityError):
            self.insert_translation(request)

    def test_revoked_secret_cannot_authorize_result_or_be_reactivated(self):
        request = self.seed_request(authenticated=True)
        self.complete_request(request)
        credential = request["config"]["credential_id"]
        self.db.execute("UPDATE credential_refs SET state='revoked',revoked_at_ms=6,lifecycle_revision=1 WHERE credential_id=?", (credential,))
        with self.assertRaises(sqlite3.IntegrityError):
            self.insert_translation(request)
        self.rejected("UPDATE credential_refs SET state='active',revoked_at_ms=NULL,lifecycle_revision=2 WHERE credential_id=?", (credential,))

    def test_manual_correction_is_independent_of_later_machine_result(self):
        first = self.seed_request()
        self.complete_request(first)
        self.insert_translation(first, "first", "初译")
        self.insert("corrections", correction_id="correction", translation_id="first", correction_revision=1,
                    corrected_text="人工修正", base_revision=0, created_at_ms=7)
        second = self.seed_request("second", unit=first["unit"], config=first["config"])
        self.complete_request(second)
        self.insert_translation(second, "second", "后到机器译文")
        self.assertEqual("人工修正", self.db.execute("SELECT corrected_text FROM corrections").fetchone()[0])
        self.rejected("UPDATE corrections SET corrected_text='overwrite' WHERE correction_id='correction'")
        self.assertEqual(2, self.db.execute("SELECT count(*) FROM translations").fetchone()[0])

    def test_failed_business_transaction_leaves_no_half_history(self):
        self.db.commit()
        with self.assertRaises(sqlite3.IntegrityError), self.db:
            self.seed_unit()
            self.insert("reading_units", unit_id="duplicate-order", session_id="s-u1",
                        unit_ordinal=1, state="finalized", created_at_ms=2)
        self.assertEqual(0, self.db.execute("SELECT count(*) FROM sessions").fetchone()[0])
        self.assertEqual(0, self.db.execute("SELECT count(*) FROM reading_unit_revisions").fetchone()[0])

    def test_configuration_test_result_is_separate_from_body_history(self):
        config = self.seed_config()
        self.insert("test_records", test_id="test", config_revision_id=config["revision_id"],
                    connection_id=config["connection_id"], model_id=config["model_id"],
                    binding_id=config["binding_id"], purpose=config["purpose"], stage="purpose",
                    request_semantics_hash=digest("test"), input_scope="sample", state="passed",
                    started_at_ms=2, completed_at_ms=3)
        self.rejected("UPDATE test_records SET state='failed' WHERE test_id='test'")
        self.assertEqual(1, self.db.execute("SELECT count(*) FROM test_records").fetchone()[0])
        self.assertEqual(0, self.db.execute("SELECT count(*) FROM reading_units").fetchone()[0])
        self.assertEqual(0, self.db.execute("SELECT count(*) FROM translations").fetchone()[0])

    def seed_dictionary(self, database):
        database.set_authorizer(_authorizer)
        database.executescript((DDL / "dictionary.sql").read_text(encoding="utf-8"))
        database.execute("INSERT INTO dictionary_meta VALUES(1,'synthetic',?,1,'test-normalization',1)", (digest("dictionary"),))
        database.execute("INSERT INTO dictionary_sources VALUES('source','Synthetic source','test',?,'https://example.invalid/test','test-license','Synthetic fixture')", (digest("source"),))
        database.execute("INSERT INTO dictionary_entries VALUES('entry','source','key','図書館','としょかん','[]','zh_base')")
        database.execute("INSERT INTO dictionary_forms VALUES('form','entry','図書館','図書館','lemma',0)")
        database.execute("INSERT INTO dictionary_senses VALUES('entry',0,'zh','图书馆',NULL,'test:entry')")

    def test_packaged_dictionary_can_be_opened_readonly_and_not_modified(self):
        with tempfile.TemporaryDirectory(prefix="luna-dictionary-design-") as directory:
            path = Path(directory) / "dictionary.sqlite3"
            with sqlite3.connect(path) as writable:
                self.seed_dictionary(writable)
            readonly = sqlite3.connect(f"{path.as_uri()}?mode=ro", uri=True)
            try:
                result = readonly.execute("SELECT gloss_text FROM dictionary_forms JOIN dictionary_senses USING(entry_id) WHERE normalized_key=?", ("図書館",)).fetchone()
                self.assertEqual("图书馆", result[0])
                with self.assertRaises(sqlite3.OperationalError):
                    readonly.execute("UPDATE dictionary_senses SET gloss_text='overwrite'")
            finally:
                readonly.close()

    def test_dictionary_english_supplement_cannot_masquerade_as_chinese(self):
        dictionary = sqlite3.connect(":memory:")
        self.addCleanup(dictionary.close)
        self.seed_dictionary(dictionary)
        with self.assertRaises(sqlite3.IntegrityError):
            dictionary.execute("INSERT INTO dictionary_senses VALUES('entry',1,'en','library',NULL,'test:entry')")
        dictionary.execute("INSERT INTO dictionary_entries VALUES('english','source','english-key','図書館','としょかん','[]','en_supplement')")
        dictionary.execute("INSERT INTO dictionary_senses VALUES('english',0,'en','library',NULL,'test:entry')")
        with self.assertRaises(sqlite3.IntegrityError):
            dictionary.execute("INSERT INTO dictionary_senses VALUES('english',1,'zh','图书馆',NULL,'test:entry')")


    def test_all_design_test_connections_reject_file_escape(self):
        with tempfile.TemporaryDirectory(prefix="database-design-trap-") as folder:
            for statement in ("ATTACH DATABASE ? AS external_db", "VACUUM INTO ?"):
                with self.subTest(statement=statement):
                    target = Path(folder) / "must-not-create.sqlite3"
                    self.db.commit()
                    with self.assertRaises(sqlite3.DatabaseError):
                        self.db.execute(statement, (str(target),))
                    self.assertFalse(target.exists())
            dictionary = sqlite3.connect(":memory:")
            self.addCleanup(dictionary.close)
            self.seed_dictionary(dictionary)
            target = Path(folder) / "dictionary-must-not-create.sqlite3"
            with self.assertRaises(sqlite3.DatabaseError):
                dictionary.execute("ATTACH DATABASE ? AS external_db", (str(target),))
            self.assertFalse(target.exists())

    def design_copy(self):
        folder = tempfile.TemporaryDirectory(prefix="database-design-export-")
        self.addCleanup(folder.cleanup)
        root = Path(folder.name)
        paths = [path for values in SOURCES.values() for path in values]
        paths += [f"{BASE}/{name}" for name in OUTPUTS]
        paths += ["docs/product/first-product-scope.md", "docs/architecture/engineering-modules.csv"]
        for name in paths:
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, target)
        return root

    def test_design_exports_are_deterministic_and_environment_independent(self):
        outputs1, meta1, errors1 = generated_outputs(ROOT)
        outputs2, meta2, errors2 = generated_outputs(ROOT)
        self.assertEqual([], errors1)
        self.assertEqual([], errors2)
        self.assertEqual(outputs1, outputs2)
        self.assertEqual(meta1, meta2)
        self.assertNotIn("sqlite_version", meta1)
        self.assertFalse(meta1["product_tests_executed"])
        self.assertEqual([], validate_design(ROOT))

    def test_export_guard_rejects_field_dictionary_drift(self):
        root = self.design_copy()
        field = root / BASE / "field-dictionary.csv"
        field.write_text(field.read_text().replace("database,table", "changed,table", 1))
        errors = validate_design(root)
        self.assertTrue(any("field-dictionary.csv" in error and "differs" in error for error in errors), errors)

    def test_export_guard_requires_field_annotations(self):
        root = self.design_copy()
        ddl = root / BASE / "profile.sql"
        ddl.write_text(ddl.read_text().replace("-- 用户显示名，不作为目标身份", "--", 1))
        _, errors = build_metadata(root)
        self.assertTrue(any("display_name" in error and "description" in error for error in errors), errors)

    def test_design_guard_compiles_foreign_key_parents(self):
        root = self.design_copy()
        ddl = root / BASE / "profile.sql"
        ddl.write_text(ddl.read_text().replace("REFERENCES sessions(session_id)", "REFERENCES absent_parent(session_id)", 1))
        _, errors = build_metadata(root)
        self.assertTrue(any("absent_parent" in error for error in errors), errors)

    def test_design_guard_cannot_attach_file_database(self):
        root = self.design_copy()
        ddl = root / BASE / "profile.sql"
        external = root / "must-not-create.sqlite3"
        ddl.write_text(ddl.read_text()+"\nATTACH DATABASE '"+str(external)+"' AS external_db;\n")
        _, errors = build_metadata(root)
        self.assertTrue(any("not authorized" in error for error in errors), errors)
        self.assertFalse(external.exists())


if __name__ == "__main__":
    unittest.main()
