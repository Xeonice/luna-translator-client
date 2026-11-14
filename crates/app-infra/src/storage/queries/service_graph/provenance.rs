//! Reader and writer share exact copied-slice validation. Source traversal is an
//! iterative postorder DAG walk, so nested codec3 sources use one verification
//! context rather than recursively starting a new loader for every edge.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

struct Proof {
    kind: String,
    source_revision: String,
    model: Option<String>,
    source_model: Option<String>,
    offline: Option<String>,
    source_offline: Option<String>,
    connection: Option<String>,
    source_connection: Option<String>,
    control_connection: Option<String>,
    control_config: Option<String>,
    reason: String,
}
fn proofs(conn: &Connection, revision: &str) -> AppResult<Vec<Proof>> {
    let mut query = conn.prepare("SELECT target_kind,source_revision_id,effective_model_id,source_model_id,effective_offline_config_id,source_offline_config_id,effective_connection_id,source_connection_id,control_connection_id,control_config_id,reason FROM activation_resource_materializations WHERE effective_revision_id=?1 ORDER BY materialization_id").map_err(err)?;
    query
        .query_map([revision], |row| {
            Ok(Proof {
                kind: row.get(0)?,
                source_revision: row.get(1)?,
                model: row.get(2)?,
                source_model: row.get(3)?,
                offline: row.get(4)?,
                source_offline: row.get(5)?,
                connection: row.get(6)?,
                source_connection: row.get(7)?,
                control_connection: row.get(8)?,
                control_config: row.get(9)?,
                reason: row.get(10)?,
            })
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)
}
fn identity(conn: &Connection, revision: &str) -> AppResult<u32> {
    let saved: Option<(u32, String, String)> = conn.query_row(
        "SELECT r.codec_version,r.content_hash,s.content_hash FROM config_revisions r JOIN config_revision_seals s USING(revision_id) WHERE r.revision_id=?1 AND NOT EXISTS(SELECT 1 FROM config_gc_marks g WHERE g.revision_id=r.revision_id)",
        [revision], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(err)?;
    let Some((codec, metadata, seal)) = saved else {
        return Err(failure("configuration_not_found"));
    };
    if metadata != seal {
        return Err(failure("configuration_seal_mismatch"));
    }
    if !(1..=3).contains(&codec) {
        return Err(failure("unsupported_configuration_codec"));
    }
    Ok(codec)
}
struct Verified {
    document: ConfigurationDocument,
    graph: ConfigurationV3,
    offline_controls: BTreeMap<String, String>,
}
/// Codec1's original top-level Vec ordering was not persisted. Its real loader
/// validates the legacy representation, and the actual copied fields below must
/// match the sealed codec3 target. This does not claim a reconstructed codec1
/// whole-document hash. Codec2 and codec3 additionally verify their real seals.
pub(super) fn load_verified(conn: &Connection, revision: &str) -> AppResult<ConfigurationV3> {
    if identity(conn, revision)? != 3 {
        return Err(failure("unsupported_configuration_codec"));
    }
    let mut loaded: BTreeMap<String, Verified> = BTreeMap::new();
    let mut processing = BTreeSet::new();
    let mut stack = vec![(revision.to_owned(), false)];
    while let Some((current, expanded)) = stack.pop() {
        if loaded.contains_key(&current) {
            continue;
        }
        if !expanded {
            if !processing.insert(current.clone()) {
                return Err(failure("materialization_source_cycle"));
            }
            let codec = identity(conn, &current)?;
            let sources = proofs(conn, &current)?;
            if codec != 3 && !sources.is_empty() {
                return Err(failure("invalid_legacy_materialization"));
            }
            stack.push((current.clone(), true));
            let unique = sources
                .into_iter()
                .map(|proof| proof.source_revision)
                .collect::<BTreeSet<_>>();
            for source in unique {
                if processing.contains(&source) {
                    return Err(failure("materialization_source_cycle"));
                }
                if !loaded.contains_key(&source) {
                    stack.push((source, false));
                }
            }
            continue;
        }
        let document = match identity(conn, &current)? {
            1 | 2 => super::super::configuration::load_document(conn, &current)?,
            3 => ConfigurationDocument::ServiceGraph {
                configuration: super::load_raw(conn, &current)?,
            },
            _ => return Err(failure("unsupported_configuration_codec")),
        };
        let graph = app_core::domain::model_config::service_draft::materialize_base(&document)?;
        let mut offline_controls = BTreeMap::new();
        for proof in proofs(conn, &current)? {
            let source = loaded
                .get(&proof.source_revision)
                .ok_or_else(|| failure("materialization_source_unverified"))?;
            if !matches!(
                proof.reason.as_str(),
                "selected_closure" | "activation_collision" | "legacy_materialization"
            ) {
                return Err(failure("invalid_materialization_reason"));
            }
            match proof.kind.as_str() {
                "http" => {
                    if proof.offline.is_some()
                        || proof.source_offline.is_some()
                        || proof.control_config.is_some()
                    {
                        return Err(failure("invalid_connection_materialization"));
                    }
                    let copied = verify_http_copy(
                        &source.document,
                        &source.graph,
                        &graph,
                        HttpCopyProof {
                            source_revision: &proof.source_revision,
                            source_id: required(&proof.source_model)?,
                            effective_id: required(&proof.model)?,
                            reason: &proof.reason,
                            scope: CopyScope::PersistedLegacySlice,
                        },
                    )?;
                    if proof.source_connection.as_deref() != Some(&copied.source_connection)
                        || proof.connection.as_deref() != Some(&copied.effective_connection)
                    {
                        return Err(failure("invalid_connection_materialization"));
                    }
                    // This also verifies all agreeing HTTP proofs and the full
                    // iterative control chain, not a first-row SQL shortcut.
                    let control = super::connection_materialization(
                        conn,
                        &current,
                        &copied.effective_connection,
                    )?
                    .ok_or_else(|| failure("invalid_connection_materialization"))?;
                    if proof.control_connection.as_deref() != Some(&control.control_connection_id) {
                        return Err(failure("invalid_connection_control_source"));
                    }
                }
                "connection" => {
                    if proof.model.is_some()
                        || proof.source_model.is_some()
                        || proof.offline.is_some()
                        || proof.source_offline.is_some()
                        || proof.control_config.is_some()
                        || proof.reason != "legacy_materialization"
                    {
                        return Err(failure("invalid_connection_materialization"));
                    }
                    let source_id = required(&proof.source_connection)?;
                    let effective_id = required(&proof.connection)?;
                    verify_connection_copy(
                        &source.document,
                        &source.graph,
                        &graph,
                        source_id,
                        effective_id,
                    )?;
                    let control = super::connection_materialization(conn, &current, effective_id)?
                        .ok_or_else(|| failure("invalid_connection_materialization"))?;
                    if proof.control_connection.as_deref() != Some(&control.control_connection_id) {
                        return Err(failure("invalid_connection_control_source"));
                    }
                }
                "offline" => {
                    if proof.model.is_some()
                        || proof.source_model.is_some()
                        || proof.connection.is_some()
                        || proof.source_connection.is_some()
                        || proof.control_connection.is_some()
                        || proof.reason == "legacy_materialization"
                    {
                        return Err(failure("invalid_offline_materialization"));
                    }
                    let source_id = required(&proof.source_offline)?;
                    let effective_id = required(&proof.offline)?;
                    verify_offline_copy(&source.graph, &graph, source_id, effective_id)?;
                    let expected = source
                        .offline_controls
                        .get(source_id)
                        .map(String::as_str)
                        .unwrap_or(source_id);
                    if proof.control_config.as_deref() != Some(expected) {
                        return Err(failure("invalid_offline_control_source"));
                    }
                    if offline_controls
                        .insert(effective_id.to_owned(), expected.to_owned())
                        .is_some()
                    {
                        return Err(failure("conflicting_offline_materialization"));
                    }
                }
                _ => return Err(failure("invalid_materialization_kind")),
            }
        }
        processing.remove(&current);
        loaded.insert(
            current,
            Verified {
                document,
                graph,
                offline_controls,
            },
        );
    }
    loaded
        .remove(revision)
        .map(|verified| verified.graph)
        .ok_or_else(|| failure("configuration_not_found"))
}
fn required(value: &Option<String>) -> AppResult<&str> {
    value
        .as_deref()
        .ok_or_else(|| failure("invalid_materialization_source"))
}
/// Exact writer behavior is unchanged. The reader also accepts already sealed
/// legacy migrations whose new codec3-only purpose/defaults were never fields in
/// either legacy codec1 or codec2 Model. Those two values remain verified by the
/// target codec3 canonical seal, and are never claimed as source-derived fields.
pub(super) enum CopyScope {
    ExactGraph,
    PersistedLegacySlice,
}
pub(super) struct CopiedHttp {
    pub source_connection: String,
    pub effective_connection: String,
}
pub(super) struct HttpCopyProof<'a> {
    pub source_revision: &'a str,
    pub source_id: &'a str,
    pub effective_id: &'a str,
    pub reason: &'a str,
    pub scope: CopyScope,
}
pub(super) fn verify_http_copy(
    document: &ConfigurationDocument,
    source: &ConfigurationV3,
    graph: &ConfigurationV3,
    proof: HttpCopyProof<'_>,
) -> AppResult<CopiedHttp> {
    let HttpCopyProof { source_revision, source_id, effective_id, reason, scope } = proof;
    let original = source
        .models
        .iter()
        .find(|model| model.model_id == source_id)
        .ok_or_else(|| failure("invalid_materialization_source"))?;
    let effective = graph
        .models
        .iter()
        .find(|model| model.model_id == effective_id)
        .ok_or_else(|| failure("invalid_materialization_target"))?;
    let original_connection = source
        .connections
        .iter()
        .find(|connection| connection.connection_id == original.connection_id)
        .ok_or_else(|| failure("invalid_materialization_source"))?;
    let actual_connection = graph
        .connections
        .iter()
        .find(|connection| connection.connection_id == effective.connection_id)
        .ok_or_else(|| failure("invalid_materialization_target"))?;
    let mut expected = original.clone();
    expected.model_id = effective.model_id.clone();
    expected.connection_id = effective.connection_id.clone();
    if matches!(scope, CopyScope::PersistedLegacySlice) && document.as_legacy().is_some() {
        expected.template_purpose = effective.template_purpose;
        expected.request_defaults = effective.request_defaults.clone();
    }
    if !original
        .default_templates
        .keys()
        .eq(effective.default_templates.keys())
    {
        return Err(failure("materialization_field_mismatch"));
    }
    expected.default_templates = effective.default_templates.clone();
    for (purpose, id) in &effective.default_templates {
        let old = original
            .default_templates
            .get(purpose)
            .ok_or_else(|| failure("materialization_field_mismatch"))?;
        let mut template = source
            .templates
            .iter()
            .find(|template| &template.template_id == old)
            .ok_or_else(|| failure("invalid_materialization_source"))?
            .clone();
        let actual = graph
            .templates
            .iter()
            .find(|template| &template.template_id == id)
            .ok_or_else(|| failure("invalid_materialization_target"))?;
        template.template_id = id.clone();
        if &template != actual {
            return Err(failure("materialization_field_mismatch"));
        }
    }
    let mut expected_connection = original_connection.clone();
    expected_connection.connection_id = actual_connection.connection_id.clone();
    if expected != *effective || expected_connection != *actual_connection {
        return Err(failure("materialization_field_mismatch"));
    }
    if reason == "legacy_materialization"
        && (document.as_legacy().is_none()
            || effective
                .legacy_eligibility_origin
                .as_ref()
                .is_none_or(|origin| {
                    origin.revision_id != source_revision || origin.model_id != source_id
                }))
    {
        return Err(failure("invalid_legacy_materialization"));
    }
    Ok(CopiedHttp {
        source_connection: original.connection_id.clone(),
        effective_connection: effective.connection_id.clone(),
    })
}
pub(super) fn verify_offline_copy(
    source: &ConfigurationV3,
    graph: &ConfigurationV3,
    source_id: &str,
    effective_id: &str,
) -> AppResult<()> {
    let original = source
        .offline_engine_configs
        .iter()
        .find(|config| config.id == source_id)
        .ok_or_else(|| failure("invalid_materialization_source"))?;
    let actual = graph
        .offline_engine_configs
        .iter()
        .find(|config| config.id == effective_id)
        .ok_or_else(|| failure("invalid_materialization_target"))?;
    let mut expected = original.clone();
    expected.id = actual.id.clone();
    if expected != *actual {
        return Err(failure("materialization_field_mismatch"));
    }
    Ok(())
}
pub(super) fn verify_connection_copy(
    document: &ConfigurationDocument,
    source: &ConfigurationV3,
    graph: &ConfigurationV3,
    source_id: &str,
    effective_id: &str,
) -> AppResult<()> {
    if document.as_legacy().is_none() {
        return Err(failure("connection_source_not_legacy"));
    }
    if source.revision_id == graph.revision_id
        || source
            .models
            .iter()
            .any(|model| model.connection_id == source_id)
        || graph
            .models
            .iter()
            .any(|model| model.connection_id == effective_id)
    {
        return Err(failure("connection_source_is_not_orphan"));
    }
    let mut expected = source
        .connections
        .iter()
        .find(|connection| connection.connection_id == source_id)
        .ok_or_else(|| failure("invalid_connection_materialization"))?
        .clone();
    let actual = graph
        .connections
        .iter()
        .find(|connection| connection.connection_id == effective_id)
        .ok_or_else(|| failure("invalid_materialization_target"))?;
    expected.connection_id = actual.connection_id.clone();
    if &expected != actual {
        return Err(failure("materialization_field_mismatch"));
    }
    Ok(())
}
