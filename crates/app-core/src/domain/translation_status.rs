//! Current translation state follows explicit admission edges, never wall-clock
//! or SQLite row order. Legacy tasks without edges remain unordered heads.
use crate::contracts::{AppError, AppResult, TranslationHeadSummary};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationStatusLineage {
    schema_version: u32,
    supersedes_task_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LineageWire {
    schema_version: u32,
    supersedes_task_ids: Vec<String>,
}

fn invalid_lineage() -> AppError {
    AppError::new(
        "invalid_translation_status_lineage",
        "storage",
        "翻译请求归属记录无效，已保留历史并拒绝推测当前状态",
    )
}

impl TranslationStatusLineage {
    pub fn new(supersedes_task_ids: Vec<String>) -> AppResult<Self> {
        let mut seen = BTreeSet::new();
        for task in &supersedes_task_ids {
            super::require_id(task).map_err(|_| invalid_lineage())?;
            if !seen.insert(task) {
                return Err(invalid_lineage());
            }
        }
        Ok(Self {
            schema_version: 1,
            supersedes_task_ids,
        })
    }
}

impl<'de> Deserialize<'de> for TranslationStatusLineage {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = LineageWire::deserialize(deserializer)?;
        if wire.schema_version != 1 {
            return Err(serde::de::Error::custom(
                "unknown translation lineage version",
            ));
        }
        Self::new(wire.supersedes_task_ids).map_err(serde::de::Error::custom)
    }
}

// Deserialize the original bytes rather than a Value: duplicate metadata keys
// must not disappear before validation. Other immutable request snapshot fields
// keep their existing owners and schemas.
#[derive(Deserialize)]
struct LineageSnapshot {
    #[serde(
        rename = "translationStatusLineage",
        default,
        deserialize_with = "present_lineage"
    )]
    lineage: Option<TranslationStatusLineage>,
}

fn present_lineage<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<TranslationStatusLineage>, D::Error> {
    TranslationStatusLineage::deserialize(deserializer).map(Some)
}

pub fn lineage_from_snapshot(snapshot: &str) -> AppResult<Option<TranslationStatusLineage>> {
    serde_json::from_str::<LineageSnapshot>(snapshot)
        .map(|snapshot| snapshot.lineage)
        .map_err(|_| invalid_lineage())
}

pub struct TranslationStatusNode {
    pub task_id: String,
    pub lineage: Option<TranslationStatusLineage>,
}

/// The caller supplies every task in exactly one stable engine lane. A missing
/// reference therefore rejects missing and cross-lane edges alike.
pub fn translation_status_heads(nodes: &[TranslationStatusNode]) -> AppResult<BTreeSet<String>> {
    let mut predecessors = BTreeMap::new();
    for node in nodes {
        super::require_id(&node.task_id).map_err(|_| invalid_lineage())?;
        let ids = node
            .lineage
            .as_ref()
            .map(|lineage| lineage.supersedes_task_ids.clone())
            .unwrap_or_default();
        if predecessors.insert(node.task_id.clone(), ids).is_some() {
            return Err(invalid_lineage());
        }
    }
    let mut referenced = BTreeSet::new();
    let mut successors: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut remaining = BTreeMap::new();
    for (task, ids) in &predecessors {
        for predecessor in ids {
            if predecessor == task || !predecessors.contains_key(predecessor) {
                return Err(invalid_lineage());
            }
            referenced.insert(predecessor.clone());
            successors.entry(predecessor).or_default().push(task);
        }
        remaining.insert(task.as_str(), ids.len());
    }
    // Iterative topological validation also handles long real histories without
    // making history length a call-stack or product retention limit.
    let mut ready: VecDeque<_> = remaining
        .iter()
        .filter_map(|(task, count)| (*count == 0).then_some(*task))
        .collect();
    let mut visited = 0;
    while let Some(task) = ready.pop_front() {
        visited += 1;
        for successor in successors.get(task).into_iter().flatten() {
            let count = remaining.get_mut(successor).ok_or_else(invalid_lineage)?;
            *count -= 1;
            if *count == 0 {
                ready.push_back(successor);
            }
        }
    }
    if visited != nodes.len() {
        return Err(invalid_lineage());
    }
    Ok(predecessors
        .into_keys()
        .filter(|task| !referenced.contains(task))
        .collect())
}

/// An unordered head set has bounded response size regardless of history size.
/// Counts describe real states; they never elect a latest task or fake success.
pub fn summarize_translation_heads<'a>(
    states: impl IntoIterator<Item = &'a str>,
) -> AppResult<TranslationHeadSummary> {
    let mut counts = [0usize; 6];
    for state in states {
        let index = match state {
            "queued" => 0,
            "running" => 1,
            "completed" => 2,
            "failed" => 3,
            "cancelled" => 4,
            "invalidated" => 5,
            _ => return Err(invalid_lineage()),
        };
        counts[index] = counts[index].checked_add(1).ok_or_else(invalid_lineage)?;
    }
    let total = counts
        .iter()
        .try_fold(0usize, |total, count| total.checked_add(*count))
        .ok_or_else(invalid_lineage)?;
    Ok(TranslationHeadSummary {
        head_count: total.to_string(),
        queued_count: counts[0].to_string(),
        running_count: counts[1].to_string(),
        completed_count: counts[2].to_string(),
        failed_count: counts[3].to_string(),
        cancelled_count: counts[4].to_string(),
        invalidated_count: counts[5].to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, replaces: Option<Vec<&str>>) -> TranslationStatusNode {
        TranslationStatusNode {
            task_id: id.into(),
            lineage: replaces.map(|ids| {
                TranslationStatusLineage::new(ids.into_iter().map(String::from).collect()).unwrap()
            }),
        }
    }

    #[test]
    fn legacy_and_old_writer_heads_stay_unordered_until_explicit_admission() {
        let mut nodes = vec![node("old-failed", None), node("old-completed", None)];
        assert_eq!(translation_status_heads(&nodes).unwrap().len(), 2);
        nodes.push(node("new", Some(vec!["old-failed", "old-completed"])));
        assert_eq!(
            translation_status_heads(&nodes).unwrap(),
            BTreeSet::from(["new".into()])
        );
        nodes.push(node("later-old-binary", None));
        assert_eq!(translation_status_heads(&nodes).unwrap().len(), 2);
        nodes.push(node("explicit", Some(vec!["new", "later-old-binary"])));
        assert_eq!(
            translation_status_heads(&nodes).unwrap(),
            BTreeSet::from(["explicit".into()])
        );
    }

    #[test]
    fn malformed_present_metadata_is_never_treated_as_legacy() {
        assert!(
            lineage_from_snapshot(r#"{"operationId":"legacy"}"#)
                .unwrap()
                .is_none()
        );
        for snapshot in [
            r#"{"translationStatusLineage":null}"#,
            r#"{"translationStatusLineage":{}}"#,
            r#"{"translationStatusLineage":{"schemaVersion":2,"supersedesTaskIds":[]}}"#,
            r#"{"translationStatusLineage":{"schemaVersion":1,"supersedesTaskIds":[""]}}"#,
            r#"{"translationStatusLineage":{"schemaVersion":1,"supersedesTaskIds":["x","x"]}}"#,
            r#"{"translationStatusLineage":{"schemaVersion":1,"supersedesTaskIds":[],"extra":true}}"#,
            r#"{"translationStatusLineage":{"schemaVersion":1,"schemaVersion":1,"supersedesTaskIds":[]}}"#,
            r#"{"translationStatusLineage":{"schemaVersion":1,"supersedesTaskIds":[]},"translationStatusLineage":{"schemaVersion":1,"supersedesTaskIds":[]}}"#,
        ] {
            assert!(lineage_from_snapshot(snapshot).is_err(), "{snapshot}");
        }
    }

    #[test]
    fn missing_cross_lane_self_duplicate_nodes_and_cycles_are_rejected() {
        for nodes in [
            vec![node("a", Some(vec!["other-lane"]))],
            vec![node("a", Some(vec!["a"]))],
            vec![node("a", None), node("a", None)],
            vec![node("a", Some(vec!["b"])), node("b", Some(vec!["a"]))],
        ] {
            assert!(translation_status_heads(&nodes).is_err());
        }
    }

    #[test]
    fn a_long_history_and_multiple_causal_heads_need_no_clock_or_recursion() {
        let mut nodes = vec![node("root", None)];
        nodes.push(node("branch-a", Some(vec!["root"])));
        nodes.push(node("branch-b", Some(vec!["root"])));
        assert_eq!(translation_status_heads(&nodes).unwrap().len(), 2);
        for n in 0..4096 {
            let prior = nodes.last().unwrap().task_id.clone();
            nodes.push(TranslationStatusNode {
                task_id: format!("long-{n}"),
                lineage: Some(TranslationStatusLineage::new(vec![prior]).unwrap()),
            });
        }
        assert_eq!(translation_status_heads(&nodes).unwrap().len(), 2);
    }
}
