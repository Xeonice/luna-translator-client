use app_core::{
    contracts::*,
    domain::model_config::{service_codec::*, service_graph::*},
};
use serde_json::json;
use ts_rs::TS;

fn input() -> serde_json::Value {
    json!({"operationId":"operation","expectedProfileGeneration":1,"expectedDraftHeadRevision":0,
    "target":{"kind":"http","preset":{"presetId":"builtin.deepseek","presetRevision":1}},"purpose":"translation","values":{},"authentication":{"kind":"none"}})
}

#[test]
fn client_input_cannot_claim_activation_legacy_proofs_or_whole_graphs() {
    assert!(serde_json::from_value::<SaveServiceDraftInput>(input()).is_ok());
    for key in [
        "role",
        "revisionRole",
        "materializationSource",
        "legacyEligibilityOrigin",
        "configuration",
        "headers",
        "tokenUri",
    ] {
        let mut value = input();
        value[key] = json!("forbidden");
        assert!(serde_json::from_value::<SaveServiceDraftInput>(value).is_err());
    }
    let duplicate = serde_json::to_string(&input()).unwrap().replace(
        "\"values\":{}",
        "\"values\":{\"model\":\"a\",\"model\":\"b\"}",
    );
    assert!(serde_json::from_str::<SaveServiceDraftInput>(&duplicate).is_err());
    let mut value = input();
    value["values"] = json!({"api_key":"sentinel-private-input"});
    let parsed: SaveServiceDraftInput = serde_json::from_value(value).unwrap();
    assert!(!format!("{parsed:?}").contains("sentinel-private-input"));
}

#[test]
fn typed_scope_authentication_and_numeric_ownership_boundaries_are_closed() {
    for value in [
        json!({"kind":"global","gameId":"other"}),
        json!({"kind":"game","gameId":"a","scopeKey":"global"}),
        json!({"kind":"automatic"}),
    ] {
        assert!(serde_json::from_value::<ServiceScope>(value).is_err());
    }
    assert!(
        ServiceScope::Game { game_id: "".into() }
            .scope_key()
            .is_err()
    );
    assert_eq!(
        ServiceScope::Game {
            game_id: "旧 游戏".into()
        }
        .scope_key()
        .unwrap(),
        "game:旧 游戏"
    );
    for value in [
        json!({"kind":"none","poolId":"hidden"}),
        json!({"kind":"existing_pool","poolId":"p","poolRevision":0}),
        json!({"kind":"google_owned_grant","authorization":{"grantId":"g","authorizationEpoch":0}}),
        json!({"kind":"api_key","key":"sentinel"}),
    ] {
        assert!(serde_json::from_value::<ServiceAuthenticationSelection>(value).is_err());
    }
    for generation in [json!(0), json!(-1), json!(4294967296u64)] {
        let mut value = input();
        value["expectedProfileGeneration"] = generation;
        assert!(serde_json::from_value::<SaveServiceDraftInput>(value).is_err());
    }
}

#[test]
fn generated_service_declarations_match_actual_optional_camelcase_payloads() {
    let options = HttpServiceOptionsV1::decl(&ts_rs::Config::default());
    assert!(options.contains("organizationId?"));
    assert!(options.contains("projectId?"));
    assert!(!options.contains("organization_id"));
    let source = SavedServiceRef::decl(&ts_rs::Config::default());
    assert!(source.contains("source: HttpConfigRef"));
    let local = EngineTarget::decl(&ts_rs::Config::default());
    assert!(local.contains("modelId"));
    assert!(local.contains("offlineConfigId"));
    assert!(!local.contains("revisionId"));
    let generated = app_core::contracts::typescript_declarations();
    assert!(generated.contains("export type ConfigurationV3"));
    assert!(generated.contains("export type SaveServiceDraftInput"));
}

#[test]
fn sql_json_carrier_is_checked_before_duplicate_information_can_be_discarded() {
    assert!(validate_unique_service_json(br#"{"mode":"set","value":{"x":1}}"#).is_ok());
    assert!(validate_unique_service_json(br#"{"mode":"set","value":{"x":1,"x":2}}"#).is_err());
    assert!(
        validate_unique_service_json(br#"{"protocol":"openai_compatible","protocol":"anthropic"}"#)
            .is_err()
    );
}
