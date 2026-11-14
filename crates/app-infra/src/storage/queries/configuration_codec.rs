//! Explicit persisted codec identity and migration preservation of pre-existing columns.
use super::*;
pub(super) fn content_hash(c: &Configuration) -> AppResult<String> {
    Ok(app_core::domain::content_hash(
        app_core::domain::model_config::codec::encode_configuration(c)?,
    ))
}
pub(super) fn verify_loaded(conn: &Connection, c: &Configuration) -> AppResult<()> {
    if c.codec_version == Some(2) {
        let expected: String = conn
            .query_row(
                "SELECT content_hash FROM config_revision_seals WHERE revision_id=?1",
                [&c.revision_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if expected != content_hash(c)? {
            return Err(AppError::new(
                "configuration_seal_mismatch",
                "storage",
                "新配置字节与不可变封存身份不符",
            ));
        }
    }
    Ok(())
}
#[derive(Debug)]
pub(in crate::storage) struct LegacyRows(Vec<(String, Vec<String>, Vec<Vec<String>>)>);
fn identifier(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn rows(conn: &Connection, table: &str, cols: &[String]) -> AppResult<Vec<Vec<String>>> {
    let sql = format!(
        "SELECT {} FROM {}",
        cols.iter()
            .map(|c| format!("quote({})", identifier(c)))
            .collect::<Vec<_>>()
            .join(","),
        identifier(table)
    );
    let mut q = conn.prepare(&sql).map_err(err)?;
    let mut result = q
        .query_map([], |r| {
            (0..cols.len())
                .map(|i| r.get(i))
                .collect::<rusqlite::Result<Vec<String>>>()
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    result.sort();
    Ok(result)
}
pub(in crate::storage) fn legacy_rows(conn: &Connection) -> AppResult<LegacyRows> {
    let mut q=conn.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name!='schema_migrations' ORDER BY name").map_err(err)?;
    let tables = q
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    let mut result = vec![];
    for table in tables {
        let mut q = conn
            .prepare(&format!("PRAGMA table_info({})", identifier(&table)))
            .map_err(err)?;
        let cols = q
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        result.push((table.clone(), cols.clone(), rows(conn, &table, &cols)?));
    }
    Ok(LegacyRows(result))
}
pub(in crate::storage) fn verify_legacy_rows(conn: &Connection, old: &LegacyRows) -> AppResult<()> {
    for (table, cols, before) in &old.0 {
        if &rows(conn, table, cols)? != before {
            return Err(AppError::new(
                "migration_data_mismatch",
                "storage",
                format!("迁移改变了原表 {table} 的旧字段事实，事务已回滚"),
            ));
        }
    }
    Ok(())
}
