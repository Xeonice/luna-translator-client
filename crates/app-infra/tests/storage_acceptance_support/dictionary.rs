//! Real offline dictionary producer and typed, source-derived field evidence.
//! Never inserts test definitions through SQL. The unchanged packaging builder
//! reads retained, hash-checked source snapshots into this run's private path.
use app_core::domain::content_hash;
use app_infra::dictionary::Dictionary;
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn proof(table: &str, columns: &[&str], assertions: usize, source: &str) -> Vec<Value> {
    assert!(assertions > 0);
    columns.iter().map(|column|json!({"id":format!("RUST-MAP:dictionary.{table}.{column}"),"status":"passed","assertions":assertions,"skipped":0,"source":source})).collect()
}
fn normalize(text: &str) -> String {
    text.trim()
        .chars()
        .map(|c| {
            if ('\u{30a1}'..='\u{30f6}').contains(&c) {
                char::from_u32(c as u32 - 0x60).unwrap()
            } else {
                c
            }
        })
        .collect()
}
pub fn build_and_verify_dictionary(output: &Path) -> (PathBuf, Vec<Value>) {
    fs::create_dir_all(output).unwrap();
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    // Import the actual producer unchanged; only its destination is redirected.
    let script = r#"import importlib.util,json,sys
from pathlib import Path
root=Path(sys.argv[1]);out=Path(sys.argv[2]);spec=importlib.util.spec_from_file_location('production_dictionary_builder',root/'scripts/resources/prepare_language_resources.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);m.LANG=out;m.LANG.mkdir(parents=True,exist_ok=True);p=m.build_dictionary(True);(out/'builder-source-witness.json').write_text(json.dumps(p,ensure_ascii=False),encoding='utf-8')
"#;
    let run = Command::new("python3")
        .args(["-c", script])
        .arg(root())
        .arg(output)
        .output()
        .unwrap();
    fs::write(output.join("builder.stdout"), &run.stdout).unwrap();
    fs::write(output.join("builder.stderr"), &run.stderr).unwrap();
    assert!(
        run.status.success(),
        "real dictionary builder failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let path = output.join("dictionary.sqlite3");
    assert!(path.is_file());
    for suffix in ["-wal", "-shm"] {
        assert!(
            !PathBuf::from(format!("{}{suffix}", path.display())).exists(),
            "dictionary producer must close database before evidence inspection"
        );
    }
    let bytes_before = content_hash(fs::read(&path).unwrap());
    let uri = format!("file:{}?immutable=1", path.display());
    let conn = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .unwrap();
    conn.execute_batch("PRAGMA query_only=ON").unwrap();
    let provenance: Value =
        serde_json::from_slice(&fs::read(output.join("dictionary-provenance.json")).unwrap())
            .unwrap();
    let witness: Value =
        serde_json::from_slice(&fs::read(output.join("builder-source-witness.json")).unwrap())
            .unwrap();
    assert_eq!(provenance, witness);
    let (singleton,version,manifest,schema,normalization,built):(u32,String,String,u32,String,i64)=conn.query_row("SELECT singleton,resource_version,manifest_sha256,index_schema_version,normalization_version,built_at_ms FROM dictionary_meta",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap();
    assert_eq!(singleton, 1);
    assert_eq!(
        Some(version.as_str()),
        provenance["resource_version"].as_str()
    );
    assert_eq!(
        manifest,
        content_hash(fs::read(output.join("dictionary-provenance.json")).unwrap())
    );
    assert_eq!(schema, 1);
    assert_eq!(normalization, "kana-hiragana-exact-v1");
    assert!(
        built >= start
            && built
                <= SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64
    );
    assert_eq!(
        provenance["production_schema_sha256"],
        content_hash(
            fs::read(root().join("crates/app-infra/src/storage/migrations/001_dictionary.sql"))
                .unwrap()
        )
    );
    let mut checks = proof(
        "dictionary_meta",
        &[
            "singleton",
            "resource_version",
            "manifest_sha256",
            "index_schema_version",
            "normalization_version",
            "built_at_ms",
        ],
        1,
        "production build_dictionary: closed nonempty index metadata, provenance exact bytes/hash, schema source identity, bounded actual build timestamp",
    );
    let mut sources = BTreeSet::new();
    let mut q=conn.prepare("SELECT source_id,display_name,snapshot_version,source_sha256,source_url,license_ref,attribution_text FROM dictionary_sources").unwrap();
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        })
        .unwrap();
    let mut count = 0;
    for row in rows {
        let (id, name, version, sha, url, license, attribution) = row.unwrap();
        let expected = provenance["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["source_id"] == id)
            .unwrap();
        for (field, actual) in [
            ("source_id", id.as_str()),
            ("display_name", name.as_str()),
            ("snapshot_version", version.as_str()),
            ("source_sha256", sha.as_str()),
            ("source_url", url.as_str()),
            ("license_ref", license.as_str()),
            ("attribution_text", attribution.as_str()),
        ] {
            assert_eq!(
                Some(actual),
                expected[field].as_str(),
                "source lineage {id}.{field}"
            );
        }
        assert!(root().join("resources").join(&license).is_file());
        assert!(sources.insert(id));
        count += 1;
    }
    assert_eq!(count, 3);
    checks.extend(proof("dictionary_sources",&["source_id","display_name","snapshot_version","source_sha256","source_url","license_ref","attribution_text"],count,"real retained Wiktionary/JMdict source hash verification + exact producer provenance relation and packaged license existence"));
    let mut entries = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    let mut q=conn.prepare("SELECT entry_id,source_id,source_entry_key,lemma,reading_kana,pos_json,content_kind FROM dictionary_entries").unwrap();
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        })
        .unwrap();
    let mut count = 0;
    for row in rows {
        let (id, source, key, lemma, reading, pos, kind) = row.unwrap();
        assert!(sources.contains(&source));
        assert!(!key.is_empty() && !lemma.is_empty());
        assert_eq!(id, content_hash(format!("{source}\0{key}")));
        let pos: Vec<String> = serde_json::from_str(&pos).unwrap();
        assert!(pos.iter().all(|p| !p.is_empty()));
        if let Some(reading) = reading {
            assert!(!reading.is_empty());
        }
        assert_eq!(
            kind,
            if source == "jmdict-en" {
                "en_supplement"
            } else {
                "zh_base"
            }
        );
        kinds.insert(kind);
        assert!(entries.insert(id));
        count += 1;
    }
    assert!(count > 100_000);
    assert_eq!(kinds.len(), 2);
    checks.extend(proof("dictionary_entries",&["entry_id","source_id","source_entry_key","lemma","reading_kana","pos_json","content_kind"],count,"real nonempty producer rows typed decode + exact source-key SHA identity, source/kind/nullable reading/JSON-array invariants; independent raw-source lineage verifier below"));
    let mut q=conn.prepare("SELECT form_id,entry_id,form_text,normalized_key,form_kind,rank_weight FROM dictionary_forms").unwrap();
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, u32>(5)?,
            ))
        })
        .unwrap();
    let mut count = 0;
    for row in rows {
        let (id, entry, text, key, kind, rank) = row.unwrap();
        assert!(entries.contains(&entry));
        assert_eq!(key, normalize(&text));
        assert!(!key.is_empty());
        assert_eq!(id, content_hash(format!("{entry}\0{key}\0{kind}")));
        assert_eq!(
            rank,
            match kind.as_str() {
                "lemma" => 0,
                "surface" => 1,
                "reading" => 2,
                _ => panic!("unregistered form kind"),
            }
        );
        count += 1;
    }
    assert!(count > entries.len());
    checks.extend(proof("dictionary_forms",&["form_id","entry_id","form_text","normalized_key","form_kind","rank_weight"],count,"all actual builder forms typed decode, independent kana normalizer, exact content-addressed form identity, rank and real entry relationship; source-lineage crosscheck below"));
    let mut q=conn.prepare("SELECT s.entry_id,s.sense_index,s.gloss_language,s.gloss_text,s.usage_notes,s.source_locator,e.content_kind FROM dictionary_senses s JOIN dictionary_entries e USING(entry_id) ORDER BY s.entry_id,s.sense_index").unwrap();
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, u32>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        })
        .unwrap();
    let mut count = 0;
    let mut previous = String::new();
    let mut ordinal = 0;
    for row in rows {
        let (entry, index, language, gloss, notes, url, kind) = row.unwrap();
        assert!(entries.contains(&entry));
        if previous != entry {
            ordinal = 0;
            previous = entry;
        }
        assert_eq!(index, ordinal);
        ordinal += 1;
        assert_eq!(language, if kind == "zh_base" { "zh" } else { "en" });
        assert!(!gloss.trim().is_empty());
        assert!(url.starts_with("https://"));
        if kind == "zh_base" {
            let notes: Value = serde_json::from_str(notes.as_deref().unwrap()).unwrap();
            assert!(notes["tags"].is_array());
        } else {
            assert!(notes.is_none());
        }
        count += 1;
    }
    assert!(count > entries.len());
    checks.extend(proof("dictionary_senses",&["entry_id","sense_index","gloss_language","gloss_text","usage_notes","source_locator"],count,"all real source senses typed decode, contiguous ordinal, entry/language relationship and nullable structured Wiktionary notes; exact independent raw-source comparison below"));
    assert_eq!(
        conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(
        conn.prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query([])
            .unwrap()
            .next()
            .unwrap()
            .is_none()
    );
    assert!(conn.execute("DELETE FROM dictionary_entries", []).is_err());
    assert_eq!(content_hash(fs::read(&path).unwrap()), bytes_before);
    // Compare every persisted lexical value against source records, rather than
    // accepting internally consistent but fabricated entries/glosses. No SQL write.
    let lineage = r#"import sqlite3,json,sys,hashlib
from pathlib import Path
from urllib.parse import quote
root=Path(sys.argv[1]);out=Path(sys.argv[2]);db=sqlite3.connect('file:'+str(out/'dictionary.sqlite3')+'?mode=ro',uri=True);db.execute('PRAGMA query_only=ON');count=0
def norm(x):return ''.join(chr(ord(c)-96) if '\u30a1'<=c<='\u30f6' else c for c in x.strip())
def check(source,key,lemma,reading,pos,kind,forms,senses):
 global count
 if not lemma or not senses:return
 identity=hashlib.sha256((source+'\0'+key).encode()).hexdigest();row=db.execute('SELECT source_id,source_entry_key,lemma,reading_kana,pos_json,content_kind FROM dictionary_entries WHERE entry_id=?',(identity,)).fetchone();assert row is not None
 assert (row[0],row[1],row[2],row[3],json.loads(row[4]),row[5])==(source,key,lemma,reading,pos,kind),(source,key)
 expected={(lemma,'lemma',0)}|{(x,'surface',1) for x in forms};expected|={(reading,'reading',2)} if reading else set();dedup={}
 for x,k,rank in [(lemma,'lemma',0)]+[(x,'surface',1) for x in forms]+([(reading,'reading',2)] if reading else []):
  n=norm(x)
  if n:dedup.setdefault((n,k),(x,n,k,rank))
 actual=db.execute('SELECT form_text,normalized_key,form_kind,rank_weight FROM dictionary_forms WHERE entry_id=?',(identity,)).fetchall();assert set(actual)==set(dedup.values())
 actual=db.execute('SELECT gloss_language,gloss_text,usage_notes,source_locator FROM dictionary_senses WHERE entry_id=? ORDER BY sense_index',(identity,)).fetchall();assert actual==senses
 count+=1
for variant in ['simplified','traditional']:
 for number,line in enumerate((root/'.build/language-downloads'/('zh-ja-'+variant+'.jsonl')).open(),1):
  e=json.loads(line)
  if e.get('lang_code')!='ja':continue
  lemma=e.get('word','');reading=None
  for f in e.get('forms',[]):
   if f.get('ruby') and f.get('form')==lemma:reading=''.join(p[1] for p in f['ruby'] if len(p)==2) or None;break
  senses=[];ids=[];url='https://zh.wiktionary.org/wiki/'+quote(lemma,safe='')+'#日語'
  for s in e.get('senses',[]):
   if s.get('id'):ids.append(s['id'])
   notes=json.dumps({'tags':s.get('tags',[]),'source_sense_id':s.get('id')},ensure_ascii=False)
   senses.extend(('zh',g,notes,url) for g in s.get('glosses',[]) if isinstance(g,str) and g.strip())
  check('zhwiktionary-'+variant,str(number)+':'+','.join(ids),lemma,reading,[e.get('pos_title') or e.get('pos','unknown')],'zh_base',[f.get('form','') for f in e.get('forms',[]) if f.get('form')],senses)
for line in (root/'.build/language-downloads/jmdict.jsonl').open():
 e=json.loads(line);heads=e.get('headwords',[]);reads=e.get('readings',[]);lemma=next(iter(heads or reads),'');url='https://www.edrdg.org/jmdict/edict_doc.html#'+quote(e['id'],safe='');senses=[('en',g,None,url) for g in e.get('glosses',[]) if isinstance(g,str) and g.strip()];check('jmdict-en',e['id'],lemma,next(iter(reads),None),[],'en_supplement',heads+reads,senses)
assert count==db.execute('SELECT count(*) FROM dictionary_entries').fetchone()[0];print(json.dumps({'all_source_entries_compared':count,'mode':'read_only_exact_source_to_index'}));db.close()
"#;
    let run = Command::new("python3")
        .args(["-c", lineage])
        .arg(root())
        .arg(output)
        .output()
        .unwrap();
    fs::write(output.join("source-lineage.stdout"), &run.stdout).unwrap();
    fs::write(output.join("source-lineage.stderr"), &run.stderr).unwrap();
    assert!(
        run.status.success(),
        "independent source lineage rejected actual builder: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(content_hash(fs::read(&path).unwrap()), bytes_before);
    // A fresh build has a fresh timestamp/hash. It cannot impersonate the frozen
    // compiled resource package; the actual compiled resource still opens/queries.
    let frozen = Dictionary::open(&root().join("resources")).unwrap();
    let results = frozen.lookup("図書館", "図書館", "トショカン").unwrap();
    assert!(
        results
            .iter()
            .any(|e| e.content_kind == "zh_base" && e.senses.iter().any(|s| s.language == "zh"))
    );
    assert!(
        results
            .iter()
            .any(|e| e.content_kind == "en_supplement"
                && e.senses.iter().all(|s| s.language == "en"))
    );
    assert!(
        results
            .iter()
            .all(|e| !e.license_ref.is_empty() && !e.attribution.is_empty())
    );
    assert!(frozen.resource_info().available);
    let frozen_info = frozen.resource_info();
    drop(frozen);
    verify_profile_link(output, &frozen_info);
    assert_eq!(checks.len(), 32);
    fs::write(
        output.join("field-evidence.json"),
        serde_json::to_vec_pretty(&checks).unwrap(),
    )
    .unwrap();
    (path, checks)
}
pub(super) fn verify_profile_link(output: &Path, dictionary: &app_core::contracts::ResourceInfo) {
    verify_profile_link_with_record(
        output,
        dictionary,
        Some("frozen-dictionary-profile-resource-link"),
    );
}
pub(super) fn verify_profile_link_with_record(
    output: &Path,
    dictionary: &app_core::contracts::ResourceInfo,
    case: Option<&str>,
) {
    use app_core::{
        contracts::{Command as ProductCommand, CommandEnvelope, LexicalToken},
        domain::capture::{ResourceSnapshotInput, TokenSnapshotInput},
        ports::ProductRepository,
    };
    use app_infra::storage::SqliteStore;
    let resources = root().join("resources");
    let manifest_bytes = fs::read(resources.join("language/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    let hash = content_hash(&manifest_bytes);
    let provenance = fs::read(resources.join("language/dictionary-provenance.json")).unwrap();
    assert_eq!(dictionary.manifest_sha256, content_hash(&provenance));
    assert_eq!(
        manifest["dictionary_provenance_sha256"],
        dictionary.manifest_sha256
    );
    assert_eq!(manifest["dictionary_version"], dictionary.version);
    let frozen_path = resources.join("language/dictionary.sqlite3");
    let frozen_hash = content_hash(fs::read(&frozen_path).unwrap());
    let frozen = Connection::open_with_flags(
        format!("file:{}?immutable=1", frozen_path.display()),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .unwrap();
    let meta: (String, String) = frozen
        .query_row(
            "SELECT resource_version,manifest_sha256 FROM dictionary_meta WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        meta,
        (
            dictionary.version.clone(),
            dictionary.manifest_sha256.clone()
        )
    );
    drop(frozen);
    let path = output.join("dictionary-consumer-profile.sqlite3");
    let store = SqliteStore::open(
        &path,
        "dictionary-link-profile",
        "dictionary-link-server",
        10,
    )
    .unwrap();
    let id = store
        .dispatch(
            CommandEnvelope {
                operation_id: "real-resource-link-source".into(),
                expected_revision: None,
                command: ProductCommand::ManualInput {
                    text: "図書館".into(),
                    source_language: "ja".into(),
                },
            },
            11,
        )
        .unwrap()
        .result_id
        .unwrap();
    let snapshot = format!("actual-language-{hash}");
    let metadata = json!({"languageManifestSha256":hash,"dictionaryVersion":dictionary.version,"dictionaryProvenanceSha256":dictionary.manifest_sha256,"dictionaryFileSha256":frozen_hash,"dictionaryResourceInfo":dictionary});
    store
        .set_tokens(
            TokenSnapshotInput {
                unit_id: id.clone(),
                text_revision: 1,
                analysis_revision: "controlled-resource-link-v1".into(),
                tokens: vec![LexicalToken {
                    surface: "図書館".into(),
                    lemma: "図書館".into(),
                    reading: "としょかん".into(),
                    part_of_speech: "名詞".into(),
                    byte_start: 0,
                    byte_end: 9,
                    utf16_start: 0,
                    utf16_end: 3,
                    known: true,
                }],
                resource: ResourceSnapshotInput {
                    snapshot_id: snapshot.clone(),
                    kind: "language".into(),
                    version: manifest["resource_version"].as_str().unwrap().into(),
                    manifest_sha256: hash.clone(),
                    contract_version: "language-v1".into(),
                    metadata_json: serde_json::to_string(&metadata).unwrap(),
                },
            },
            12,
        )
        .unwrap();
    assert_eq!(
        store.unit(id.clone(), 1).unwrap().tokens[0].surface,
        "図書館"
    );
    store.checkpoint().unwrap();
    drop(store);
    for (reopen, server, now) in [
        (false, "dictionary-link-server", 12),
        (true, "dictionary-link-reopened", 20),
    ] {
        let reopened = reopen
            .then(|| SqliteStore::open(&path, "dictionary-link-profile", server, now).unwrap());
        if let Some(s) = &reopened {
            assert_eq!(s.unit(id.clone(), 1).unwrap().tokens[0].lemma, "図書館");
            assert_eq!(s.snapshot().unwrap().profile_generation, 2);
        }
        let profile = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        profile.execute_batch("PRAGMA query_only=ON").unwrap();
        let actual:(String,String,String,String,String,String,String)=profile.query_row("SELECT l.unit_id,l.source_sha256,l.language_resource_snapshot_id,r.kind,r.version_label,r.manifest_sha256,r.metadata_json FROM lexical_snapshots l JOIN resource_snapshots r ON r.resource_snapshot_id=l.language_resource_snapshot_id WHERE l.unit_id=?1",[&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).unwrap();
        assert_eq!(actual.0, id);
        assert_eq!(actual.1, content_hash("図書館"));
        assert_eq!(actual.2, snapshot);
        assert_eq!(actual.3, "language");
        assert_eq!(
            Some(actual.4.as_str()),
            manifest["resource_version"].as_str()
        );
        assert_eq!(actual.5, hash);
        let stored: Value = serde_json::from_str(&actual.6).unwrap();
        assert_eq!(stored, metadata);
        assert_eq!(stored["dictionaryVersion"], dictionary.version);
        assert_eq!(
            stored["dictionaryProvenanceSha256"],
            dictionary.manifest_sha256
        );
        assert_eq!(stored["dictionaryFileSha256"], frozen_hash);
        assert_eq!(
            profile
                .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
                    .get::<_, u32>(
                    0
                ))
                .unwrap(),
            0
        );
        drop(profile);
        drop(reopened);
    }
    assert_eq!(content_hash(fs::read(&frozen_path).unwrap()), frozen_hash);
    assert_eq!(
        content_hash(fs::read(resources.join("language/manifest.json")).unwrap()),
        hash
    );
    let evidence = json!({"kind":"real_dictionary_language_profile_resource_chain","profile":"dictionary-consumer-profile.sqlite3","reading_unit_id":id,"language_snapshot_id":snapshot,"language_manifest_sha256":hash,"language_resource_version":manifest["resource_version"],"dictionary_version":dictionary.version,"dictionary_provenance_sha256":dictionary.manifest_sha256,"dictionary_file_sha256":frozen_hash,"source_hash":content_hash("図書館"),"fresh_and_reopen_assertions":32,"resource_kinds_preserved":true,"frozen_assets_unchanged":true,"scope":"controlled aligned token DTO tests persistence/resource identity; not a parser quality or live game claim"});
    fs::write(
        output.join("cross-database-resource-link.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    if let Some(case) = case {
        super::evidence::record(case, &[("STORAGE-16", 32)], evidence);
    }
}
