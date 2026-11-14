#[allow(dead_code)]
#[path = "storage_acceptance_support/evidence.rs"]
mod acceptance_evidence;
use app_core::{contracts::*, domain::*, ports::ProductRepository};
use app_infra::storage::SqliteStore;
use std::{collections::BTreeMap, path::PathBuf};
fn temp(name: &str) -> PathBuf {
    if let Some(output) = std::env::var_os("FP_STORAGE_OUTPUT") {
        let directory = PathBuf::from(output)
            .join("components")
            .join("product-data")
            .join(format!(
                "{}-{name}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&directory).unwrap();
        return directory.join("profile.sqlite3");
    }
    let p = std::env::temp_dir().join(format!("luna-storage-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p.join(format!(
        "{}.sqlite",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
fn store(name: &str) -> SqliteStore {
    SqliteStore::open(temp(name), "profile-test", "server-test", 10).unwrap()
}
fn aggregation_store(name: &str) -> (SqliteStore, PathBuf) {
    let path = temp(name);
    let store = SqliteStore::open(&path, "profile-test", "server-test", 10).unwrap();
    hook(&store, None);
    store
        .receive_hook_text(event("1", "candidate"), 31)
        .unwrap();
    command(
        &store,
        "aggregate-select",
        Command::SelectSource {
            source_id: "source-1".into(),
        },
    );
    (store, path)
}
fn inspect_capture(path: &std::path::Path, sql: &str) -> i64 {
    rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap()
        .query_row(sql, [], |r| r.get(0))
        .unwrap()
}
#[test]
fn capture_fragments_have_immutable_revisions_and_ordered_raw_provenance() {
    let (store, path) = aggregation_store("aggregate-provenance");
    let first = store
        .ingest_hook_text(event("2", "静かな😀"), 1000)
        .unwrap();
    let first = first.visible_version.unwrap();
    assert!(store.snapshot().unwrap().tasks.is_empty());
    let final_outcome = store
        .ingest_hook_text(event("3", "朝だった。"), 2307)
        .unwrap();
    let last = final_outcome.visible_version.unwrap();
    assert_eq!(first.unit_id, last.unit_id);
    assert_eq!(last.text_revision, 2);
    assert_eq!(final_outcome.finalized_versions, vec![last.clone()]);
    assert_eq!(
        store.unit(last.unit_id.clone(), 1).unwrap().source_text,
        "静かな😀"
    );
    assert_eq!(
        store.unit(last.unit_id.clone(), 2).unwrap().source_text,
        "静かな😀朝だった。"
    );
    assert_eq!(store.snapshot().unwrap().history.units.len(), 1);
    let history = store
        .history_page(HistoryQuery {
            filter: HistoryFilter::default(),
            cursor: None,
            limit: 100,
        })
        .unwrap();
    assert_eq!(history.units.len(), 1);
    assert_eq!(history.units[0].text_revision, 2);
    let conn =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let spans:Vec<Vec<u8>>=conn.prepare("SELECT substr(p.payload_bytes,f.byte_start+1,f.byte_end-f.byte_start) FROM source_refs f JOIN raw_event_payloads p USING(event_id) WHERE f.unit_id=?1 AND f.text_revision=2 ORDER BY f.ref_ordinal").unwrap().query_map([&last.unit_id],|r|r.get(0)).unwrap().collect::<Result<_,_>>().unwrap();
    assert_eq!(
        String::from_utf8(spans.concat()).unwrap(),
        history.units[0].source_text
    );
    assert_eq!(spans.len(), 2);
    assert_eq!(inspect_capture(&path, "SELECT count(*) FROM raw_events"), 3);
    assert_eq!(
        inspect_capture(
            &path,
            "SELECT count(*) FROM reading_units WHERE state='open'"
        ),
        0
    );
    acceptance_evidence::record(
        "unicode-multiline-refs",
        &[("STORAGE-21", 1)],
        serde_json::json!({"actual_test":"capture_fragments_have_immutable_revisions_and_ordered_raw_provenance"}),
    );
}
#[test]
fn capture_v2_slow_fragment_survives_idle_and_keeps_exact_provenance() {
    let (store, path) = aggregation_store("aggregate-v2-slow");
    let first = store
        .ingest_hook_text(event("2", "静かな😀朝の"), 1000)
        .unwrap()
        .visible_version
        .unwrap();
    let idle = store
        .flush_hook_capture(
            CaptureFlush {
                session_id: Some("hook-session".into()),
                reason: CaptureFlushReason::Due,
            },
            2501,
        )
        .unwrap();
    assert!(
        idle.is_empty(),
        "incomplete source must remain open after the idle boundary"
    );
    assert_eq!(
        inspect_capture(
            &path,
            "SELECT count(*) FROM reading_units WHERE state='open'"
        ),
        1
    );
    assert!(store.snapshot().unwrap().tasks.is_empty());
    let done = store
        .ingest_hook_text(event("3", "物語だった。"), 3584)
        .unwrap();
    let latest = done.visible_version.unwrap();
    assert_eq!(latest.unit_id, first.unit_id);
    assert_eq!(latest.text_revision, 2);
    assert_eq!(done.finalized_versions, vec![latest.clone()]);
    assert_eq!(
        store.unit(first.unit_id.clone(), 1).unwrap().source_text,
        "静かな😀朝の"
    );
    assert_eq!(
        store.unit(first.unit_id, 2).unwrap().source_text,
        "静かな😀朝の物語だった。"
    );
    assert_eq!(store.snapshot().unwrap().history.units.len(), 1);
    let conn =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let spans: Vec<Vec<u8>> = conn.prepare("SELECT substr(p.payload_bytes,f.byte_start+1,f.byte_end-f.byte_start) FROM source_refs f JOIN raw_event_payloads p USING(event_id) WHERE f.unit_id=?1 AND f.text_revision=2 ORDER BY f.ref_ordinal")
        .unwrap().query_map([&latest.unit_id], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(spans.len(), 2);
    assert_eq!(
        String::from_utf8(spans.concat()).unwrap(),
        "静かな😀朝の物語だった。"
    );
    assert!(
        store
            .flush_hook_capture(
                CaptureFlush {
                    session_id: None,
                    reason: CaptureFlushReason::Due
                },
                9000
            )
            .unwrap()
            .is_empty()
    );
}
fn assert_quoted_prefix_growth(parts: &[&str], prefix: &str, incoming: &str, tail: &str) {
    let (store, path) = aggregation_store(&format!("quoted-v3-{}", content_hash(incoming)));
    let mut old = None;
    for (index, part) in parts.iter().enumerate() {
        let out = store
            .ingest_hook_text(
                event(&(index + 2).to_string(), part),
                1000 + index as i64 * 100,
            )
            .unwrap();
        assert!(out.finalized_versions.is_empty());
        old = out.visible_version;
    }
    let old = old.unwrap();
    let original = parts.concat();
    assert_eq!(
        store
            .unit(old.unit_id.clone(), old.text_revision)
            .unwrap()
            .source_text,
        original
    );
    let growth = store
        .ingest_hook_text(event("100", incoming), 3200)
        .unwrap();
    let version = growth.visible_version.unwrap();
    assert_eq!(version.unit_id, old.unit_id);
    assert_eq!(version.text_revision, old.text_revision + 1);
    assert_eq!(
        store
            .unit(version.unit_id.clone(), version.text_revision)
            .unwrap()
            .source_text,
        format!("{prefix}{incoming}"),
        "quoted snapshot must retain the prefix and replace the entire old quoted suffix"
    );
    assert!(growth.finalized_versions.is_empty());
    let replay = store
        .ingest_hook_text(event("100", incoming), 3300)
        .unwrap();
    assert!(replay.visible_version.is_none());
    assert!(replay.finalized_versions.is_empty());
    assert_eq!(
        store
            .unit(old.unit_id.clone(), old.text_revision)
            .unwrap()
            .source_text,
        original
    );
    let complete = store.ingest_hook_text(event("101", tail), 3600).unwrap();
    let final_version = complete.visible_version.unwrap();
    assert_eq!(final_version.unit_id, old.unit_id);
    assert_eq!(complete.finalized_versions, vec![final_version.clone()]);
    let expected = format!("{prefix}{incoming}{tail}");
    assert_eq!(
        store
            .unit(final_version.unit_id.clone(), final_version.text_revision)
            .unwrap()
            .source_text,
        expected
    );
    let c =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let bytes: Vec<Vec<u8>> = c.prepare("SELECT substr(p.payload_bytes,f.byte_start+1,f.byte_end-f.byte_start) FROM source_refs f JOIN raw_event_payloads p USING(event_id) WHERE f.unit_id=?1 AND f.text_revision=?2 ORDER BY f.ref_ordinal").unwrap().query_map(rusqlite::params![final_version.unit_id,final_version.text_revision],|r|r.get(0)).unwrap().collect::<Result<_,_>>().unwrap();
    assert_eq!(bytes.concat(), expected.as_bytes());
    assert_eq!(
        inspect_capture(&path, "SELECT count(*) FROM reading_units"),
        1
    );
}
#[test]
fn capture_v3_quoted_growth_japanese_unicode_and_multi_ref_prefix() {
    assert_quoted_prefix_growth(
        &["【", "葵😀】「明日は"],
        "【葵😀】",
        "「明日は図書館で",
        "待とう。」",
    );
}
#[test]
fn capture_v3_quoted_growth_chinese_does_not_duplicate_the_quote() {
    assert_quoted_prefix_growth(&["旁白“风停了，"], "旁白", "“风停了，雨也快", "停了。”");
}
#[test]
fn capture_v3_quoted_growth_prefix_ending_at_a_ref_boundary() {
    assert_quoted_prefix_growth(
        &["が😀", "『来週は"],
        "が😀",
        "『来週は一緒に",
        "出かけよう。』",
    );
}

fn reference_rows(
    path: &std::path::Path,
    unit: &str,
    revision: u32,
) -> Vec<(String, i64, i64, Vec<u8>)> {
    let c = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap();
    c.prepare("SELECT f.event_id,f.byte_start,f.byte_end,substr(p.payload_bytes,f.byte_start+1,f.byte_end-f.byte_start) FROM source_refs f JOIN raw_event_payloads p USING(event_id) WHERE f.unit_id=?1 AND f.text_revision=?2 ORDER BY f.ref_ordinal").unwrap().query_map(rusqlite::params![unit,revision],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap().collect::<Result<_,_>>().unwrap()
}
#[test]
fn capture_v3_prefix_spans_multiple_refs_with_nonzero_raw_offset_and_keeps_old_refs() {
    let (store, path) = aggregation_store("quoted-v3-offset");
    let raw = format!("{}【", "a".repeat(CAPTURE_MAX_BYTES));
    let first = store
        .ingest_hook_text(event("2", &raw), 1000)
        .unwrap()
        .visible_version
        .unwrap();
    let old = store
        .ingest_hook_text(event("3", "葵😀】「前半"), 1100)
        .unwrap()
        .visible_version
        .unwrap();
    assert_eq!(old.unit_id, first.unit_id);
    let original = reference_rows(&path, &old.unit_id, old.text_revision);
    assert_eq!(original.len(), 2);
    assert_eq!(original[0].1, CAPTURE_MAX_BYTES as i64);
    let grown = store
        .ingest_hook_text(event("4", "「前半と続き"), 3200)
        .unwrap()
        .visible_version
        .unwrap();
    assert_eq!(grown.unit_id, old.unit_id);
    let expected = "【葵😀】「前半と続き";
    assert_eq!(
        store
            .unit(grown.unit_id.clone(), grown.text_revision)
            .unwrap()
            .source_text,
        expected
    );
    let refs = reference_rows(&path, &grown.unit_id, grown.text_revision);
    assert_eq!(refs.len(), 3);
    assert_eq!(
        (&refs[0].0, refs[0].1, refs[0].2),
        (
            &event("2", "").event_id,
            CAPTURE_MAX_BYTES as i64,
            (CAPTURE_MAX_BYTES + "【".len()) as i64
        )
    );
    assert_eq!((refs[1].1, refs[1].2), (0, "葵😀】".len() as i64));
    assert_eq!(
        refs.iter().flat_map(|r| r.3.clone()).collect::<Vec<_>>(),
        expected.as_bytes()
    );
    assert_eq!(
        reference_rows(&path, &old.unit_id, old.text_revision),
        original
    );
    let c =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let snapshot:String=c.query_row("SELECT processing_snapshot_json FROM reading_unit_revisions WHERE unit_id=?1 AND text_revision=?2",rusqlite::params![grown.unit_id,grown.text_revision],|r|r.get(0)).unwrap();
    let snapshot: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    assert_eq!(snapshot["fold"], "GrowingQuotedSnapshot");
    assert_eq!(snapshot["prefixBytes"], "【葵😀】".len());
    assert_eq!(
        snapshot["quotedGrowth"],
        "unique-open-quote-prefix-growth-v1"
    );
}
#[test]
fn capture_v3_rejects_missing_or_invalid_utf8_provenance_and_records_unknown_integrity() {
    for damaged in ["missing", "utf8"] {
        let (store, path) = aggregation_store(&format!("quoted-v3-damaged-{damaged}"));
        let old = store
            .ingest_hook_text(event("2", "名前「前半"), 1000)
            .unwrap()
            .visible_version
            .unwrap();
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute("PRAGMA foreign_keys=ON", []).unwrap();
        c.execute("DELETE FROM source_refs WHERE unit_id=?1", [&old.unit_id])
            .unwrap();
        if damaged == "utf8" {
            c.execute(
                "INSERT INTO source_refs VALUES(?1,1,'hook-session',0,?2,1,?3)",
                rusqlite::params![
                    old.unit_id,
                    event("2", "").event_id,
                    "名前「前半".len() as i64
                ],
            )
            .unwrap();
        }
        drop(c);
        let error = store
            .ingest_hook_text_with_policy(event("3", "「前半の続き"), true, 3200)
            .unwrap_err();
        assert_eq!(error.code, "storage_failure");
        assert_eq!(
            store
                .unit(old.unit_id.clone(), old.text_revision)
                .unwrap()
                .source_text,
            "名前「前半"
        );
        assert_eq!(
            inspect_capture(&path, "SELECT count(*) FROM reading_unit_revisions"),
            1
        );
        assert_eq!(
            inspect_capture(&path, "SELECT count(*) FROM raw_events"),
            2,
            "failed transaction must not pretend the new raw fact committed"
        );
        store
            .dispatch(
                CommandEnvelope {
                    operation_id: "damaged-disconnect".into(),
                    expected_revision: None,
                    command: Command::Disconnect,
                },
                3500,
            )
            .unwrap();
        assert_eq!(
            inspect_capture(
                &path,
                "SELECT count(*) FROM hook_events WHERE code='storage_unconfirmed' AND integrity='unknown'"
            ),
            1
        );
        assert!(store.snapshot().unwrap().tasks.is_empty());
    }
}
#[test]
fn capture_v3_quoted_growth_ref_capacity_and_zero_length_legacy_span_are_bounded() {
    let (store, path) = aggregation_store("quoted-v3-many-refs");
    let mut old = None;
    for n in 0..126 {
        let text = if n == 125 {
            "「前半".into()
        } else {
            format!("字{n}")
        };
        old = store
            .ingest_hook_text(event(&(n + 2).to_string(), &text), 1000 + n as i64)
            .unwrap()
            .visible_version;
    }
    let old = old.unwrap();
    let old_refs = reference_rows(&path, &old.unit_id, old.text_revision);
    assert_eq!(old_refs.len(), 126);
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute(
        "INSERT INTO source_refs VALUES(?1,?2,'hook-session',126,?3,0,0)",
        rusqlite::params![old.unit_id, old.text_revision, event("2", "").event_id],
    )
    .unwrap();
    drop(c);
    let prefix = store
        .unit(old.unit_id.clone(), old.text_revision)
        .unwrap()
        .source_text
        .strip_suffix("「前半")
        .unwrap()
        .to_owned();
    let grown = store
        .ingest_hook_text(event("1000", "「前半の続き"), 3200)
        .unwrap()
        .visible_version
        .unwrap();
    assert_eq!(grown.unit_id, old.unit_id);
    assert_eq!(grown.text_revision, 127);
    let refs = reference_rows(&path, &grown.unit_id, grown.text_revision);
    assert_eq!(
        refs.len(),
        126,
        "zero-length source stays in old revision only; no fake merged raw span"
    );
    assert_eq!(
        refs.iter().flat_map(|r| r.3.clone()).collect::<Vec<_>>(),
        format!("{prefix}「前半の続き").as_bytes()
    );
    let capped = store.ingest_hook_text(event("1001", "続く"), 3300).unwrap();
    assert_eq!(capped.visible_version.as_ref().unwrap().text_revision, 128);
    assert_eq!(
        capped.finalized_versions,
        vec![capped.visible_version.unwrap()]
    );
    let next = store
        .ingest_hook_text(event("1002", "「前半の続きと後"), 3400)
        .unwrap()
        .visible_version
        .unwrap();
    assert_ne!(next.unit_id, old.unit_id);
}

#[test]
fn capture_v3_retained_refs_plus_incoming_reach_exactly_128_then_seal() {
    for extra_zero in [false, true] {
        let (store, path) = aggregation_store(&format!("quoted-v3-ref-edge-{extra_zero}"));
        let mut old = None;
        for n in 0..127 {
            let text = if n == 126 {
                "名「前半".into()
            } else {
                format!("字{n}")
            };
            old = store
                .ingest_hook_text(event(&(n + 2).to_string(), &text), 1000 + n as i64)
                .unwrap()
                .visible_version;
        }
        let old = old.unwrap();
        if extra_zero {
            let c = rusqlite::Connection::open(&path).unwrap();
            c.execute(
                "INSERT INTO source_refs VALUES(?1,?2,'hook-session',127,?3,0,0)",
                rusqlite::params![old.unit_id, old.text_revision, event("2", "").event_id],
            )
            .unwrap();
        }
        let growth = store
            .ingest_hook_text(event("1000", "「前半の続き"), 3200)
            .unwrap();
        let new = growth.visible_version.unwrap();
        if extra_zero {
            assert_ne!(new.unit_id, old.unit_id);
            assert_eq!(new.text_revision, 1);
            assert_eq!(growth.finalized_versions, vec![old]);
        } else {
            assert_eq!(new.unit_id, old.unit_id);
            assert_eq!(new.text_revision, 128);
            assert_eq!(
                reference_rows(&path, &new.unit_id, new.text_revision).len(),
                128
            );
            assert_eq!(growth.finalized_versions, vec![new]);
        }
    }
}

#[test]
fn capture_v3_quoted_snapshot_exact_byte_limit_and_overflow_preserve_original_facts() {
    let prefix = "a".repeat(60_000);
    let suffix = format!("「{}", "語".repeat(1000));
    let original = format!("{prefix}{suffix}");
    for overflow in [false, true] {
        let (store, path) = aggregation_store(&format!("quoted-v3-byte-edge-{overflow}"));
        let old = store
            .ingest_hook_text(event("2", &original), 1000)
            .unwrap()
            .visible_version
            .unwrap();
        let extra = CAPTURE_MAX_BYTES - prefix.len() - suffix.len() + usize::from(overflow);
        let incoming = format!("{suffix}{}", "b".repeat(extra));
        assert!(original.len() + incoming.len() > CAPTURE_MAX_BYTES);
        let result = store.ingest_hook_text(event("3", &incoming), 3200).unwrap();
        let version = result.visible_version.unwrap();
        if overflow {
            assert_ne!(version.unit_id, old.unit_id);
            assert_eq!(version.text_revision, 1);
            assert_eq!(result.finalized_versions, vec![old.clone()]);
            assert_eq!(
                store.unit(version.unit_id, 1).unwrap().source_text,
                incoming
            );
        } else {
            assert_eq!(version.unit_id, old.unit_id);
            assert_eq!(version.text_revision, 2);
            assert_eq!(result.finalized_versions, vec![version.clone()]);
            let exact = store.unit(version.unit_id.clone(), 2).unwrap().source_text;
            assert_eq!(exact.len(), CAPTURE_MAX_BYTES);
            assert_eq!(exact, format!("{prefix}{incoming}"));
            assert_eq!(
                reference_rows(&path, &version.unit_id, 2)
                    .iter()
                    .flat_map(|r| r.3.clone())
                    .collect::<Vec<_>>(),
                exact.as_bytes()
            );
        }
        assert_eq!(store.unit(old.unit_id, 1).unwrap().source_text, original);
        assert_eq!(inspect_capture(&path, "SELECT count(*) FROM raw_events"), 3);
    }
}
#[test]
fn capture_growing_snapshot_preserves_old_revision_until_hard_age_finalizes() {
    let (store, path) = aggregation_store("aggregate-growth");
    let first = store
        .ingest_hook_text(event("2", "雨"), 1000)
        .unwrap()
        .visible_version
        .unwrap();
    let second = store
        .ingest_hook_text(event("3", "雨がやんだ"), 1200)
        .unwrap();
    assert!(second.finalized_versions.is_empty());
    assert_eq!(
        second.visible_version.as_ref().unwrap().unit_id,
        first.unit_id
    );
    assert_eq!(
        store.unit(first.unit_id.clone(), 0).unwrap().source_text,
        "雨がやんだ"
    );
    assert_eq!(
        store.unit(first.unit_id.clone(), 1).unwrap().source_text,
        "雨"
    );
    assert_eq!(
        inspect_capture(
            &path,
            "SELECT count(*) FROM source_refs WHERE text_revision=2"
        ),
        1
    );
    let ready = store
        .flush_hook_capture(
            CaptureFlush {
                session_id: Some("hook-session".into()),
                reason: CaptureFlushReason::Due,
            },
            1000 + CAPTURE_MAX_AGE_MS,
        )
        .unwrap();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].text_revision, 2);
    assert!(
        store
            .flush_hook_capture(
                CaptureFlush {
                    session_id: None,
                    reason: CaptureFlushReason::Due
                },
                9000
            )
            .unwrap()
            .is_empty()
    );
}

#[test]
fn capture_v2_slow_growth_ignores_idle_and_hard_age_is_single_shot() {
    let (store, path) = aggregation_store("aggregate-v2-growth");
    let first = store
        .ingest_hook_text(event("2", "雨"), 1000)
        .unwrap()
        .visible_version
        .unwrap();
    for now in [2499, 2500, 2501, 3300] {
        assert!(
            store
                .flush_hook_capture(
                    CaptureFlush {
                        session_id: None,
                        reason: CaptureFlushReason::Due
                    },
                    now
                )
                .unwrap()
                .is_empty()
        );
    }
    let latest = store
        .ingest_hook_text(event("3", "雨がやんだ"), 3700)
        .unwrap()
        .visible_version
        .unwrap();
    assert_eq!(first.unit_id, latest.unit_id);
    assert_eq!(latest.text_revision, 2);
    assert_eq!(
        store.unit(first.unit_id.clone(), 1).unwrap().source_text,
        "雨"
    );
    assert_eq!(
        store.unit(first.unit_id.clone(), 2).unwrap().source_text,
        "雨がやんだ"
    );
    assert_eq!(
        inspect_capture(
            &path,
            "SELECT count(*) FROM source_refs WHERE text_revision=2"
        ),
        1
    );
    assert!(
        store
            .flush_hook_capture(
                CaptureFlush {
                    session_id: None,
                    reason: CaptureFlushReason::Due
                },
                6999
            )
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .flush_hook_capture(
                CaptureFlush {
                    session_id: None,
                    reason: CaptureFlushReason::Due
                },
                7000
            )
            .unwrap(),
        vec![latest]
    );
    assert!(
        store
            .flush_hook_capture(
                CaptureFlush {
                    session_id: None,
                    reason: CaptureFlushReason::Due
                },
                7001
            )
            .unwrap()
            .is_empty()
    );
}

#[test]
fn capture_v2_balanced_quotes_keep_inner_punctuation_and_raw_unicode() {
    let (store, path) = aggregation_store("aggregate-v2-quotes");
    let first = store
        .ingest_hook_text(event("2", "「😀まだ続く。"), 1000)
        .unwrap();
    assert!(first.finalized_versions.is_empty());
    assert!(
        store
            .flush_hook_capture(
                CaptureFlush {
                    session_id: None,
                    reason: CaptureFlushReason::Due
                },
                2600
            )
            .unwrap()
            .is_empty()
    );
    let done = store
        .ingest_hook_text(event("3", "続きです。」\n"), 3800)
        .unwrap();
    assert_eq!(
        first.visible_version.unwrap().unit_id,
        done.visible_version.as_ref().unwrap().unit_id
    );
    assert_eq!(done.finalized_versions.len(), 1);
    assert_eq!(
        store.snapshot().unwrap().current.unwrap().source_text,
        "「😀まだ続く。続きです。」\n"
    );
    let quoted_word = store
        .ingest_hook_text(event("4", "「引用」"), 4000)
        .unwrap();
    assert!(quoted_word.finalized_versions.is_empty());
    assert!(
        store
            .flush_hook_capture(
                CaptureFlush {
                    session_id: None,
                    reason: CaptureFlushReason::Due
                },
                5600
            )
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        inspect_capture(
            &path,
            "SELECT count(*) FROM reading_units WHERE state='open'"
        ),
        1
    );
}

#[test]
fn capture_v2_replay_repetition_rollback_and_revision_capacity_remain_distinct() {
    let (store, path) = aggregation_store("aggregate-v2-repeat");
    let first = store
        .ingest_hook_text(event("2", "長い文"), 1000)
        .unwrap()
        .visible_version
        .unwrap();
    assert!(
        store
            .ingest_hook_text(event("2", "長い文"), 1100)
            .unwrap()
            .visible_version
            .is_none()
    );
    let repeated = store.ingest_hook_text(event("3", "長い文"), 2700).unwrap();
    assert_eq!(repeated.finalized_versions, vec![first.clone()]);
    let repeated = repeated.visible_version.unwrap();
    assert_ne!(first.unit_id, repeated.unit_id);
    let rollback = store.ingest_hook_text(event("4", "長い"), 2800).unwrap();
    assert_eq!(rollback.finalized_versions, vec![repeated]);
    assert_eq!(inspect_capture(&path, "SELECT count(*) FROM raw_events"), 4);
    for n in 1..CAPTURE_MAX_REVISIONS {
        let out = store
            .ingest_hook_text(
                event(&(n + 4).to_string(), &format!("片{n}")),
                2800 + i64::from(n),
            )
            .unwrap();
        if n == CAPTURE_MAX_REVISIONS - 1 {
            assert_eq!(
                out.visible_version.as_ref().unwrap().text_revision,
                CAPTURE_MAX_REVISIONS
            );
            assert_eq!(out.finalized_versions, vec![out.visible_version.unwrap()]);
        } else {
            assert!(out.finalized_versions.is_empty());
        }
    }
    assert_eq!(
        inspect_capture(
            &path,
            "SELECT max(text_revision) FROM reading_unit_revisions"
        ),
        i64::from(CAPTURE_MAX_REVISIONS)
    );
    assert_eq!(
        inspect_capture(
            &path,
            "SELECT max(n) FROM (SELECT count(*) n FROM source_refs GROUP BY unit_id,text_revision)"
        ),
        CAPTURE_MAX_REFS as i64
    );
    let next = store
        .ingest_hook_text(event("1000", "新規"), 3000)
        .unwrap()
        .visible_version
        .unwrap();
    assert_eq!(next.text_revision, 1);
}

// Construct historical on-disk facts, never rewriting a revision or adding a
// test-only production policy selector. The source remains exact and referenced.
fn legacy_open_fixture(store: &SqliteStore, path: &std::path::Path, processing: &str) -> String {
    let base = store
        .ingest_hook_text(event("2", "過去"), 1000)
        .unwrap()
        .visible_version
        .unwrap();
    store
        .flush_hook_capture(
            CaptureFlush {
                session_id: None,
                reason: CaptureFlushReason::Shutdown,
            },
            1100,
        )
        .unwrap();
    let old_snapshot = serde_json::json!({"idleMs":1500,"maxAgeMs":6000,"maxBytes":65536,"maxRevisions":128,"maxRefs":128,"fold":"NewUnit","join":"exact-concatenation","unitProjection":"latest"}).to_string();
    let old_snapshot = if processing == CAPTURE_PROCESSING_REVISION {
        serde_json::json!({"policy":"bounded-continuation-v2","idleMs":1500,"idleFinalizesIncomplete":false,"completion":"balanced-terminal-v2","maxAgeMs":6000,"maxBytes":65536,"maxRevisions":128,"maxRefs":128,"fold":"NewUnit","join":"exact-concatenation","unitProjection":"latest"}).to_string()
    } else {
        old_snapshot
    };
    let c = rusqlite::Connection::open(path).unwrap();
    c.execute("PRAGMA foreign_keys=ON", []).unwrap();
    c.execute(
        "INSERT INTO reading_units VALUES('legacy-open','hook-session',2,'open',1000)",
        [],
    )
    .unwrap();
    c.execute("INSERT INTO reading_unit_revisions SELECT 'legacy-open',1,session_id,source_text,source_byte_length,source_sha256,source_language,?2,?3,selection_revision,1000 FROM reading_unit_revisions WHERE unit_id=?1 AND text_revision=1",rusqlite::params![base.unit_id,processing,old_snapshot]).unwrap();
    c.execute("INSERT INTO source_refs SELECT 'legacy-open',1,session_id,ref_ordinal,event_id,byte_start,byte_end FROM source_refs WHERE unit_id=?1 AND text_revision=1", [&base.unit_id]).unwrap();
    old_snapshot
}

#[test]
fn capture_v2_never_continues_legacy_or_unknown_policy_and_legacy_timer_keeps_v1() {
    for processing in [
        CAPTURE_PROCESSING_REVISION_V1,
        CAPTURE_PROCESSING_REVISION,
        "future-unrecognized-policy",
    ] {
        for timer in [false, true] {
            let (store, path) =
                aggregation_store(&format!("aggregate-legacy-{processing}-{timer}"));
            let original = legacy_open_fixture(&store, &path, processing);
            if timer {
                if processing == CAPTURE_PROCESSING_REVISION_V1 {
                    assert!(
                        store
                            .flush_hook_capture(
                                CaptureFlush {
                                    session_id: None,
                                    reason: CaptureFlushReason::Due
                                },
                                2499
                            )
                            .unwrap()
                            .is_empty()
                    );
                }
                let due = store
                    .flush_hook_capture(
                        CaptureFlush {
                            session_id: None,
                            reason: CaptureFlushReason::Due,
                        },
                        if processing == CAPTURE_PROCESSING_REVISION {
                            7000
                        } else {
                            2500
                        },
                    )
                    .unwrap();
                assert_eq!(
                    due,
                    vec![CaptureVersion {
                        unit_id: "legacy-open".into(),
                        text_revision: 1
                    }]
                );
            } else {
                let next = store.ingest_hook_text(event("3", "続き"), 2000).unwrap();
                assert_eq!(
                    next.finalized_versions,
                    vec![CaptureVersion {
                        unit_id: "legacy-open".into(),
                        text_revision: 1
                    }]
                );
                assert_ne!(
                    next.visible_version.as_ref().unwrap().unit_id,
                    "legacy-open"
                );
                assert_eq!(next.visible_version.unwrap().text_revision, 1);
            }
            let c = rusqlite::Connection::open_with_flags(
                &path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .unwrap();
            let old: (String,String,String,u32) = c.query_row("SELECT source_text,processing_revision,processing_snapshot_json,text_revision FROM reading_unit_revisions WHERE unit_id='legacy-open'", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
            assert_eq!(old, ("過去".into(), processing.into(), original, 1));
            assert_eq!(
                store.unit("legacy-open".into(), 1).unwrap().source_text,
                "過去"
            );
            assert_eq!(
                inspect_capture(
                    &path,
                    "SELECT count(*) FROM source_refs WHERE unit_id='legacy-open'"
                ),
                1
            );
        }
    }
}

#[test]
fn capture_v2_processing_identity_cannot_reuse_legacy_translation_cache() {
    let (store, path) = aggregation_store("aggregate-legacy-cache");
    let mut c = config();
    c.bindings[0]
        .schedule
        .insert("cacheTtlMs".into(), serde_json::json!(10000));
    let saved = command(
        &store,
        "cache-config-save",
        Command::SaveConfiguration {
            configuration: c,
            expected_head_revision: 0,
        },
    );
    command(
        &store,
        "cache-config-apply",
        Command::ApplyConfiguration {
            revision_id: saved.result_id.unwrap(),
            expected_head_revision: 0,
        },
    );
    legacy_open_fixture(&store, &path, CAPTURE_PROCESSING_REVISION_V1);
    let new = store
        .ingest_hook_text(event("3", "過去"), 2000)
        .unwrap()
        .visible_version
        .unwrap();
    store
        .flush_hook_capture(
            CaptureFlush {
                session_id: None,
                reason: CaptureFlushReason::Shutdown,
            },
            2100,
        )
        .unwrap();
    let old_task = store
        .prepare_tasks_for_operation(
            "legacy-open".into(),
            1,
            "translation".into(),
            "legacy-cache-task".into(),
            2200,
        )
        .unwrap()
        .remove(0);
    store.complete_task(completion(&old_task), 2300).unwrap();
    let new_task = store
        .prepare_tasks_for_operation(
            new.unit_id,
            1,
            "translation".into(),
            "v2-cache-task".into(),
            2400,
        )
        .unwrap()
        .remove(0);
    assert_eq!(old_task.unit.source_text, new_task.unit.source_text);
    assert_ne!(
        old_task.request_semantics_hash,
        new_task.request_semantics_hash
    );
    assert!(new_task.cached_result.is_none());
    assert_eq!(
        store
            .unit("legacy-open".into(), 1)
            .unwrap()
            .translations
            .len(),
        1
    );
}
#[test]
fn capture_v3_processing_identity_cannot_reuse_v2_translation_cache() {
    let (store, path) = aggregation_store("aggregate-v2-cache");
    let mut c = config();
    c.bindings[0]
        .schedule
        .insert("cacheTtlMs".into(), serde_json::json!(10000));
    let saved = command(
        &store,
        "cache-config-save",
        Command::SaveConfiguration {
            configuration: c,
            expected_head_revision: 0,
        },
    );
    command(
        &store,
        "cache-config-apply",
        Command::ApplyConfiguration {
            revision_id: saved.result_id.unwrap(),
            expected_head_revision: 0,
        },
    );
    legacy_open_fixture(&store, &path, CAPTURE_PROCESSING_REVISION);
    let new = store
        .ingest_hook_text(event("3", "過去"), 2000)
        .unwrap()
        .visible_version
        .unwrap();
    store
        .flush_hook_capture(
            CaptureFlush {
                session_id: None,
                reason: CaptureFlushReason::Shutdown,
            },
            2100,
        )
        .unwrap();
    let old_task = store
        .prepare_tasks_for_operation(
            "legacy-open".into(),
            1,
            "translation".into(),
            "legacy-cache-task".into(),
            2200,
        )
        .unwrap()
        .remove(0);
    store.complete_task(completion(&old_task), 2300).unwrap();
    let new_task = store
        .prepare_tasks_for_operation(
            new.unit_id,
            1,
            "translation".into(),
            "v2-cache-task".into(),
            2400,
        )
        .unwrap()
        .remove(0);
    assert_eq!(old_task.unit.source_text, new_task.unit.source_text);
    assert_ne!(
        old_task.request_semantics_hash,
        new_task.request_semantics_hash
    );
    assert!(new_task.cached_result.is_none());
    assert_eq!(
        store
            .unit("legacy-open".into(), 1)
            .unwrap()
            .translations
            .len(),
        1
    );
}
#[test]
fn capture_capacity_splits_utf8_without_discarding_raw_text() {
    for sequence in ["2", "71", "18446744073709551615"] {
        let (store, path) = aggregation_store(&format!("aggregate-capacity-{sequence}"));
        let text = format!(
            "{}{}終",
            "語".repeat(CAPTURE_MAX_BYTES / 3 + 1),
            "😀".repeat(CAPTURE_MAX_BYTES / 4)
        );
        let outcome = store
            .ingest_hook_text(event(sequence, &text), 1000)
            .unwrap();
        assert_eq!(outcome.finalized_versions.len(), 2);
        let snapshot = store.snapshot().unwrap();
        let mut units = snapshot.history.units.clone();
        units.reverse();
        assert_eq!(units.len(), 3);
        assert!(
            units
                .iter()
                .all(|u| u.source_text.len() <= CAPTURE_MAX_BYTES)
        );
        assert_eq!(
            snapshot.current.unwrap().source_text,
            units.last().unwrap().source_text
        );
        assert_eq!(
            units
                .iter()
                .map(|u| u.source_text.as_str())
                .collect::<String>(),
            text
        );
        assert_eq!(inspect_capture(&path, "SELECT count(*) FROM raw_events"), 2);
        assert_eq!(
            inspect_capture(&path, "SELECT sum(byte_end-byte_start) FROM source_refs"),
            text.len() as i64
        );
    }
}
#[test]
fn capture_command_boundaries_seal_inside_existing_commit_cas() {
    for (name, action) in [
        ("pause", Command::SetCapturePaused { paused: true }),
        (
            "select",
            Command::SelectSource {
                source_id: "source-1".into(),
            },
        ),
        ("disconnect", Command::Disconnect),
    ] {
        let (store, path) = aggregation_store(&format!("aggregate-{name}"));
        store
            .ingest_hook_text(event("2", "未完の原文"), 1000)
            .unwrap();
        let before = store.snapshot().unwrap();
        let receipt = store
            .dispatch(
                CommandEnvelope {
                    operation_id: format!("boundary-{name}"),
                    expected_revision: Some(before.commit_revision),
                    command: action,
                },
                1100,
            )
            .unwrap();
        assert_eq!(receipt.revision, before.commit_revision + 1);
        assert_eq!(
            inspect_capture(
                &path,
                "SELECT count(*) FROM reading_units WHERE state='open'"
            ),
            0
        );
        assert_eq!(
            store.snapshot().unwrap().history.units[0].source_text,
            "未完の原文"
        );
        assert!(store.snapshot().unwrap().tasks.is_empty());
    }
}
#[test]
fn capture_abnormal_and_restart_preserve_source_without_resuming_requests() {
    let (store, path) = aggregation_store("aggregate-abnormal");
    store
        .ingest_hook_text(event("2", "受信済み"), 1000)
        .unwrap();
    store
        .record_diagnostic(
            Diagnostic {
                code: "integrity_lost".into(),
                stage: "capture".into(),
                message: "controlled loss".into(),
                recoverable: true,
            },
            1100,
        )
        .unwrap();
    assert_eq!(
        inspect_capture(
            &path,
            "SELECT count(*) FROM reading_units WHERE state='open'"
        ),
        0
    );
    assert_eq!(store.snapshot().unwrap().session.unwrap().state, "failed");
    drop(store);
    let (store, path) = aggregation_store("aggregate-restart");
    store
        .ingest_hook_text(event("2", "再起動前"), 1000)
        .unwrap();
    drop(store);
    let reopened = SqliteStore::open(&path, "profile-test", "new-server", 2000).unwrap();
    assert_eq!(
        inspect_capture(
            &path,
            "SELECT count(*) FROM reading_units WHERE state='open'"
        ),
        0
    );
    let snapshot = reopened.snapshot().unwrap();
    assert_eq!(snapshot.session.unwrap().state, "crashed");
    assert_eq!(snapshot.history.units[0].source_text, "再起動前");
    assert!(snapshot.tasks.is_empty());
}
#[test]
fn raw_retention_preserves_payloads_referenced_by_open_capture() {
    let (store, path) = aggregation_store("aggregate-retention");
    let version = store
        .ingest_hook_text(event("2", "正在聚合的正文"), 1000)
        .unwrap()
        .visible_version
        .unwrap();
    let policy = app_core::domain::retention::RetentionPolicy {
        max_rows_per_batch: 100,
        raw_payload_before_ms: Some(9000),
        raw_payload_byte_limit: Some(0),
        ..Default::default()
    };
    store
        .maintain_storage(
            policy.clone(),
            Default::default(),
            "retain-open-payload".into(),
            2000,
        )
        .unwrap();
    assert_eq!(
        inspect_capture(&path, "SELECT count(*) FROM raw_event_payloads"),
        2
    );
    let error = store
        .maintain_storage(
            app_core::domain::retention::RetentionPolicy {
                max_rows_per_batch: 100,
                history_unit_ids: vec![version.unit_id],
                ..Default::default()
            },
            Default::default(),
            "reject-open-unit-delete".into(),
            2001,
        )
        .unwrap_err();
    assert_eq!(error.code, "reading_object_in_use");
    store
        .flush_hook_capture(
            CaptureFlush {
                session_id: None,
                reason: CaptureFlushReason::Shutdown,
            },
            2002,
        )
        .unwrap();
    // Sealing a ReadingUnit does not authorize deleting the active session.
    store
        .maintain_storage(
            policy.clone(),
            Default::default(),
            "protect-active-after-seal".into(),
            2003,
        )
        .unwrap();
    assert_eq!(
        inspect_capture(&path, "SELECT count(*) FROM raw_event_payloads"),
        2
    );
    store
        .dispatch(
            CommandEnvelope {
                operation_id: "explicit-retention-disconnect".into(),
                expected_revision: None,
                command: Command::Disconnect,
            },
            2004,
        )
        .unwrap();
    store
        .maintain_storage(policy, Default::default(), "retain-after-seal".into(), 2005)
        .unwrap();
    assert_eq!(
        inspect_capture(&path, "SELECT count(*) FROM raw_event_payloads"),
        0
    );
    assert_eq!(inspect_capture(&path, "SELECT count(*) FROM raw_events"), 2);
}
#[test]
fn automatic_capture_authority_rejects_old_growth_revision_but_explicit_history_remains_usable() {
    let (store, _) = aggregation_store("aggregate-final-version-authority");
    configured(&store);
    let first = store
        .ingest_hook_text(event("2", "雨"), 1000)
        .unwrap()
        .visible_version
        .unwrap();
    let latest = store
        .ingest_hook_text(event("3", "雨がやんだ。"), 1100)
        .unwrap()
        .visible_version
        .unwrap();
    assert!(
        store
            .prepare_capture_tasks(first.clone(), "automatic-old-half".into(), 1200)
            .unwrap()
            .is_empty()
    );
    let tasks = store
        .prepare_capture_tasks(latest, "automatic-final-version".into(), 1201)
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].unit.source_text, "雨がやんだ。");
    assert_eq!(tasks[0].unit.text_revision, 2);
    let explicit = store
        .prepare_tasks_for_operation(
            first.unit_id,
            1,
            "translation".into(),
            "explicit-old-history".into(),
            1202,
        )
        .unwrap();
    assert_eq!(explicit.len(), 1);
    assert_eq!(explicit[0].unit.source_text, "雨");
}
fn command(s: &SqliteStore, op: &str, mut c: Command) -> CommandReceipt {
    match &mut c {
        Command::SaveConfiguration {
            expected_head_revision,
            ..
        } => *expected_head_revision = s.snapshot().unwrap().configuration.draft_head_revision,
        Command::ApplyConfiguration {
            expected_head_revision,
            ..
        } => *expected_head_revision = s.snapshot().unwrap().configuration.applied_head_revision,
        _ => {}
    }
    s.dispatch(
        CommandEnvelope {
            operation_id: op.into(),
            expected_revision: None,
            command: c,
        },
        20,
    )
    .unwrap()
}
fn game() -> GameInput {
    GameInput {
        game_id: None,
        row_revision: None,
        display_name: "Test game".into(),
        original_title: None,
        source_language: Some("ja".into()),
        cover_asset_ref: None,
        target: GameTarget {
            environment_kind: "crossover".into(),
            environment_key: "crossover:/private/bottle".into(),
            bottle_locator: Some("/private/bottle".into()),
            launch_path: Some("C:\\games\\game.exe".into()),
            canonical_exe_path: Some("C:\\games\\game.exe".into()),
            target_path_key: Some("c:/games/game.exe".into()),
            working_directory: None,
            arguments: vec![],
            architecture: Some("x86".into()),
            verification_state: "verified".into(),
        },
    }
}
fn config() -> Configuration {
    Configuration {
        revision_id: String::new(),
        connections: vec![Connection {
            connection_id: "connection".into(),
            display_name: "Local server".into(),
            protocol: "openai_compatible".into(),
            deployment: "local".into(),
            auth_mode: "none".into(),
            endpoint: Some("http://127.0.0.1:1234/v1".into()),
            credential_pool_id: None,
            credential_pool_revision: None,
            proxy_mode: "disabled".into(),
            network_metadata: BTreeMap::new(),
            availability_intent: true,
            adapter_binding: None,
            preset_source: None,
        }],
        models: vec![Model {
            default_templates: Default::default(),
            model_id: "model".into(),
            connection_id: "connection".into(),
            actual_model_id: Some("test-model".into()),
            display_name: "Test".into(),
            selection_source: "manual".into(),
            capabilities: BTreeMap::new(),
            parameters: BTreeMap::from([(
                "temperature".into(),
                Parameter {
                    mode: ParameterMode::Set,
                    value: Some(serde_json::json!(0)),
                },
            )]),
        }],
        bindings: vec![PurposeBinding {
            binding_id: "translation".into(),
            scope_key: "global".into(),
            game_id: None,
            purpose: "translation".into(),
            binding_mode: "override".into(),
            enabled: true,
            output_mode: Some("text".into()),
            source_language: Some("ja".into()),
            target_language: Some("zh-Hans".into()),
            template_id: None,
            context: BTreeMap::new(),
            schedule: BTreeMap::new(),
            parameters: BTreeMap::new(),
            engines: vec![Engine {
                engine_id: "engine".into(),
                model_id: "model".into(),
                enabled: true,
                template_id: None,
                parameters: BTreeMap::new(),
            }],
        }],
        templates: vec![],
        term_sets: vec![],
        codec_version: None,
    }
}
fn configured(s: &SqliteStore) {
    let r = command(
        s,
        "save-config",
        Command::SaveConfiguration {
            configuration: config(),
            expected_head_revision: 0,
        },
    );
    command(
        s,
        "apply-config",
        Command::ApplyConfiguration {
            revision_id: r.result_id.unwrap(),
            expected_head_revision: 0,
        },
    );
}
fn manual(s: &SqliteStore, op: &str, text: &str) -> String {
    command(
        s,
        op,
        Command::ManualInput {
            text: text.into(),
            source_language: "ja".into(),
        },
    )
    .result_id
    .unwrap()
}
fn completion(t: &TaskDispatch) -> TaskCompletion {
    TaskCompletion {
        task_id: t.task_id.clone(),
        attempt_id: t.attempt_id.clone(),
        text: "翻译结果".into(),
        output_mode: "text".into(),
        structured: serde_json::json!({}),
        target_language: "zh-Hans".into(),
        provider_label: "Test provider".into(),
    }
}
#[test]
fn fresh_schema_has_exact_profile_table_count_and_bundled_version() {
    let path = temp("schema");
    let s = SqliteStore::open(&path, "profile-test", "server-test", 10).unwrap();
    let (v, source) = s.database_versions().unwrap();
    assert!(!v.is_empty());
    assert!(!source.is_empty());
    s.checkpoint().unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let reviewed: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("docs/architecture/database/v5/schema-contract.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        count,
        reviewed["databases"]["profile"]["tables"]
            .as_object()
            .unwrap()
            .len() as i64
    );
    let old = rusqlite::Connection::open_in_memory().unwrap();
    old.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    old.execute_batch(include_str!("fixtures/http-schema2-original.sql"))
        .unwrap();
    assert_eq!(
        old.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        52
    );
    // Keep the original v3 inventory obligation on its independently frozen
    // migration, alongside the original schema2 count and the actual current v4.
    let migration_three = std::fs::read_to_string(root.join("docs/architecture/database/v3/003_http_service_identity.design.sql")).unwrap();
    let three_review: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("docs/architecture/database/v3/storage-review.json")).unwrap()).unwrap();
    assert_eq!(app_core::domain::content_hash(migration_three.as_bytes()), three_review["reviewed_design_sha256"]);
    old.execute_batch(&migration_three).unwrap();
    let three_contract: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("docs/architecture/database/v3/schema-contract.json")).unwrap()).unwrap();
    assert_eq!(old.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", [], |r| r.get::<_,i64>(0)).unwrap(), three_contract["databases"]["profile"]["tables"].as_object().unwrap().len() as i64);
    assert_eq!(conn.query_row("SELECT max(version) FROM schema_migrations", [], |r| r.get::<_,u32>(0)).unwrap(), 5);
    assert_eq!(s.snapshot().unwrap().revision, 0);
}
#[test]
fn game_cas_idempotency_archive_and_history_preservation() {
    let s = store("library");
    let r = command(&s, "add", Command::SaveGame(Box::new(game())));
    let replay = command(&s, "add", Command::SaveGame(Box::new(game())));
    assert!(replay.replayed);
    assert_eq!(replay.request_id, r.request_id);
    assert_eq!(replay.revision, r.revision);
    let mut g = game();
    g.display_name = "Different".into();
    assert_eq!(
        s.dispatch(
            CommandEnvelope {
                operation_id: "add".into(),
                expected_revision: None,
                command: Command::SaveGame(Box::new(g.clone()))
            },
            20
        )
        .unwrap_err()
        .code,
        "operation_conflict"
    );
    let snap = s.snapshot().unwrap();
    g.game_id = r.result_id.clone();
    g.row_revision = Some(1);
    command(&s, "edit", Command::SaveGame(Box::new(g.clone())));
    assert_eq!(
        s.dispatch(
            CommandEnvelope {
                operation_id: "stale".into(),
                expected_revision: None,
                command: Command::SaveGame(Box::new(g))
            },
            20
        )
        .unwrap_err()
        .code,
        "revision_conflict"
    );
    manual(&s, "text", "同じ台詞");
    command(
        &s,
        "archive",
        Command::ArchiveGame {
            game_id: r.result_id.unwrap(),
            row_revision: 2,
        },
    );
    assert!(s.snapshot().unwrap().games.is_empty());
    assert_eq!(s.snapshot().unwrap().history.units.len(), 1);
    assert_eq!(snap.games.len(), 1);
    acceptance_evidence::record(
        "operation-cas-idempotency",
        &[("STORAGE-20", 1)],
        serde_json::json!({"actual_test":"game_cas_idempotency_archive_and_history_preservation"}),
    );
}
#[test]
fn draft_is_not_applied_and_zero_parameter_survives() {
    let s = store("draft");
    command(
        &s,
        "save",
        Command::SaveConfiguration {
            configuration: config(),
            expected_head_revision: 0,
        },
    );
    let snap = s.snapshot().unwrap();
    assert!(snap.configuration.applied.is_none());
    assert_eq!(
        snap.configuration.draft.as_legacy().expect("legacy storage fixture remains codec1/2").clone().models[0].parameters["temperature"].value,
        Some(serde_json::json!(0))
    );
    configured(&s);
    assert!(s.snapshot().unwrap().configuration.applied.is_some());
    acceptance_evidence::record(
        "draft-tristate-parameters",
        &[("STORAGE-19", 1)],
        serde_json::json!({"actual_test":"draft_is_not_applied_and_zero_parameter_survives"}),
    );
}
#[test]
fn cancellation_wins_and_late_result_cannot_enter_history() {
    let s = store("cancel");
    configured(&s);
    let uid = manual(&s, "m", "原文");
    let t = s
        .prepare_tasks(uid, 1, "translation".into(), 30)
        .unwrap()
        .remove(0);
    command(
        &s,
        "cancel",
        Command::CancelTask {
            task_id: t.task_id.clone(),
        },
    );
    assert_eq!(
        s.complete_task(completion(&t), 40).unwrap_err().code,
        "task_not_authorized"
    );
    assert!(
        s.snapshot().unwrap().history.units[0]
            .translations
            .is_empty()
    );
}
#[test]
fn completed_old_sentence_stays_old_and_corrections_are_independent() {
    let s = store("results");
    configured(&s);
    let uid = manual(&s, "m1", "最初の台詞");
    let t = s
        .prepare_tasks(uid.clone(), 1, "translation".into(), 30)
        .unwrap()
        .remove(0);
    let newer = manual(&s, "m2", "次の台詞");
    s.complete_task(completion(&t), 40).unwrap();
    let snap = s.snapshot().unwrap();
    let old = snap
        .history
        .units
        .iter()
        .find(|u| u.unit_id == uid)
        .unwrap();
    let translation = old.translations[0].translation_id.clone();
    assert!(
        snap.history
            .units
            .iter()
            .find(|u| u.unit_id == newer)
            .unwrap()
            .translations
            .is_empty()
    );
    command(
        &s,
        "correction",
        Command::SaveCorrection {
            translation_id: translation.clone(),
            base_revision: 0,
            text: "我的修正".into(),
        },
    );
    let snap = s.snapshot().unwrap();
    let t = &snap
        .history
        .units
        .iter()
        .find(|u| u.unit_id == uid)
        .unwrap()
        .translations[0];
    assert_eq!(t.text, "翻译结果");
    assert_eq!(t.correction.as_ref().unwrap().text, "我的修正");
    assert!(
        s.dispatch(
            CommandEnvelope {
                operation_id: "bad-correction".into(),
                expected_revision: None,
                command: Command::SaveCorrection {
                    translation_id: translation,
                    base_revision: 0,
                    text: "冲突".into()
                }
            },
            50
        )
        .is_err()
    );
    acceptance_evidence::record(
        "correction-immutable-results",
        &[("STORAGE-13", 1)],
        serde_json::json!({"actual_test":"completed_old_sentence_stays_old_and_corrections_are_independent"}),
    );
}
#[test]
fn unknown_schema_refused_without_mutating_original() {
    let p = temp("unknown");
    let conn = rusqlite::Connection::open(&p).unwrap();
    conn.execute_batch("CREATE TABLE unrelated(x); INSERT INTO unrelated VALUES(1)")
        .unwrap();
    drop(conn);
    let before = std::fs::read(&p).unwrap();
    assert!(SqliteStore::open(&p, "p", "s", 1).is_err());
    assert_eq!(before, std::fs::read(&p).unwrap());
}
#[test]
fn backup_is_consistent_and_never_overwrites_existing() {
    let s = store("backup");
    manual(&s, "m", "保存済み");
    let p = temp("backup-destination");
    s.backup(&p).unwrap();
    let restored = SqliteStore::open(&p, "profile-test", "server-restored", 50).unwrap();
    assert_eq!(
        restored.snapshot().unwrap().history.units[0].source_text,
        "保存済み"
    );
    assert!(s.backup(&p).is_err());
}
#[test]
fn repeated_dialogue_is_not_text_deduplicated() {
    let s = store("repeated");
    manual(&s, "m1", "そうだ");
    manual(&s, "m2", "そうだ");
    assert_eq!(s.snapshot().unwrap().history.units.len(), 2);
}
use app_core::domain::capture::*;
fn resource(kind: &str) -> ResourceSnapshotInput {
    ResourceSnapshotInput {
        snapshot_id: format!("resource-{kind}"),
        kind: kind.into(),
        version: "fixture-v1".into(),
        manifest_sha256: content_hash(kind),
        contract_version: "1".into(),
        metadata_json: "{}".into(),
    }
}
fn hook(s: &SqliteStore, game: Option<String>) {
    let library_target = game.as_ref().map(|id| {
        s.snapshot()
            .unwrap()
            .games
            .into_iter()
            .find(|record| &record.game_id == id)
            .expect("hook fixture game must exist")
            .target
    });
    s.open_hook_session(
        HookSessionInput {
            session_id: "hook-session".into(),
            game_id: game,
            environment_key: library_target
                .as_ref()
                .map(|target| target.environment_key.clone())
                .unwrap_or_else(|| "crossover:/private/bottle".into()),
            target_exe_path: library_target
                .as_ref()
                .and_then(|target| target.canonical_exe_path.clone())
                .unwrap_or_else(|| "C:\\game.exe".into()),
            windows_pid: 100,
            process_created_u64: "18446744073709551615".into(),
            architecture: "x86".into(),
            broker_instance_id: "broker-1".into(),
            resource: resource("core"),
            target_identity_sha256: content_hash("target"),
        },
        30,
    )
    .unwrap();
}
fn event(sequence: &str, text: &str) -> HookTextInput {
    HookTextInput {
        session_id: "hook-session".into(),
        event_id: format!("event-{sequence}"),
        sequence_u64: sequence.into(),
        source: HookSourceInput {
            source_id: "source-1".into(),
            host_instance_id: "host-1".into(),
            hook_address_u64: "18446744073709551615".into(),
            context_u64: "0".into(),
            subcontext_u64: "9223372036854775808".into(),
            hook_code: "HW...".into(),
            hook_name: "TextOutW".into(),
            encoding: "utf-16le".into(),
            stream_identity_sha256: content_hash("stream"),
            identity_extensions_json: "{}".into(),
        },
        text: text.into(),
        source_language: "ja".into(),
    }
}
#[test]
fn hook_requires_explicit_selection_and_preserves_repeat_provenance() {
    let s = store("hook");
    hook(&s, None);
    assert!(
        s.receive_hook_text(event("18446744073709551613", "最初"), 40)
            .unwrap()
            .is_none()
    );
    let snap = s.snapshot().unwrap();
    assert!(snap.history.units.is_empty());
    assert_eq!(
        snap.session.as_ref().unwrap().sources[0].hook_name,
        "TextOutW"
    );
    assert!(!snap.session.as_ref().unwrap().sources[0].selected);
    assert_eq!(snap.session.as_ref().unwrap().sources[0].sample, "最初");
    command(
        &s,
        "select",
        Command::SelectSource {
            source_id: "source-1".into(),
        },
    );
    s.receive_hook_text(event("18446744073709551614", "同じ台詞"), 41)
        .unwrap()
        .unwrap();
    s.receive_hook_text(event("18446744073709551615", "同じ台詞"), 42)
        .unwrap()
        .unwrap();
    assert_eq!(s.snapshot().unwrap().history.units.len(), 2);
    assert!(
        s.receive_hook_text(event("18446744073709551615", "同じ台詞"), 43)
            .unwrap()
            .is_none()
    );
    assert!(
        s.receive_hook_text(event("18446744073709551615", "改ざん"), 43)
            .is_err()
    );
    acceptance_evidence::record(
        "hook-identity-provenance",
        &[("STORAGE-21", 1)],
        serde_json::json!({"actual_test":"hook_requires_explicit_selection_and_preserves_repeat_provenance"}),
    );
}
#[test]
fn lexical_alignment_is_checked_against_immutable_utf8_source() {
    let s = store("tokens");
    let uid = manual(&s, "m", "彼女😀と");
    let mut input = TokenSnapshotInput {
        unit_id: uid,
        text_revision: 1,
        analysis_revision: "fixture-v1".into(),
        resource: resource("language"),
        tokens: vec![LexicalToken {
            surface: "彼女".into(),
            lemma: "彼女".into(),
            reading: "かのじょ".into(),
            part_of_speech: "代名詞".into(),
            byte_start: 0,
            byte_end: 6,
            utf16_start: 0,
            utf16_end: 2,
            known: true,
        }],
    };
    let mut invalid = input.clone();
    invalid.tokens[0].byte_end = 5;
    assert!(s.set_tokens(invalid, 40).is_err());
    s.set_tokens(input.clone(), 40).unwrap();
    let snap = s.snapshot().unwrap();
    assert_eq!(snap.history.units[0].tokens, input.tokens);
    input.tokens[0].surface = "彼氏".into();
    assert!(s.set_tokens(input, 41).is_err());
}
#[test]
fn lexical_analysis_versions_coexist_without_rewriting_old_snapshots() {
    let path = temp("lexical-versions");
    let s = SqliteStore::open(path.clone(), "profile-test", "server-test", 10).unwrap();
    let uid = manual(&s, "lexical-source", "雨\n駅");
    let token = |surface: &str, byte_start, byte_end, utf16_start, utf16_end| LexicalToken {
        surface: surface.into(),
        lemma: surface.into(),
        reading: String::new(),
        part_of_speech: String::new(),
        known: false,
        byte_start,
        byte_end,
        utf16_start,
        utf16_end,
    };
    let legacy = vec![token("雨", 0, 3, 0, 1), token("駅", 4, 7, 2, 3)];
    let mut input = TokenSnapshotInput {
        unit_id: uid.clone(),
        text_revision: 1,
        analysis_revision: "mecab-unidic-v1".into(),
        tokens: legacy.clone(),
        resource: resource("language"),
    };
    s.set_tokens(input.clone(), 40).unwrap();
    let inspect =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let old: (String, String, i64) = inspect
        .query_row(
            "SELECT lexical_id,analysis_revision,token_count FROM lexical_snapshots",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    input.analysis_revision = "mecab-unidic-v2-lossless-whitespace".into();
    input.tokens.insert(1, token("\n", 3, 4, 1, 2));
    s.set_tokens(input.clone(), 41).unwrap();
    assert_eq!(s.unit(uid, 1).unwrap().tokens, input.tokens);
    let retained: (String, String, i64) = inspect.query_row("SELECT lexical_id,analysis_revision,token_count FROM lexical_snapshots WHERE lexical_id=?1", [&old.0], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(retained, old);
    assert_eq!(
        inspect
            .query_row("SELECT count(*) FROM lexical_snapshots", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        inspect
            .query_row(
                "SELECT count(*) FROM lexical_tokens WHERE lexical_id=?1",
                [&old.0],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        legacy.len() as i64
    );
    assert!(
        s.set_tokens(input, 42).is_err(),
        "Same immutable analysis version must not be replaced."
    );
    assert_eq!(
        inspect
            .query_row("SELECT count(*) FROM lexical_snapshots", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
}
#[test]
fn integrity_loss_stops_session_and_rejects_later_text() {
    let s = store("loss");
    hook(&s, None);
    s.record_diagnostic(
        Diagnostic {
            code: "integrity_lost".into(),
            stage: "capture".into(),
            message: "Queue overflow".into(),
            recoverable: true,
        },
        40,
    )
    .unwrap();
    assert!(s.receive_hook_text(event("1", "拒否"), 50).is_err());
    assert_eq!(
        s.snapshot().unwrap().session.as_ref().unwrap().state,
        "failed"
    );
    assert_eq!(s.snapshot().unwrap().diagnostics[0].code, "integrity_lost");
}
fn credential() -> CredentialReference {
    CredentialReference {
        credential_id: "credential-1".into(),
        credential_version_id: "vault-version-1".into(),
        pool_id: "pool-1".into(),
        pool_revision: 1,
        label: "Test".into(),
        masked_hint: "••••1234".into(),
    }
}
#[test]
fn pending_and_revoked_credentials_never_authorize() {
    let s = store("credentials");
    let c = credential();
    s.begin_credential(c.clone(), 20).unwrap();
    assert_eq!(s.pending_credentials().unwrap().len(), 1);
    assert!(s.credential_for_pool(c.pool_id.clone(), 1).is_err());
    s.activate_credential(c.credential_id.clone(), 30).unwrap();
    assert_eq!(
        s.credential_for_pool(c.pool_id.clone(), 1).unwrap(),
        c.credential_version_id
    );
    s.revoke_credential(c.credential_id.clone(), 40).unwrap();
    assert!(s.credential_for_pool(c.pool_id.clone(), 1).is_err());
    assert!(s.activate_credential(c.credential_id.clone(), 50).is_err());
    assert_eq!(s.pending_credentials().unwrap().len(), 1);
    s.mark_credential_cleaned(c.credential_id, 50).unwrap();
    assert!(s.pending_credentials().unwrap().is_empty());
}
#[test]
fn profile_writer_lease_refuses_second_instance() {
    let p = temp("lease");
    let _first = SqliteStore::open(&p, "p", "s", 1).unwrap();
    let failure = SqliteStore::open(&p, "p", "other", 2);
    assert!(failure.is_err());
    assert_eq!(failure.err().unwrap().code, "profile_in_use");
}
#[test]
fn display_rename_preserves_active_task_but_semantic_change_invalidates() {
    let s = store("semantics");
    configured(&s);
    let uid = manual(&s, "m", "待つ");
    let t = s
        .prepare_tasks(uid.clone(), 1, "translation".into(), 30)
        .unwrap()
        .remove(0);
    let mut c = s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy storage fixture remains codec1/2").clone();
    c.connections[0].display_name = "Renamed".into();
    c.models[0].display_name = "Renamed model".into();
    let r = command(
        &s,
        "rename",
        Command::SaveConfiguration {
            configuration: c,
            expected_head_revision: 0,
        },
    );
    command(
        &s,
        "apply-rename",
        Command::ApplyConfiguration {
            revision_id: r.result_id.unwrap(),
            expected_head_revision: 0,
        },
    );
    s.complete_task(completion(&t), 40).unwrap();
    let t = s
        .prepare_tasks(uid, 1, "translation".into(), 50)
        .unwrap()
        .remove(0);
    let mut c = s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy storage fixture remains codec1/2").clone();
    c.models[0].actual_model_id = Some("other-model".into());
    let r = command(
        &s,
        "change-model",
        Command::SaveConfiguration {
            configuration: c,
            expected_head_revision: 0,
        },
    );
    command(
        &s,
        "apply-model",
        Command::ApplyConfiguration {
            revision_id: r.result_id.unwrap(),
            expected_head_revision: 0,
        },
    );
    assert!(s.complete_task(completion(&t), 60).is_err());
}
#[test]
fn game_override_disables_only_that_games_inherited_tasks() {
    let s = store("game-override");
    let gid = command(&s, "add", Command::SaveGame(Box::new(game())))
        .result_id
        .unwrap();
    configured(&s);
    hook(&s, Some(gid.clone()));
    s.receive_hook_text(event("1", "候補"), 40).unwrap();
    command(
        &s,
        "select",
        Command::SelectSource {
            source_id: "source-1".into(),
        },
    );
    let uid = s
        .receive_hook_text(event("2", "ゲーム台詞"), 41)
        .unwrap()
        .unwrap();
    let game_task = s
        .prepare_tasks(uid, 1, "translation".into(), 42)
        .unwrap()
        .remove(0);
    let manual_uid = manual(&s, "manual", "手動");
    let manual_task = s
        .prepare_tasks(manual_uid, 1, "translation".into(), 43)
        .unwrap()
        .remove(0);
    let mut c = s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy storage fixture remains codec1/2").clone();
    let mut b = c.bindings[0].clone();
    b.binding_id = "game-translation".into();
    b.scope_key = format!("game:{gid}");
    b.game_id = Some(gid);
    b.enabled = false;
    c.bindings.push(b);
    let r = command(
        &s,
        "override",
        Command::SaveConfiguration {
            configuration: c,
            expected_head_revision: 0,
        },
    );
    command(
        &s,
        "apply-override",
        Command::ApplyConfiguration {
            revision_id: r.result_id.unwrap(),
            expected_head_revision: 0,
        },
    );
    assert!(s.complete_task(completion(&game_task), 60).is_err());
    s.complete_task(completion(&manual_task), 60).unwrap();
}
#[test]
fn configuration_head_cas_rejects_stale_form_even_with_fresh_commit_revision() {
    let s = store("head-cas");
    let expected = s.snapshot().unwrap().configuration.draft_head_revision;
    command(
        &s,
        "save1",
        Command::SaveConfiguration {
            configuration: config(),
            expected_head_revision: expected,
        },
    );
    let commit = s.snapshot().unwrap().commit_revision;
    let failure = s
        .dispatch(
            CommandEnvelope {
                operation_id: "save-stale".into(),
                expected_revision: Some(commit),
                command: Command::SaveConfiguration {
                    configuration: config(),
                    expected_head_revision: expected,
                },
            },
            30,
        )
        .unwrap_err();
    assert_eq!(failure.code, "revision_conflict");
    assert_eq!(s.snapshot().unwrap().configuration.draft_head_revision, 1);
    acceptance_evidence::record(
        "configuration-head-cas",
        &[("STORAGE-19", 1)],
        serde_json::json!({"actual_test":"configuration_head_cas_rejects_stale_form_even_with_fresh_commit_revision"}),
    );
}
#[test]
fn active_and_duplicate_operation_never_prepare_a_second_request() {
    let s = store("task-operation");
    configured(&s);
    let uid = manual(&s, "m", "同じ本文");
    let first = s
        .prepare_tasks_for_operation(
            uid.clone(),
            1,
            "translation".into(),
            "translate-1".into(),
            40,
        )
        .unwrap();
    assert_eq!(first.len(), 1);
    assert!(
        s.prepare_tasks_for_operation(
            uid.clone(),
            1,
            "translation".into(),
            "translate-1".into(),
            41
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        s.prepare_tasks_for_operation(
            uid.clone(),
            1,
            "translation".into(),
            "translate-2".into(),
            42
        )
        .unwrap()
        .is_empty()
    );
    s.complete_task(completion(&first[0]), 43).unwrap();
    assert!(
        s.prepare_tasks_for_operation(
            uid.clone(),
            1,
            "translation".into(),
            "translate-1".into(),
            44
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        s.prepare_tasks_for_operation(uid, 1, "translation".into(), "translate-new".into(), 45)
            .unwrap()
            .len(),
        1
    );
    acceptance_evidence::record(
        "task-operation-idempotency",
        &[("STORAGE-20", 1)],
        serde_json::json!({"actual_test":"active_and_duplicate_operation_never_prepare_a_second_request"}),
    );
}
#[test]
fn runtime_controls_receive_idempotent_storage_receipts() {
    let s = store("controls");
    hook(&s, None);
    let baseline = s.snapshot().unwrap().commit_revision;
    let paused = command(&s, "pause", Command::SetCapturePaused { paused: true });
    let replay = command(&s, "pause", Command::SetCapturePaused { paused: true });
    assert!(replay.replayed);
    assert_eq!(replay.revision, paused.revision);
    command(
        &s,
        "translate-off",
        Command::SetTranslationEnabled { enabled: false },
    );
    let unbind = CommandEnvelope {
        operation_id: "unbind".into(),
        expected_revision: Some(0), // unrelated raw/model revisions are not a window permission
        command: Command::UnbindWindow(GameWindowActivationRequest {
            expected_session_id: "hook-session".into(),
            expected_target_identity_sha256: content_hash("controlled-window"),
        }),
    };
    assert_eq!(
        s.dispatch(unbind.clone(), 40).unwrap_err().code,
        "native_effect_required"
    );
    assert!(s.preflight_window_unbind(unbind.clone()).unwrap().is_none());
    // Storage fixture models the dedicated callback after a controlled local
    // association effect; ordinary dispatch must never manufacture this fact.
    let committed = s.commit_window_unbind(unbind.clone(), 41).unwrap();
    let replay = s.preflight_window_unbind(unbind).unwrap().unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.revision, committed.revision);
    assert_eq!(s.snapshot().unwrap().commit_revision - baseline, 3);
}
#[test]
fn actual_schema_field_or_trigger_drift_is_rejected_before_mutation() {
    let s = store("schema-drift");
    let p = temp("schema-drift-copy");
    s.backup(&p).unwrap();
    let conn = rusqlite::Connection::open(&p).unwrap();
    conn.execute_batch("DROP TRIGGER immutable_corrections;")
        .unwrap();
    drop(conn);
    let before = std::fs::read(&p).unwrap();
    let result = SqliteStore::open(&p, "profile-test", "other", 50);
    assert_eq!(result.err().unwrap().code, "schema_contract_mismatch");
    assert_eq!(std::fs::read(&p).unwrap(), before);
}
#[test]
fn revoked_credential_invalidates_started_task_and_send_permission() {
    let s = store("revoke-task");
    let cred = credential();
    s.begin_credential(cred.clone(), 10).unwrap();
    s.activate_credential(cred.credential_id.clone(), 11)
        .unwrap();
    let mut c = config();
    c.connections[0].auth_mode = "api_key".into();
    c.connections[0].credential_pool_id = Some(cred.pool_id.clone());
    c.connections[0].credential_pool_revision = Some(1);
    let r = command(
        &s,
        "save",
        Command::SaveConfiguration {
            configuration: c,
            expected_head_revision: 0,
        },
    );
    command(
        &s,
        "apply",
        Command::ApplyConfiguration {
            revision_id: r.result_id.unwrap(),
            expected_head_revision: 0,
        },
    );
    let uid = manual(&s, "m", "原文");
    let t = s
        .prepare_tasks(uid, 1, "translation".into(), 40)
        .unwrap()
        .remove(0);
    s.validate_task_authority(t.task_id.clone()).unwrap();
    s.revoke_credential(cred.credential_id, 50).unwrap();
    assert!(s.validate_task_authority(t.task_id.clone()).is_err());
    assert!(s.complete_task(completion(&t), 60).is_err());
    assert!(
        s.snapshot().unwrap().history.units[0]
            .translations
            .is_empty()
    );
}
#[test]
fn stream_identity_cannot_be_reused_with_changed_context() {
    let s = store("source-reuse");
    hook(&s, None);
    s.receive_hook_text(event("1", "候補"), 40).unwrap();
    let mut changed = event("2", "誤った出所");
    changed.source.context_u64 = "1".into();
    assert_eq!(
        s.receive_hook_text(changed, 41).unwrap_err().code,
        "source_identity_conflict"
    );
    let mut changed_event = event("1", "候補");
    changed_event.sequence_u64 = "3".into();
    assert_eq!(
        s.receive_hook_text(changed_event, 42).unwrap_err().code,
        "event_identity_conflict"
    );
}
#[test]
fn reused_task_operation_for_different_fixed_object_is_rejected() {
    let s = store("task-operation-conflict");
    configured(&s);
    let first = manual(&s, "first", "一");
    let second = manual(&s, "second", "二");
    s.prepare_tasks_for_operation(first, 1, "translation".into(), "same-op".into(), 40)
        .unwrap();
    assert_eq!(
        s.prepare_tasks_for_operation(second, 1, "translation".into(), "same-op".into(), 41)
            .unwrap_err()
            .code,
        "operation_conflict"
    );
}
#[test]
fn independent_model_tests_preserve_scope_and_staleness_without_history() {
    use app_core::domain::testing::*;
    let s = store("model-test-record");
    configured(&s);
    let draft = s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy storage fixture remains codec1/2").clone();
    let hash = scope_semantics_hash(&draft, "model", "model").unwrap();
    s.record_model_test(ModelTestRecordInput {
        test_id: "actual-test-metadata".into(),
        config_revision_id: draft.revision_id.clone(),
        connection_id: Some("connection".into()),
        model_id: Some("model".into()),
        binding_id: None,
        purpose: None,
        stage: "model".into(),
        request_semantics_hash: content_hash("test-plan-fixture"),
        input_scope: "sample".into(),
        unit_id: None,
        text_revision: None,
        input_summary: serde_json::json!({"sampleId":"greeting-v1","scopeSemanticsHash":hash}),
        response_summary: serde_json::json!({"responseHash":content_hash("fixture-response")}),
        output_validation: serde_json::json!({"nonempty":true}),
        state: "passed".into(),
        started_at_ms: 30,
        completed_at_ms: 40,
        error_code: None,
        error_stage: None,
        credentials: vec![],
    })
    .unwrap();
    let snap = s.snapshot().unwrap();
    assert!(snap.history.units.is_empty());
    assert!(!snap.configuration_tests[0].stale);
    assert_eq!(snap.configuration_tests[0].object_kind, "model");
    assert_eq!(snap.configuration_tests[0].object_id, "model");
    let mut c = draft.clone();
    c.models[0].display_name = "Renamed".into();
    command(
        &s,
        "rename",
        Command::SaveConfiguration {
            configuration: c.clone(),
            expected_head_revision: 0,
        },
    );
    assert!(!s.snapshot().unwrap().configuration_tests[0].stale);
    c.models[0].actual_model_id = Some("other-actual".into());
    command(
        &s,
        "actual-change",
        Command::SaveConfiguration {
            configuration: c,
            expected_head_revision: 0,
        },
    );
    assert!(s.snapshot().unwrap().configuration_tests[0].stale);
}
#[test]
fn context_word_task_freezes_actual_token_and_persists_selection() {
    let s = store("context-word");
    let mut c = config();
    let mut b = c.bindings[0].clone();
    b.binding_id = "word-binding".into();
    b.purpose = "context_word".into();
    b.output_mode = Some("plain_text".into());
    c.bindings.push(b);
    let r = command(
        &s,
        "save",
        Command::SaveConfiguration {
            configuration: c,
            expected_head_revision: 0,
        },
    );
    command(
        &s,
        "apply",
        Command::ApplyConfiguration {
            revision_id: r.result_id.unwrap(),
            expected_head_revision: 0,
        },
    );
    let uid = manual(&s, "m", "彼女と待つ");
    let token = LexicalToken {
        surface: "彼女".into(),
        lemma: "彼女".into(),
        reading: "かのじょ".into(),
        part_of_speech: "代名詞".into(),
        byte_start: 0,
        byte_end: 6,
        utf16_start: 0,
        utf16_end: 2,
        known: true,
    };
    s.set_tokens(
        TokenSnapshotInput {
            unit_id: uid.clone(),
            text_revision: 1,
            analysis_revision: "fixture-v1".into(),
            tokens: vec![token.clone()],
            resource: resource("language"),
        },
        30,
    )
    .unwrap();
    assert!(
        s.prepare_study_tasks(
            uid.clone(),
            1,
            "context_word".into(),
            None,
            "bad-word".into(),
            40
        )
        .is_err()
    );
    let tasks = s
        .prepare_study_tasks(uid, 1, "context_word".into(), Some(0), "word-op".into(), 40)
        .unwrap();
    assert_eq!(tasks[0].selected_token, Some(token));
    let mut result = completion(&tasks[0]);
    result.output_mode = "plain_text".into();
    result.text = "彼女在这句中是等待的共同对象。".into();
    s.complete_task(result, 50).unwrap();
    let snap = s.snapshot().unwrap();
    assert_eq!(snap.study_results.len(), 1);
    assert_eq!(snap.study_results[0].purpose, "context_word");
    assert_eq!(snap.study_results[0].selection_byte_start, Some(0));
    assert_eq!(snap.study_results[0].selection_byte_end, Some(6));
    assert!(snap.history.units[0].translations.is_empty());
}
#[test]
fn cache_hit_preserves_independent_history_and_current_authorization() {
    let s = store("cache");
    let mut c = config();
    c.bindings[0]
        .schedule
        .insert("cacheTtlMs".into(), serde_json::json!(1000));
    let r = command(
        &s,
        "save",
        Command::SaveConfiguration {
            configuration: c,
            expected_head_revision: 0,
        },
    );
    command(
        &s,
        "apply",
        Command::ApplyConfiguration {
            revision_id: r.result_id.unwrap(),
            expected_head_revision: 0,
        },
    );
    let uid = manual(&s, "first", "繰り返し");
    let first = s
        .prepare_tasks(uid, 1, "translation".into(), 40)
        .unwrap()
        .remove(0);
    assert!(first.cached_result.is_none());
    s.complete_task(completion(&first), 50).unwrap();
    let uid = manual(&s, "second", "繰り返し");
    let second = s
        .prepare_tasks(uid, 1, "translation".into(), 60)
        .unwrap()
        .remove(0);
    assert_eq!(second.cached_result.as_ref().unwrap().text, "翻译结果");
    s.validate_task_authority(second.task_id.clone()).unwrap();
    s.complete_task(completion(&second), 61).unwrap();
    let snap = s.snapshot().unwrap();
    assert_eq!(snap.history.units.len(), 2);
    assert!(snap.history.units.iter().all(|u| u.translations.len() == 1));
    let uid = manual(&s, "third", "繰り返し");
    let third = s
        .prepare_tasks(uid, 1, "translation".into(), 2000)
        .unwrap()
        .remove(0);
    assert!(third.cached_result.is_none());
    acceptance_evidence::record(
        "cache-semantic-authority",
        &[("STORAGE-13", 1)],
        serde_json::json!({"actual_test":"cache_hit_preserves_independent_history_and_current_authorization"}),
    );
}
#[test]
fn rust_derives_target_keys_and_rejects_relative_verified_paths() {
    let s = store("paths");
    let mut first = game();
    first.target.canonical_exe_path = Some("c:/GAMES/./folder/../game.exe".into());
    first.target.target_path_key = Some("forged-first".into());
    command(&s, "first", Command::SaveGame(Box::new(first)));
    let mut duplicate = game();
    duplicate.target.target_path_key = Some("different-forged-key".into());
    let failure = s
        .dispatch(
            CommandEnvelope {
                operation_id: "duplicate".into(),
                expected_revision: None,
                command: Command::SaveGame(Box::new(duplicate)),
            },
            30,
        )
        .unwrap_err();
    assert_eq!(failure.code, "duplicate_record");
    let mut invalid = game();
    invalid.target.canonical_exe_path = Some("game.exe".into());
    assert!(
        s.dispatch(
            CommandEnvelope {
                operation_id: "relative".into(),
                expected_revision: None,
                command: Command::SaveGame(Box::new(invalid))
            },
            30
        )
        .is_err()
    );
    assert_eq!(
        s.snapshot().unwrap().games[0]
            .target
            .target_path_key
            .as_deref(),
        Some("c:\\games\\game.exe")
    );
}
#[test]
fn same_hook_session_reuses_identity_and_language_comes_from_declared_game() {
    let s = store("reuse-session");
    let mut input = game();
    input.source_language = Some("zh-Hans".into());
    let gid = command(&s, "game", Command::SaveGame(Box::new(input)))
        .result_id
        .unwrap();
    hook(&s, Some(gid.clone()));
    let revision = s.snapshot().unwrap().commit_revision;
    hook(&s, Some(gid));
    assert_eq!(s.snapshot().unwrap().commit_revision, revision);
    s.receive_hook_text(event("1", "候補"), 40).unwrap();
    command(
        &s,
        "select",
        Command::SelectSource {
            source_id: "source-1".into(),
        },
    );
    let mut text = event("2", "中文台词");
    text.source_language = "ja".into();
    let id = s.receive_hook_text(text, 41).unwrap().unwrap();
    assert_eq!(s.unit(id, 1).unwrap().source_language, "zh-Hans");
}
#[test]
fn manual_input_does_not_replace_active_capture_session_or_source_selection() {
    let s = store("manual-hook-isolation");
    hook(&s, None);
    s.receive_hook_text(event("1", "候補"), 30).unwrap();
    command(
        &s,
        "select",
        Command::SelectSource {
            source_id: "source-1".into(),
        },
    );
    manual(&s, "manual", "手入力");
    let snapshot = s.snapshot().unwrap();
    let session = snapshot.session.as_ref().unwrap();
    assert_eq!(session.session_id, "hook-session");
    assert_eq!(session.source_kind, "hook");
    assert_eq!(session.sources[0].source_id, "source-1");
    assert!(session.sources[0].selected);
    assert!(
        snapshot
            .history
            .units
            .iter()
            .any(|u| u.source_text == "手入力")
    );
}
#[test]
fn unicode_malformed_windows_path_is_rejected_without_panicking() {
    for path in [
        "C漢",
        "C:relative.exe",
        "../relative.exe",
        "C:\\..\\escape.exe",
        "\\\\server",
        "C:\\file.exe:stream",
    ] {
        assert!(normalize_windows_path(path).is_err(), "{path}");
    }
    assert_eq!(
        normalize_windows_path("C:/ゲーム/作品.exe").unwrap().1,
        "c:\\ゲーム\\作品.exe"
    );
}
use app_core::domain::retention::*;
#[test]
fn raw_retention_drops_only_payload_and_history_clear_respects_protected_objects() {
    let p = temp("retention");
    let s = SqliteStore::open(&p, "profile-test", "server-test", 10).unwrap();
    hook(&s, None);
    s.receive_hook_text(event("1", "候補"), 30).unwrap();
    command(
        &s,
        "select",
        Command::SelectSource {
            source_id: "source-1".into(),
        },
    );
    let uid = s
        .receive_hook_text(event("2", "保持する原文"), 31)
        .unwrap()
        .unwrap();
    // This existing case exercises cleanup of completed history. Establish
    // that lifecycle boundary through the production flush, preserving its
    // original deletion/rollback assertions; open protection is tested above.
    s.flush_hook_capture(
        CaptureFlush {
            session_id: Some("hook-session".into()),
            reason: CaptureFlushReason::Shutdown,
        },
        32,
    )
    .unwrap();
    s.dispatch(
        CommandEnvelope {
            operation_id: "completed-history-disconnect".into(),
            expected_revision: None,
            command: Command::Disconnect,
        },
        33,
    )
    .unwrap();
    let before = s.snapshot().unwrap().commit_revision;
    let failure = s
        .maintain_storage(
            RetentionPolicy {
                max_rows_per_batch: 100,
                raw_payload_before_ms: Some(40),
                history_unit_ids: vec![uid.clone()],
                ..Default::default()
            },
            ProtectedObjects {
                unit_ids: vec![uid.clone()],
                ..Default::default()
            },
            "reject-protected".into(),
            50,
        )
        .unwrap_err();
    assert_eq!(failure.code, "reading_object_in_use");
    assert_eq!(s.snapshot().unwrap().commit_revision, before);
    let conn = rusqlite::Connection::open(&p).unwrap();
    assert_eq!(
        conn.query_row::<u32, _, _>("SELECT count(*) FROM raw_event_payloads", [], |r| r.get(0))
            .unwrap(),
        2
    );
    let r = s
        .maintain_storage(
            RetentionPolicy {
                max_rows_per_batch: 100,
                raw_payload_before_ms: Some(40),
                ..Default::default()
            },
            ProtectedObjects::default(),
            "payload-clean".into(),
            50,
        )
        .unwrap();
    assert_eq!(r.removed_raw_payloads, 2);
    assert_eq!(s.unit(uid.clone(), 1).unwrap().source_text, "保持する原文");
    assert_eq!(
        conn.query_row::<u32, _, _>("SELECT count(*) FROM raw_events", [], |r| r.get(0))
            .unwrap(),
        2
    );
    assert_eq!(
        conn.query_row::<u32, _, _>("SELECT count(*) FROM source_refs", [], |r| r.get(0))
            .unwrap(),
        1
    );
    let r = s
        .maintain_storage(
            RetentionPolicy {
                max_rows_per_batch: 100,
                history_unit_ids: vec![uid],
                ..Default::default()
            },
            ProtectedObjects::default(),
            "history-clean".into(),
            60,
        )
        .unwrap();
    assert_eq!(r.removed_history_units, 1);
    assert!(s.snapshot().unwrap().history.units.is_empty());
    acceptance_evidence::record(
        "retention-protected-provenance",
        &[("STORAGE-14", 1)],
        serde_json::json!({"actual_test":"raw_retention_drops_only_payload_and_history_clear_respects_protected_objects"}),
    );
}
#[test]
fn config_gc_retains_header_seal_tombstone_and_refuses_referenced_snapshots() {
    let p = temp("config-gc");
    let s = SqliteStore::open(&p, "profile-test", "server-test", 10).unwrap();
    let first = command(
        &s,
        "first",
        Command::SaveConfiguration {
            configuration: config(),
            expected_head_revision: 0,
        },
    )
    .result_id
    .unwrap();
    let second = command(
        &s,
        "second",
        Command::SaveConfiguration {
            configuration: config(),
            expected_head_revision: 0,
        },
    )
    .result_id
    .unwrap();
    let r = s
        .maintain_storage(
            RetentionPolicy {
                max_rows_per_batch: 100,
                config_revision_ids: vec![first.clone()],
                ..Default::default()
            },
            ProtectedObjects::default(),
            "gc-first".into(),
            40,
        )
        .unwrap();
    assert_eq!(r.collected_config_snapshots, 1);
    let conn = rusqlite::Connection::open(&p).unwrap();
    for table in [
        "config_revisions",
        "config_revision_seals",
        "config_gc_marks",
    ] {
        let sql = format!("SELECT count(*) FROM {table} WHERE revision_id=?1");
        assert_eq!(
            conn.query_row::<u32, _, _>(&sql, [&first], |r| r.get(0))
                .unwrap(),
            1
        );
    }
    assert_eq!(
        conn.query_row::<u32, _, _>(
            "SELECT count(*) FROM models WHERE revision_id=?1",
            [&first],
            |r| r.get(0)
        )
        .unwrap(),
        0
    );
    assert!(
        s.maintain_storage(
            RetentionPolicy {
                max_rows_per_batch: 100,
                config_revision_ids: vec![second],
                ..Default::default()
            },
            ProtectedObjects::default(),
            "gc-referenced".into(),
            50
        )
        .is_err()
    );
    assert!(
        s.maintain_storage(
            RetentionPolicy {
                max_rows_per_batch: 100,
                config_revision_ids: vec![first],
                ..Default::default()
            },
            ProtectedObjects::default(),
            "gc-first".into(),
            60
        )
        .unwrap()
        .replayed
    );
    acceptance_evidence::record(
        "sealed-config-gc",
        &[("STORAGE-22", 1)],
        serde_json::json!({"actual_test":"config_gc_retains_header_seal_tombstone_and_refuses_referenced_snapshots"}),
    );
}
fn second_credential() -> CredentialReference {
    let mut c = credential();
    c.credential_id = "credential-2".into();
    c.credential_version_id = "vault-version-2".into();
    c.pool_id = "pool-2".into();
    c
}
fn pool_input(policy: &str) -> CredentialPoolInput {
    CredentialPoolInput {
        pool_id: Some("custom-pool".into()),
        expected_revision: 0,
        policy: policy.into(),
        metadata: BTreeMap::new(),
        members: vec![
            CredentialPoolMemberInput {
                member_id: None,
                credential_id: "credential-1".into(),
                enabled: true,
                weight: 2,
            },
            CredentialPoolMemberInput {
                member_id: None,
                credential_id: "credential-2".into(),
                enabled: true,
                weight: 1,
            },
        ],
    }
}
#[test]
fn pool_round_robin_weighting_and_attempt_credential_are_exact_and_versioned() {
    let s = store("pool");
    for c in [credential(), second_credential()] {
        s.begin_credential(c.clone(), 10).unwrap();
        s.activate_credential(c.credential_id, 11).unwrap();
    }
    command(
        &s,
        "rr",
        Command::SaveCredentialPool {
            pool: pool_input("round_robin"),
        },
    );
    assert_eq!(
        s.select_credential("custom-pool".into(), 1)
            .unwrap()
            .credential_id,
        "credential-1"
    );
    assert_eq!(
        s.select_credential("custom-pool".into(), 1)
            .unwrap()
            .credential_id,
        "credential-2"
    );
    let mut weighted = pool_input("weighted");
    weighted.expected_revision = 1;
    command(
        &s,
        "weighted",
        Command::SaveCredentialPool { pool: weighted },
    );
    let actual = (0..3)
        .map(|_| {
            s.select_credential("custom-pool".into(), 2)
                .unwrap()
                .credential_id
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, ["credential-1", "credential-1", "credential-2"]);
    let mut c = config();
    c.connections[0].auth_mode = "api_key".into();
    c.connections[0].credential_pool_id = Some("custom-pool".into());
    c.connections[0].credential_pool_revision = Some(1);
    let r = command(
        &s,
        "save",
        Command::SaveConfiguration {
            configuration: c,
            expected_head_revision: 0,
        },
    );
    command(
        &s,
        "apply",
        Command::ApplyConfiguration {
            revision_id: r.result_id.unwrap(),
            expected_head_revision: 0,
        },
    );
    let uid = manual(&s, "m", "原文");
    let t = s
        .prepare_tasks(uid, 1, "translation".into(), 40)
        .unwrap()
        .remove(0);
    assert_eq!(t.credential_vault_ref.as_deref(), Some("vault-version-1"));
    s.validate_task_authority(t.task_id).unwrap();
    assert_eq!(
        s.snapshot()
            .unwrap()
            .credential_pools
            .iter()
            .filter(|p| p.pool_id == "custom-pool")
            .count(),
        2
    );
    assert!(
        s.dispatch(
            CommandEnvelope {
                operation_id: "stale-pool".into(),
                expected_revision: None,
                command: Command::SaveCredentialPool {
                    pool: pool_input("round_robin")
                }
            },
            50
        )
        .is_err()
    );
}
#[test]
fn retention_executes_bounded_batches_and_reports_remaining_work() {
    let s = store("bounded-retention");
    hook(&s, None);
    for n in 1..=4 {
        s.receive_hook_text(event(&n.to_string(), "候補"), 30 + n)
            .unwrap();
    }
    s.dispatch(
        CommandEnvelope {
            operation_id: "bounded-retention-disconnect".into(),
            expected_revision: None,
            command: Command::Disconnect,
        },
        40,
    )
    .unwrap();
    let policy = RetentionPolicy {
        max_rows_per_batch: 1,
        raw_payload_before_ms: Some(100),
        ..Default::default()
    };
    let first = s
        .maintain_storage(
            policy.clone(),
            ProtectedObjects::default(),
            "batch-1".into(),
            100,
        )
        .unwrap();
    assert_eq!(first.removed_raw_payloads, 1);
    assert!(first.more_work);
    for n in 2..=4 {
        let result = s
            .maintain_storage(
                policy.clone(),
                ProtectedObjects::default(),
                format!("batch-{n}"),
                100,
            )
            .unwrap();
        assert_eq!(result.removed_raw_payloads, 1);
        assert_eq!(result.more_work, n != 4);
    }
}
#[test]
fn active_tasks_are_not_lost_when_more_than_100_newer_tasks_complete() {
    let s = store("active-projection");
    configured(&s);
    let old_unit = manual(&s, "old-unit", "古い原文");
    let pending = s
        .prepare_tasks(old_unit, 1, "translation".into(), 30)
        .unwrap()
        .remove(0);
    let unit = manual(&s, "new-unit", "新しい原文");
    for n in 0..105 {
        let task = s
            .prepare_tasks_for_operation(
                unit.clone(),
                1,
                "translation".into(),
                format!("operation-{n}"),
                40 + n * 2,
            )
            .unwrap()
            .remove(0);
        s.complete_task(completion(&task), 41 + n * 2).unwrap();
    }
    let snapshot = s.snapshot().unwrap();
    assert!(
        snapshot
            .tasks
            .iter()
            .any(|t| t.task_id == pending.task_id && t.status == "running")
    );
    assert_eq!(
        snapshot
            .tasks
            .iter()
            .filter(|t| t.status == "completed")
            .count(),
        100
    );
    s.validate_task_authority(pending.task_id).unwrap();
}
#[test]
fn confirmed_window_updates_real_target_and_preserves_launcher_arguments() {
    let s = store("confirmed-target");
    let mut input = game();
    input.target.canonical_exe_path = None;
    input.target.target_path_key = None;
    input.target.verification_state = "unresolved".into();
    input.target.launch_path = Some("C:\\launcher.exe".into());
    input.target.arguments = vec!["--safe-option".into()];
    let gid = command(&s, "add", Command::SaveGame(Box::new(input)))
        .result_id
        .unwrap();
    hook(&s, Some(gid));
    let game = s.snapshot().unwrap().games.remove(0);
    assert_eq!(
        game.target.canonical_exe_path.as_deref(),
        Some("C:\\game.exe")
    );
    assert_eq!(game.target.target_path_key.as_deref(), Some("c:\\game.exe"));
    assert_eq!(game.target.launch_path.as_deref(), Some("C:\\launcher.exe"));
    assert_eq!(game.target.arguments, ["--safe-option"]);
    assert_eq!(game.target.verification_state, "verified");
    assert_eq!(game.row_revision, 2);
}
#[test]
fn confirmed_target_conflict_rolls_back_session_and_library_mutations() {
    let s = store("target-conflict");
    let mut first = game();
    first.target.canonical_exe_path = Some("C:\\game.exe".into());
    let first = command(&s, "first", Command::SaveGame(Box::new(first)))
        .result_id
        .unwrap();
    let mut second = game();
    second.display_name = "Another entry".into();
    second.target.canonical_exe_path = Some("C:\\other.exe".into());
    let second = command(&s, "second", Command::SaveGame(Box::new(second)))
        .result_id
        .unwrap();
    let before = s.snapshot().unwrap().commit_revision;
    let result = s.open_hook_session(
        HookSessionInput {
            session_id: "conflicting-session".into(),
            game_id: Some(second.clone()),
            environment_key: "crossover:/private/bottle".into(),
            target_exe_path: "C:\\game.exe".into(),
            windows_pid: 100,
            process_created_u64: "100".into(),
            architecture: "x86".into(),
            broker_instance_id: "broker".into(),
            resource: resource("core"),
            target_identity_sha256: content_hash("target"),
        },
        40,
    );
    assert!(result.is_err());
    let snapshot = s.snapshot().unwrap();
    assert_eq!(snapshot.commit_revision, before);
    assert!(snapshot.session.is_none());
    assert_eq!(
        snapshot
            .games
            .iter()
            .find(|g| g.game_id == second)
            .unwrap()
            .target
            .canonical_exe_path
            .as_deref(),
        Some("C:\\other.exe")
    );
    assert!(snapshot.games.iter().any(|g| g.game_id == first));
}
#[test]
fn capture_lifecycle_is_opening_until_connected_and_terminal_stays_visible() {
    let s = store("capture-lifecycle");
    hook(&s, None);
    assert_eq!(s.snapshot().unwrap().session.unwrap().state, "opening");
    s.record_diagnostic(
        Diagnostic {
            code: "capture_connected".into(),
            stage: "capture".into(),
            message: "Connected".into(),
            recoverable: true,
        },
        40,
    )
    .unwrap();
    assert_eq!(s.snapshot().unwrap().session.unwrap().state, "active");
    s.record_diagnostic(
        Diagnostic {
            code: "game_disconnected".into(),
            stage: "capture".into(),
            message: "Exited".into(),
            recoverable: true,
        },
        50,
    )
    .unwrap();
    assert_eq!(s.snapshot().unwrap().session.unwrap().state, "stopped");
    s.record_diagnostic(
        Diagnostic {
            code: "capture_connected".into(),
            stage: "capture".into(),
            message: "Late ready".into(),
            recoverable: true,
        },
        60,
    )
    .unwrap();
    assert_eq!(s.snapshot().unwrap().session.unwrap().state, "stopped");
    assert!(s.receive_hook_text(event("1", "遅い結果"), 70).is_err());
}
#[test]
fn first_real_text_can_activate_an_opening_hook_without_fabricating_ready() {
    let s = store("text-lifecycle");
    hook(&s, None);
    assert_eq!(s.snapshot().unwrap().session.unwrap().state, "opening");
    s.receive_hook_text(event("1", "実際の候補"), 40).unwrap();
    assert_eq!(s.snapshot().unwrap().session.unwrap().state, "active");
    s.record_diagnostic(
        Diagnostic {
            code: "capture_failed".into(),
            stage: "capture".into(),
            message: "Broker died".into(),
            recoverable: true,
        },
        50,
    )
    .unwrap();
    assert_eq!(s.snapshot().unwrap().session.unwrap().state, "failed");
    assert!(s.receive_hook_text(event("2", "拒否"), 60).is_err());
}
#[test]
fn abi_ready_is_not_game_connected_and_new_hook_closes_prior_capture_state() {
    let s = store("abi-not-ready");
    hook(&s, None);
    s.record_diagnostic(
        Diagnostic {
            code: "broker_ready".into(),
            stage: "broker".into(),
            message: "ABI handshake only".into(),
            recoverable: true,
        },
        31,
    )
    .unwrap();
    assert_eq!(
        s.snapshot().unwrap().session.as_ref().unwrap().state,
        "opening"
    );
    s.open_hook_session(
        HookSessionInput {
            session_id: "new-session".into(),
            game_id: None,
            environment_key: "crossover:/private/bottle".into(),
            target_exe_path: "C:\\new.exe".into(),
            windows_pid: 101,
            process_created_u64: "101".into(),
            architecture: "x86".into(),
            broker_instance_id: "new-broker".into(),
            resource: resource("core"),
            target_identity_sha256: content_hash("new-target"),
        },
        40,
    )
    .unwrap();
    let session = s.snapshot().unwrap().session.unwrap();
    assert_eq!(session.session_id, "new-session");
    assert_eq!(session.state, "opening");
    assert!(
        s.receive_hook_text(event("1", "古いセッション"), 41)
            .is_err()
    );
}

#[test]
fn changed_game_glossary_invalidates_inherited_work_without_cancelling_other_scopes() {
    let s = store("game-glossary-inheritance");
    let gid = command(&s, "add-game", Command::SaveGame(Box::new(game())))
        .result_id
        .unwrap();
    configured(&s);
    hook(&s, Some(gid.clone()));
    s.receive_hook_text(event("1", "候補"), 40).unwrap();
    command(
        &s,
        "select",
        Command::SelectSource {
            source_id: "source-1".into(),
        },
    );
    let unit = s
        .receive_hook_text(event("2", "図書館に行く"), 41)
        .unwrap()
        .unwrap();
    let task = s
        .prepare_tasks(unit, 1, "translation".into(), 42)
        .unwrap()
        .remove(0);
    let manual_unit = manual(&s, "manual-other-scope", "図書館");
    let manual_task = s
        .prepare_tasks(manual_unit, 1, "translation".into(), 43)
        .unwrap()
        .remove(0);
    let mut configuration = s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy storage fixture remains codec1/2").clone();
    configuration.term_sets.push(TermSet {
        term_set_id: "game-glossary".into(),
        scope_key: format!("game:{gid}"),
        game_id: Some(gid),
        display_name: "游戏词汇".into(),
        enabled: true,
        terms: vec![Term {
            term_id: "library-term".into(),
            source_text: Some("図書館".into()),
            target_text: Some("图书馆".into()),
            source_language: Some("ja".into()),
            target_language: Some("zh-Hans".into()),
            enabled: true,
        }],
    });
    let revision = command(
        &s,
        "save-terms",
        Command::SaveConfiguration {
            configuration,
            expected_head_revision: 0,
        },
    )
    .result_id
    .unwrap();
    command(
        &s,
        "apply-terms",
        Command::ApplyConfiguration {
            revision_id: revision,
            expected_head_revision: 0,
        },
    );
    assert!(s.validate_task_authority(task.task_id.clone()).is_err());
    assert!(s.complete_task(completion(&task), 60).is_err());
    s.complete_task(completion(&manual_task), 61).unwrap();
    assert_eq!(s.unit(task.unit.unit_id, 1).unwrap().translations.len(), 0);
}

#[test]
fn global_parameter_changes_invalidate_game_override_that_inherits_them() {
    let s = store("global-parameters-inherited-by-game");
    let gid = command(&s, "add-game", Command::SaveGame(Box::new(game())))
        .result_id
        .unwrap();
    let mut c = config();
    let mut game_binding = c.bindings[0].clone();
    game_binding.binding_id = "game-translation".into();
    game_binding.scope_key = format!("game:{gid}");
    game_binding.game_id = Some(gid.clone());
    game_binding.engines[0].engine_id = "game-engine".into();
    c.bindings.push(game_binding);
    c.bindings[0].parameters.insert(
        "max_tokens".into(),
        Parameter {
            mode: ParameterMode::Set,
            value: Some(serde_json::json!(100)),
        },
    );
    c.bindings[0].engines[0].parameters.insert(
        "temperature".into(),
        Parameter {
            mode: ParameterMode::Set,
            value: Some(serde_json::json!(0.2)),
        },
    );
    let r = command(
        &s,
        "save",
        Command::SaveConfiguration {
            configuration: c,
            expected_head_revision: 0,
        },
    );
    command(
        &s,
        "apply",
        Command::ApplyConfiguration {
            revision_id: r.result_id.unwrap(),
            expected_head_revision: 0,
        },
    );
    hook(&s, Some(gid));
    s.receive_hook_text(event("1", "候補"), 40).unwrap();
    command(
        &s,
        "select",
        Command::SelectSource {
            source_id: "source-1".into(),
        },
    );
    let uid = s
        .receive_hook_text(event("2", "継承する"), 41)
        .unwrap()
        .unwrap();
    for (index, layer) in ["binding", "engine"].into_iter().enumerate() {
        let task = s
            .prepare_tasks(uid.clone(), 1, "translation".into(), 42 + index as i64)
            .unwrap()
            .remove(0);
        let mut next = s.snapshot().unwrap().configuration.draft.as_legacy().expect("legacy storage fixture remains codec1/2").clone();
        if layer == "binding" {
            next.bindings[0].parameters.insert(
                "max_tokens".into(),
                Parameter {
                    mode: ParameterMode::Set,
                    value: Some(serde_json::json!(200)),
                },
            );
        } else {
            next.bindings[0].engines[0].parameters.insert(
                "temperature".into(),
                Parameter {
                    mode: ParameterMode::Set,
                    value: Some(serde_json::json!(0.4)),
                },
            );
        }
        let r = command(
            &s,
            &format!("save-{layer}"),
            Command::SaveConfiguration {
                configuration: next,
                expected_head_revision: 0,
            },
        );
        command(
            &s,
            &format!("apply-{layer}"),
            Command::ApplyConfiguration {
                revision_id: r.result_id.unwrap(),
                expected_head_revision: 0,
            },
        );
        assert!(
            s.validate_task_authority(task.task_id.clone()).is_err(),
            "{layer}"
        );
        assert!(s.complete_task(completion(&task), 60).is_err(), "{layer}");
    }
}

#[test]
fn hud_preference_patches_preserve_other_fields_replay_and_reopen() {
    let path = temp("hud-preference-patches");
    let store = SqliteStore::open(&path, "hud-pref", "first", 1).unwrap();
    let mut expected = Preferences {
        theme: "light".into(),
        transparency_mode: "reduced".into(),
        source_font_pt: 27.,
        translation_font_pt: 33.,
        hud_width_pt: 707.,
        shortcuts: BTreeMap::from([("toggleHud".into(), "CmdOrCtrl+Alt+H".into())]),
        overrides: BTreeMap::from([("unrelated".into(), true)]),
        ..Preferences::default()
    };
    let original = command(
        &store,
        "hud-original",
        Command::SavePreferences {
            preferences: expected.clone(),
        },
    );
    let variants = [
        (
            HudPreference::Segmentation,
            "reading.segmentationEnabled",
            false,
        ),
        (HudPreference::Compact, "hud.compact", true),
        (HudPreference::AlwaysOnTop, "hud.alwaysOnTop", false),
    ];
    for (index, (preference, key, enabled)) in variants.into_iter().enumerate() {
        let operation = format!("hud-patch-{index}");
        let patch = Command::SetHudPreference {
            preference,
            enabled,
        };
        let result = command(&store, &operation, patch.clone());
        if key == "reading.segmentationEnabled" {
            expected.segmentation_enabled = enabled;
        } else {
            expected.overrides.insert(key.into(), enabled);
        }
        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.preferences, expected);
        assert!(result.revision > original.revision);
        let replay = command(&store, &operation, patch);
        assert!(replay.replayed);
        assert_eq!(replay.result_id, result.result_id);
        assert_eq!(
            store.snapshot().unwrap().commit_revision,
            snapshot.commit_revision
        );
    }
    // Each typed preference can change its own key, not a key supplied by UI.
    assert!(serde_json::from_value::<Command>(serde_json::json!({"kind":"set_hud_preference","payload":{"preference":"theme","enabled":false}})).is_err());
    let reader =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let original_overrides: String = reader
        .query_row(
            "SELECT overrides_json FROM preference_revisions WHERE preference_revision_id=?1",
            [original.result_id.unwrap()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<BTreeMap<String, bool>>(&original_overrides).unwrap(),
        BTreeMap::from([("unrelated".into(), true)])
    );
    drop(reader);
    drop(store);
    let reopened = SqliteStore::open(&path, "hud-pref", "second", 10).unwrap();
    assert_eq!(reopened.snapshot().unwrap().preferences, expected);
}
