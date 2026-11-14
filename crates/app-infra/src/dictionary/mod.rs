//! Read-only offline dictionary, with source language and attribution preserved.
use app_core::contracts::{DictionaryEntry, DictionarySense, ResourceInfo};
use rusqlite::{Connection, OpenFlags, params};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path, sync::Mutex};
pub struct Dictionary {
    connection: Mutex<Connection>,
    resource: ResourceInfo,
}
impl Dictionary {
    pub fn open(resource_dir: &Path) -> Result<Self, String> {
        let provenance = std::fs::read(resource_dir.join("language/dictionary-provenance.json"))
            .map_err(|e| e.to_string())?;
        let provenance_sha = format!("{:x}", Sha256::digest(&provenance));
        let manifest_bytes = std::fs::read(resource_dir.join("language/manifest.json"))
            .map_err(|e| e.to_string())?;
        if manifest_bytes
            != include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../resources/language/manifest.json"
            ))
        {
            return Err("dictionary manifest differs from compiled resource lock".into());
        }
        let manifest: serde_json::Value =
            serde_json::from_slice(&manifest_bytes).map_err(|e| e.to_string())?;
        if manifest["dictionary_provenance_sha256"].as_str() != Some(provenance_sha.as_str()) {
            return Err("dictionary provenance hash mismatch".into());
        }
        let path = resource_dir.join("language/dictionary.sqlite3");
        let entry = manifest["files"]
            .as_array()
            .ok_or("missing language file inventory")?
            .iter()
            .find(|entry| entry["path"] == "language/dictionary.sqlite3")
            .ok_or("dictionary missing from resource manifest")?;
        let metadata = path.symlink_metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || Some(metadata.len()) != entry["size"].as_u64()
        {
            return Err("dictionary file size/type mismatch".into());
        }
        let mut file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut bytes = [0u8; 65_536];
        loop {
            let count = file.read(&mut bytes).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&bytes[..count]);
        }
        if entry["sha256"].as_str() != Some(format!("{:x}", hash.finalize()).as_str()) {
            return Err("dictionary file hash mismatch".into());
        }
        let connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| e.to_string())?;
        connection
            .pragma_update(None, "query_only", true)
            .map_err(|e| e.to_string())?;
        let (version,stored_hash,normalization):(String,String,String)=connection.query_row("SELECT resource_version,manifest_sha256,normalization_version FROM dictionary_meta WHERE singleton=1 AND index_schema_version=1",[],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(|e|e.to_string())?;
        if stored_hash != provenance_sha
            || manifest["dictionary_version"].as_str() != Some(version.as_str())
            || normalization != "kana-hiragana-exact-v1"
        {
            return Err("dictionary metadata does not match reviewed resource version".into());
        }
        Ok(Self {
            connection: Mutex::new(connection),
            resource: ResourceInfo {
                kind: "dictionary".into(),
                version,
                manifest_sha256: provenance_sha,
                available: true,
                detail: "随包中文基础辞书；JMdict英语释义单独标注补充，逐条保留来源与许可".into(),
            },
        })
    }
    pub fn resource_info(&self) -> ResourceInfo {
        self.resource.clone()
    }
    pub fn lookup(
        &self,
        lemma: &str,
        surface: &str,
        reading: &str,
    ) -> Result<Vec<DictionaryEntry>, String> {
        if [lemma, surface, reading]
            .iter()
            .any(|value| value.len() > 65_536 || value.contains('\0'))
        {
            return Err("invalid dictionary lookup text".into());
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| "dictionary reader poisoned")?;
        // A reading is a fallback, never an extra OR term on an exact spelling.
        // Prefer the lexical lemma before the inflected source surface. The
        // fixed v1 index also retains spelling aliases; those may be used only
        // when no canonical headword exists for either identity.
        let lemma = normalize(lemma);
        let surface = normalize(surface);
        let keys = [&lemma, &surface];
        for kind in ["lemma", "surface"] {
            for (index, key) in keys.iter().enumerate() {
                if key.is_empty() || (index == 1 && surface == lemma) {
                    continue;
                }
                let entries = query_entries(&connection, key, Some(kind))?;
                if !entries.is_empty() {
                    return Ok(entries);
                }
            }
        }
        let reading = normalize(reading);
        if reading.is_empty() {
            return Ok(Vec::new());
        }
        query_entries(&connection, &reading, None)
    }
}

/// Every result retains its real source headword and its own senses. Reading
/// fallback must not manufacture a merged entry labelled as the selected word.
fn query_entries(
    connection: &Connection,
    key: &str,
    form_kind: Option<&str>,
) -> Result<Vec<DictionaryEntry>, String> {
    let mut statement=connection.prepare("SELECT DISTINCT e.entry_id,e.lemma,e.reading_kana,e.pos_json,e.content_kind,s.display_name,s.snapshot_version,s.license_ref,s.attribution_text FROM dictionary_forms f JOIN dictionary_entries e ON e.entry_id=f.entry_id JOIN dictionary_sources s ON s.source_id=e.source_id WHERE f.normalized_key=?1 AND (?2 IS NULL OR f.form_kind=?2) ORDER BY CASE e.content_kind WHEN 'zh_base' THEN 0 ELSE 1 END,e.lemma,e.entry_id LIMIT 32").map_err(|e|e.to_string())?;
    let records = statement
        .query_map(params![key, form_kind], |row| {
            Ok(DictionaryEntry {
                entry_id: row.get(0)?,
                lemma: row.get(1)?,
                reading: row.get(2)?,
                part_of_speech: serde_json::from_str(&row.get::<_, String>(3)?).map_err(
                    |error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    },
                )?,
                content_kind: row.get(4)?,
                source_label: row.get(5)?,
                source_version: row.get(6)?,
                license_ref: row.get(7)?,
                attribution: row.get(8)?,
                senses: Vec::new(),
            })
        })
        .map_err(|e| e.to_string())?;
    let mut output = Vec::new();
    for record in records {
        let mut entry = record.map_err(|e| e.to_string())?;
        let mut senses=connection.prepare("SELECT gloss_language,gloss_text,usage_notes,source_locator FROM dictionary_senses WHERE entry_id=?1 ORDER BY sense_index").map_err(|e|e.to_string())?;
        entry.senses = senses
            .query_map([&entry.entry_id], |row| {
                Ok(DictionarySense {
                    language: row.get(0)?,
                    text: row.get(1)?,
                    usage_notes: row.get(2)?,
                    source_locator: row.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        output.push(entry);
    }
    Ok(output)
}
fn normalize(text: &str) -> String {
    text.trim()
        .chars()
        .map(|c| {
            if ('\u{30a1}'..='\u{30f6}').contains(&c) {
                char::from_u32(c as u32 - 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_offline_dictionary_separates_chinese_and_english_and_cannot_write() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
        let dictionary = Dictionary::open(&root).expect("real packaged dictionary");
        let entries = dictionary.lookup("図書館", "図書館", "トショカン").unwrap();
        assert!(entries.iter().any(|entry| {
            entry.content_kind == "zh_base"
                && entry
                    .senses
                    .iter()
                    .any(|s| s.language == "zh" && s.text.contains("圖書館"))
        }));
        assert!(
            entries
                .iter()
                .any(|entry| entry.content_kind == "en_supplement"
                    && entry.senses.iter().all(|s| s.language == "en"))
        );
        assert!(entries.iter().all(|entry| {
            !entry.license_ref.is_empty()
                && !entry.attribution.is_empty()
                && entry
                    .senses
                    .iter()
                    .all(|sense| sense.source_locator.starts_with("https://"))
        }));
        assert!(
            dictionary
                .lookup("ルナ架空語九八七六五", "", " ")
                .unwrap()
                .is_empty()
        );
        assert!(
            dictionary
                .connection
                .lock()
                .unwrap()
                .execute("DELETE FROM dictionary_entries", [])
                .is_err()
        );
    }
    #[test]
    fn exact_headwords_do_not_collect_homophone_senses() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
        let dictionary = Dictionary::open(&root).expect("real immutable offline index");
        for (headword, reading) in [("駅", "えき"), ("橋", "はし"), ("箸", "はし")] {
            let entries = dictionary.lookup(headword, headword, reading).unwrap();
            assert!(!entries.is_empty(), "real headword {headword}");
            assert!(
                entries.iter().all(|entry| entry.lemma == headword),
                "exact {headword} must not widen through reading {reading}: {:?}",
                entries.iter().map(|entry| &entry.lemma).collect::<Vec<_>>()
            );
            eprintln!(
                "DICTIONARY_EXACT {}",
                serde_json::json!({"word":headword,"reading":reading,"headwords":entries.iter().map(|entry|&entry.lemma).collect::<Vec<_>>(),"entries":entries.len()})
            );
        }
        // An incorrect reading still cannot override an exact orthographic identity.
        let entries = dictionary.lookup("橋", "橋", "えき").unwrap();
        assert!(!entries.is_empty() && entries.iter().all(|entry| entry.lemma == "橋"));
    }

    #[test]
    fn inflected_surfaces_keep_their_lexical_headword_and_aliases_still_work() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
        let dictionary = Dictionary::open(&root).expect("real immutable offline index");
        for (lemma, surface, reading) in [("歩く", "歩いた", "あるいた"), ("見る", "見た", "みた")]
        {
            let entries = dictionary.lookup(lemma, surface, reading).unwrap();
            assert!(!entries.is_empty() && entries.iter().all(|entry| entry.lemma == lemma));
        }
        let alias = dictionary
            .lookup("あうんの呼吸", "あうんの呼吸", "あうんのこきゅう")
            .unwrap();
        assert!(
            alias.iter().any(|entry| entry.lemma == "阿吽の呼吸"),
            "an indexed alternative spelling must keep its true canonical headword"
        );
    }

    #[test]
    fn phonetic_fallback_preserves_distinct_real_headwords_and_senses() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
        let dictionary = Dictionary::open(&root).expect("real immutable offline index");
        let missing = "索引にない表記九八七六五";
        assert!(dictionary.lookup(missing, missing, "").unwrap().is_empty());
        let entries = dictionary.lookup(missing, missing, "エキ").unwrap();
        let headwords = entries
            .iter()
            .map(|entry| entry.lemma.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(headwords.len() > 1 && headwords.contains("駅") && headwords.contains("役"));
        assert!(
            entries
                .iter()
                .all(|entry| entry.lemma != missing && !entry.senses.is_empty())
        );
        let connection = dictionary.connection.lock().unwrap();
        for entry in &entries {
            let real_headword: String = connection
                .query_row(
                    "SELECT lemma FROM dictionary_entries WHERE entry_id=?1",
                    [&entry.entry_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(entry.lemma, real_headword);
            let real_senses: Vec<String> = connection.prepare("SELECT gloss_text FROM dictionary_senses WHERE entry_id=?1 ORDER BY sense_index").unwrap().query_map([&entry.entry_id], |row|row.get(0)).unwrap().collect::<Result<_,_>>().unwrap();
            assert_eq!(
                entry
                    .senses
                    .iter()
                    .map(|sense| sense.text.clone())
                    .collect::<Vec<_>>(),
                real_senses
            );
        }
        eprintln!(
            "DICTIONARY_FALLBACK {}",
            serde_json::json!({"reading":"エキ","headwords":headwords,"entries":entries.len(),"resource_version":dictionary.resource_info().version})
        );
    }
}
