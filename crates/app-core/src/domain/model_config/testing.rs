//! Completed model tests have their own provenance, never ordinary reading history.
use crate::{contracts::*, domain::canonical_hash};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCredentialUse {
    pub credential_id: String,
    pub pool_id: Option<String>,
    pub pool_revision: Option<u32>,
    pub member_id: Option<String>,
    pub masked_identity: String,
    pub outcome: String,
}
#[derive(Debug, Clone)]
pub struct ModelTestRecordInput {
    pub test_id: String,
    pub config_revision_id: String,
    pub connection_id: Option<String>,
    pub model_id: Option<String>,
    pub binding_id: Option<String>,
    pub purpose: Option<String>,
    pub stage: String,
    pub request_semantics_hash: String,
    pub input_scope: String,
    pub unit_id: Option<String>,
    pub text_revision: Option<u32>,
    pub input_summary: serde_json::Value,
    pub response_summary: serde_json::Value,
    pub output_validation: serde_json::Value,
    pub state: String,
    pub started_at_ms: i64,
    pub completed_at_ms: i64,
    pub error_code: Option<String>,
    pub error_stage: Option<String>,
    pub credentials: Vec<TestCredentialUse>,
}
/// Scope identity excludes labels and unrelated entities. The actual request plan has a separate hash.
pub fn scope_semantics_hash(c: &Configuration, kind: &str, id: &str) -> AppResult<String> {
    let connection = |id: &str| -> AppResult<&Connection> {
        c.connections
            .iter()
            .find(|v| v.connection_id == id)
            .ok_or_else(|| AppError::new("connection_missing", "test", "连接不存在"))
    };
    let con_identity = |con: &Connection| {
        let mut value = serde_json::json!({"protocol":con.protocol,"deployment":con.deployment,"authMode":con.auth_mode,"endpoint":con.endpoint,"poolId":con.credential_pool_id,"poolRevision":con.credential_pool_revision,"proxyMode":con.proxy_mode,"networkMetadata":con.network_metadata});
        if let Some(binding) = &con.adapter_binding {
            value["adapterBinding"] = serde_json::json!(binding);
        }
        value
    };
    let model_identity = |m: &Model| -> AppResult<serde_json::Value> {
        Ok(
            serde_json::json!({"connection":con_identity(connection(&m.connection_id)?),"model":m.actual_model_id,"parameters":m.parameters,"capabilities":m.capabilities}),
        )
    };
    match kind {
        "local" => {
            let mut c = c.clone();
            c.revision_id.clear();
            for v in &mut c.connections {
                v.display_name.clear();
            }
            for v in &mut c.models {
                v.display_name.clear();
            }
            for v in &mut c.templates {
                v.display_name.clear();
            }
            for v in &mut c.term_sets {
                v.display_name.clear();
            }
            canonical_hash(&c)
        }
        "connection" => canonical_hash(&con_identity(connection(id)?)),
        "model" => {
            let m = c
                .models
                .iter()
                .find(|v| v.model_id == id)
                .ok_or_else(|| AppError::new("model_missing", "test", "模型不存在"))?;
            canonical_hash(&model_identity(m)?)
        }
        "purpose" => {
            let b = c
                .bindings
                .iter()
                .find(|v| v.binding_id == id)
                .ok_or_else(|| AppError::new("purpose_missing", "test", "用途不存在"))?;
            let models = b
                .engines
                .iter()
                .filter(|e| e.enabled)
                .map(|e| {
                    let m = c
                        .models
                        .iter()
                        .find(|m| m.model_id == e.model_id)
                        .ok_or_else(|| AppError::new("model_missing", "test", "用途模型不存在"))?;
                    model_identity(m)
                })
                .collect::<AppResult<Vec<_>>>()?;
            let mut selected_templates = std::collections::BTreeSet::new();
            for engine in b.engines.iter().filter(|e| e.enabled) {
                let model = c
                    .models
                    .iter()
                    .find(|m| m.model_id == engine.model_id)
                    .ok_or_else(|| AppError::new("model_missing", "test", "用途模型不存在"))?;
                if let Some(template) = super::templates::resolve_template(c, model, b, engine)? {
                    selected_templates.insert(template.template_id.clone());
                }
            }
            let templates: Vec<_> = c
                .templates
                .iter()
                .filter(|t| selected_templates.contains(&t.template_id))
                .map(|t| {
                    (
                        &t.template_id,
                        &t.purpose,
                        &t.body,
                        &t.variables,
                        &t.compatibility,
                    )
                })
                .collect();
            let terms: Vec<_> = c
                .term_sets
                .iter()
                .filter(|t| t.enabled && (t.scope_key == "global" || t.scope_key == b.scope_key))
                .map(|t| (&t.scope_key, &t.terms))
                .collect();
            canonical_hash(&(b, models, templates, terms))
        }
        _ => Err(AppError::new("invalid_test_stage", "test", "未知测试范围")),
    }
}
