//! Explicit configuration tests are application use cases, separate from reading tasks.
use crate::{
    application::request_plan::{self, RequestPlan},
    contracts::*,
    domain::{TaskDispatch, canonical_hash, content_hash, testing::*},
    ports::{IdentitySource, ProductRepository, runtime::ModelGateway},
};
use serde_json::json;
type TestTarget = (
    Option<Connection>,
    Option<Model>,
    Option<PurposeBinding>,
    Option<Engine>,
);

pub async fn run(
    repository: &dyn ProductRepository,
    clock: &dyn IdentitySource,
    gateway: &dyn ModelGateway,
    configuration: Configuration,
    request: ConfigurationTestRequest,
) -> AppResult<ConfigurationTestResult> {
    let scope_hash =
        scope_semantics_hash(&configuration, &request.object_kind, &request.object_id)?;
    let mut targets: Vec<TestTarget> = Vec::new();
    match request.object_kind.as_str() {
        "local" => targets.push((None, None, None, None)),
        "connection" => targets.push((
            Some(
                configuration
                    .connections
                    .iter()
                    .find(|v| v.connection_id == request.object_id)
                    .cloned()
                    .ok_or_else(|| AppError::new("connection_missing", "test", "连接不存在"))?,
            ),
            None,
            None,
            None,
        )),
        "model" => {
            let model = configuration
                .models
                .iter()
                .find(|v| v.model_id == request.object_id)
                .cloned()
                .ok_or_else(|| AppError::new("model_missing", "test", "模型不存在"))?;
            let con = configuration
                .connections
                .iter()
                .find(|v| v.connection_id == model.connection_id)
                .cloned()
                .ok_or_else(|| AppError::new("connection_missing", "test", "连接不存在"))?;
            targets.push((Some(con), Some(model), None, None));
        }
        "purpose" => {
            let binding = configuration
                .bindings
                .iter()
                .find(|v| v.binding_id == request.object_id)
                .cloned()
                .ok_or_else(|| AppError::new("purpose_missing", "test", "用途不存在"))?;
            for engine in binding.engines.iter().filter(|e| e.enabled) {
                let model = configuration
                    .models
                    .iter()
                    .find(|v| v.model_id == engine.model_id)
                    .cloned()
                    .ok_or_else(|| AppError::new("model_missing", "test", "用途模型不存在"))?;
                let con = configuration
                    .connections
                    .iter()
                    .find(|v| v.connection_id == model.connection_id)
                    .cloned()
                    .ok_or_else(|| AppError::new("connection_missing", "test", "用途连接不存在"))?;
                targets.push((
                    Some(con),
                    Some(model),
                    Some(binding.clone()),
                    Some(engine.clone()),
                ));
            }
            if targets.is_empty() {
                return Err(AppError::new(
                    "engine_missing",
                    "test",
                    "用途尚未选择启用的引擎",
                ));
            }
        }
        _ => return Err(AppError::new("invalid_test_stage", "test", "未知测试层级")),
    }
    let mut model_ids = Vec::new();
    for (connection, model, binding, engine) in targets {
        let test_id = clock.new_id("model-test");
        let started = clock.now_ms();
        let mut credentials = Vec::new();
        let mut plan_identity = json!({"scope":scope_hash,"stage":request.object_kind});
        let result:AppResult<Vec<String>>=async {
            let Some(connection)=&connection else{crate::domain::validate_configuration(&configuration,false)?;return Ok(vec![]);};
            let selected=if let Some(kind)=crate::domain::model_config::catalog::credential_kind_for_auth_mode(&connection.auth_mode)? {Some(repository.select_credential_for_kind(connection.credential_pool_id.clone().ok_or_else(||AppError::new("missing_credential","test","连接未选择凭据池"))?,connection.credential_pool_revision.ok_or_else(||AppError::new("missing_credential","test","凭据池版本缺失"))?,kind)?)}else{None};
            let locator=selected.as_ref().map(|s|s.vault_ref.clone());
            if let Some(selected)=selected{credentials.push(TestCredentialUse{credential_id:selected.credential_id,pool_id:Some(selected.pool_id),pool_revision:Some(selected.pool_revision),member_id:Some(selected.member_id),masked_identity:selected.masked_identity,outcome:"not_attempted".into()});}
            if request.object_kind=="connection"&&!crate::domain::model_config::is_translation_protocol(&connection.protocol){
                plan_identity=json!({"scope":scope_hash,"protocol":connection.protocol,"endpoint":connection.endpoint,"route":"models","credential":credentials});
                return gateway.discover_models(connection,locator.as_deref()).await;
            }
            let actual_model=model.as_ref().and_then(|m|m.actual_model_id.clone());
            let plan=if let (Some(binding),Some(engine),Some(model))=(&binding,&engine,&model){
                let task=TaskDispatch{task_id:test_id.clone(),attempt_id:test_id.clone(),unit:ReadingUnit{source_summary:None,unit_id:"explicit-test-sample".into(),session_id:"explicit-test-sample".into(),game_id:None,text_revision:1,source_text:"こんにちは。".into(),source_hash:content_hash("こんにちは。"),source_language:"ja".into(),created_at_ms:started as f64,translations:vec![],tokens:vec![]},configuration:configuration.clone(),binding:binding.clone(),engine:engine.clone(),model:model.clone(),connection:connection.clone(),request_semantics_hash:String::new(),authorization_epoch:0,profile_generation:0,server_instance_id:"explicit-model-test".into(),frozen_history:vec![],selected_token:None,cached_result:None,credential_vault_ref:locator.clone()};
                request_plan::compose(&task,&[])?
            }else{RequestPlan{adapter_binding: connection.adapter_binding.clone(), purpose: connection.adapter_binding.as_ref().map(|_|CatalogPurpose::Translation),protocol:connection.protocol.clone(),endpoint:connection.endpoint.clone().ok_or_else(||AppError::new("missing_endpoint","test","尚未填写服务地址"))?,model:actual_model.clone(),source_text:"こんにちは。".into(),source_language:"ja".into(),target_language:"zh-CN".into(),messages:vec![json!({"role":"system","content":"Translate the source into Simplified Chinese. Return only the translation."}),json!({"role":"user","content":"こんにちは。"})],parameters:model.as_ref().map(|m|crate::domain::resolve_parameters(&[&m.parameters])).unwrap_or_default(),structured:false,stream:model.as_ref().and_then(|m|m.capabilities.get("streaming")).and_then(serde_json::Value::as_bool).unwrap_or_else(||crate::domain::model_config::default_request_streaming(connection)),timeout_ms:30000}};
            plan_identity=json!({"plan":plan,"credentials":credentials});
            let text=gateway.test_request(connection,&plan,locator.as_deref()).await?;
            if plan.structured&&request_plan::validate_study_text(&text,&plan.source_text).2!="structured"{return Err(AppError::new("invalid_structured_output","test","模型返回文字，但本用途的结构引用未通过校验"));}
            Ok(actual_model.into_iter().collect())
        }.await;
        let state = if result.is_ok() { "passed" } else { "failed" };
        for credential in &mut credentials {
            credential.outcome = state.into();
        }
        repository.record_model_test(ModelTestRecordInput {
            test_id,
            config_revision_id: configuration.revision_id.clone(),
            connection_id: connection.as_ref().map(|v| v.connection_id.clone()),
            model_id: model.as_ref().map(|v| v.model_id.clone()),
            binding_id: binding.as_ref().map(|v| v.binding_id.clone()),
            purpose: binding.as_ref().map(|v| v.purpose.clone()),
            stage: request.object_kind.clone(),
            request_semantics_hash: canonical_hash(&plan_identity)?,
            input_scope: "sample".into(),
            unit_id: None,
            text_revision: None,
            input_summary: json!({"sampleId":"greeting-ja-v1","scopeSemanticsHash":scope_hash}),
            response_summary: json!({"modelIds":result.as_ref().ok()}),
            output_validation: json!({"protocolValid":result.is_ok()}),
            state: state.into(),
            started_at_ms: started,
            completed_at_ms: clock.now_ms(),
            error_code: result.as_ref().err().map(|e| e.code.clone()),
            error_stage: result.as_ref().err().map(|e| e.stage.clone()),
            credentials,
        })?;
        model_ids.extend(result?);
    }
    model_ids.sort();
    model_ids.dedup();
    Ok(ConfigurationTestResult {
        revision_id: request.revision_id,
        object_kind: request.object_kind,
        object_id: request.object_id,
        status: "passed".into(),
        message: "指定范围的检查已实际完成；其他模型、用途与未使用凭据仍未测".into(),
        model_ids,
    })
}
