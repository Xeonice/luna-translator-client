//! Per-execution assertion evidence shared by the actual storage test targets.
//! No inventory entry creates a proof: callers record only after their own
//! production assertions return successfully. The final target rejects gaps.
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

pub fn directory(component: &str, fallback: &Path) -> PathBuf {
    let Some(output) = std::env::var_os("FP_STORAGE_OUTPUT") else {
        fs::create_dir_all(fallback).unwrap();
        return fallback.to_owned();
    };
    assert!(
        component
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
    );
    let path = PathBuf::from(output)
        .join("components")
        .join(component)
        .join(format!("{}-{}", std::process::id(), now_ms()));
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn record(case: &str, checks: &[(&str, u64)], detail: Value) {
    assert!(!checks.is_empty());
    assert!(checks.iter().all(|(_, count)| *count > 0));
    let Some(output) = std::env::var_os("FP_STORAGE_OUTPUT") else {
        return;
    };
    assert!(
        case.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    );
    let path = PathBuf::from(output).join("asserted-cases");
    fs::create_dir_all(&path).unwrap();
    let value = json!({"schema_version":1,"kind":"storage_asserted_case",
        "run_id":std::env::var("FP_STORAGE_RUN_ID").unwrap(),
        "source_tree_sha256":std::env::var("FP_STORAGE_SOURCE_SHA256").unwrap(),
        "case":case,"producer_pid":std::process::id(),"completed_at_ms":now_ms(),
        "checks":checks.iter().map(|(id,count)|json!({"id":id,"status":"passed","assertions":count,"skipped":0})).collect::<Vec<_>>(),
        "detail":detail});
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path.join(format!("{case}.json")))
        .unwrap();
    file.write_all(&serde_json::to_vec_pretty(&value).unwrap())
        .unwrap();
    file.sync_all().unwrap();
}
