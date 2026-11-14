//! Object-scoped read composition; immutable task facts and bounded snapshot
//! history are separate. Only the caller's explicit read transaction is accepted.
use super::*;
use app_core::domain::translation_status::{
    TranslationStatusLineage, TranslationStatusNode, lineage_from_snapshot,
    summarize_translation_heads, translation_status_heads,
};
use std::collections::BTreeSet;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct TranslationLane {
    pub unit_id: String,
    pub text_revision: u32,
    pub scope_key: String,
    pub binding_id: String,
    pub engine_id: String,
}

struct LaneFacts {
    nodes: Vec<TranslationStatusNode>,
    views: BTreeMap<String, TaskView>,
}
fn lane_facts(conn: &Connection, lane: &TranslationLane) -> AppResult<LaneFacts> {
    // Complete one stable lane, never the 100-terminal display window. The
    // existing unit/revision/purpose index excludes unrelated reading history.
    let sql = format!(
        "SELECT {TASK_VIEW_COLUMNS},q.input_snapshot_json {TASK_VIEW_JOINS} WHERE q.unit_id=?1 AND q.text_revision=?2 AND q.purpose='translation' AND q.scope_key=?3 AND q.binding_id=?4 AND q.engine_id=?5"
    );
    let mut statement = conn.prepare(&sql).map_err(err)?;
    let rows = statement
        .query_map(
            params![
                lane.unit_id,
                lane.text_revision,
                lane.scope_key,
                lane.binding_id,
                lane.engine_id
            ],
            |row| Ok((task_view_row(row)?, row.get::<_, String>(8)?)),
        )
        .map_err(err)?;
    let mut nodes = Vec::new();
    let mut views = BTreeMap::new();
    for row in rows {
        let (view, snapshot) = row.map_err(err)?;
        nodes.push(TranslationStatusNode {
            task_id: view.task_id.clone(),
            lineage: lineage_from_snapshot(&snapshot)?,
        });
        views.insert(view.task_id.clone(), view);
    }
    Ok(LaneFacts { nodes, views })
}

pub(super) fn heads(conn: &Connection, lane: &TranslationLane) -> AppResult<BTreeSet<String>> {
    translation_status_heads(&lane_facts(conn, lane)?.nodes)
}
/// Admission's Immediate transaction calls this before its new task is inserted.
pub(super) fn for_admission(
    conn: &Connection,
    lane: &TranslationLane,
) -> AppResult<TranslationStatusLineage> {
    TranslationStatusLineage::new(heads(conn, lane)?.into_iter().collect())
}
fn engine_status(conn: &Connection, lane: &TranslationLane) -> AppResult<TranslationEngineStatus> {
    let facts = lane_facts(conn, lane)?;
    let heads = translation_status_heads(&facts.nodes)?;
    if heads.is_empty() {
        return Ok(TranslationEngineStatus::Unrequested);
    }
    if heads.len() == 1 {
        return Ok(TranslationEngineStatus::Current {
            task: facts.views[heads.first().expect("nonempty head")].clone(),
        });
    }
    let summary =
        summarize_translation_heads(heads.iter().map(|id| facts.views[id].status.as_str()))?;
    let active_task = if summary.queued_count == "1" && summary.running_count == "0"
        || summary.running_count == "1" && summary.queued_count == "0"
    {
        heads.iter().find_map(|id| {
            let view = &facts.views[id];
            matches!(view.status.as_str(), "queued" | "running").then(|| view.clone())
        })
    } else {
        None
    };
    Ok(TranslationEngineStatus::Unordered {
        summary,
        active_task,
    })
}

pub(in crate::storage) fn reading_unit_projection(
    tx: &Transaction<'_>,
    uid: &str,
    revision: u32,
) -> AppResult<ReadingUnitProjection> {
    if revision == 0 {
        return Err(AppError::new(
            "invalid_reading_revision",
            "reading",
            "翻译状态必须指定确切的正原文版本",
        ));
    }
    // This first SELECT pins the SQLite read snapshot. Every subsequent unit,
    // config, graph, task and identity observation comes from this transaction.
    let (profile_id,server_instance_id,profile_generation,commit_revision):(String,String,u32,u32) = tx.query_row("SELECT profile_id,server_instance_id,runtime_generation,commit_revision FROM profile_meta WHERE singleton=1",[],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).map_err(err)?;
    let unit = super::unit(tx, uid, revision)?;
    let applied = configuration::state(tx)?.applied;
    let mut engines = Vec::new();
    let mut lanes = Vec::new();
    match applied.as_ref() {
        Some(ConfigurationDocument::Legacy { configuration }) => {
            if let Some(binding) = effective_binding(configuration, unit.game_id.as_deref(), "translation")
                && binding.enabled
            {
                for engine in binding.engines.iter().filter(|engine| engine.enabled) {
                    let model = configuration.models.iter().find(|model| model.model_id == engine.model_id)
                        .ok_or_else(|| AppError::new("missing_model", "models", "模型不存在"))?;
                    let connection = configuration.connections.iter().find(|connection| connection.connection_id == model.connection_id)
                        .ok_or_else(|| AppError::new("missing_connection", "models", "连接不存在"))?;
                    lanes.push((binding.scope_key.clone(), binding.binding_id.clone(), engine.engine_id.clone(), connection.display_name.clone()));
                }
            }
        }
        Some(ConfigurationDocument::ServiceGraph { configuration }) => {
            use app_core::domain::model_config::service_graph::{EngineTarget, effective_service_binding};
            if let Some(binding) = effective_service_binding(configuration, unit.game_id.as_deref(), CatalogPurpose::Translation)
                && binding.enabled
            {
                for engine in binding.engines.iter().filter(|engine| engine.enabled) {
                    let label = match &engine.target {
                        EngineTarget::Http { model_id } => {
                            let model = configuration.models.iter().find(|model| &model.model_id == model_id)
                                .ok_or_else(|| AppError::new("missing_model", "models", "模型不存在"))?;
                            configuration.connections.iter().find(|connection| connection.connection_id == model.connection_id)
                                .ok_or_else(|| AppError::new("missing_connection", "models", "连接不存在"))?.display_name.clone()
                        }
                        EngineTarget::Offline { offline_config_id } => configuration.offline_engine_configs.iter()
                            .find(|config| &config.id == offline_config_id)
                            .ok_or_else(|| AppError::new("missing_offline_config", "models", "本机翻译配置不存在"))?.display_name.clone(),
                    };
                    lanes.push((binding.scope_key.clone(), binding.binding_id.clone(), engine.engine_id.clone(), label));
                }
            }
        }
        None => {}
    }
    for (scope_key, binding_id, engine_id, provider_label) in lanes {
        let lane = TranslationLane { unit_id: uid.into(), text_revision: revision, scope_key, binding_id, engine_id };
        let status = match engine_status(tx, &lane) {
            Ok(status) => status,
            Err(error) if error.code == "invalid_translation_status_lineage" => TranslationEngineStatus::Unavailable { error: error.into() },
            Err(error) => return Err(error),
        };
        engines.push(TranslationEngineProjection {
            scope_key: lane.scope_key, binding_id: lane.binding_id, engine_id: lane.engine_id, provider_label, status,
        });
    }
    let translation_state = ReadingTranslationState {
        unit_id: unit.unit_id.clone(),
        text_revision: unit.text_revision,
        source_hash: unit.source_hash.clone(),
        engines,
    };
    Ok(ReadingUnitProjection {
        profile_id,
        server_instance_id,
        profile_generation,
        commit_revision,
        unit,
        translation_state,
    })
}
