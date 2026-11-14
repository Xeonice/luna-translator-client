use app_core::contracts::{SettingsPageIntent, SettingsPageRequest};

#[test]
fn settings_navigation_only_accepts_the_translation_destination() {
    let intent: SettingsPageIntent = serde_json::from_str("\"translation\"").unwrap();
    assert_eq!(intent, SettingsPageIntent::Translation);
    assert_eq!(serde_json::to_string(&intent).unwrap(), "\"translation\"");
    for unsupported in ["general", "network", "connection", "settings", "Translation", ""] {
        assert!(serde_json::from_value::<SettingsPageIntent>(serde_json::json!(unsupported)).is_err());
    }
}

#[test]
fn settings_navigation_request_round_trips_the_exact_wire_shape_and_revision_range() {
    for revision in [1, u32::MAX] {
        let request = SettingsPageRequest {
            revision,
            intent: SettingsPageIntent::Translation,
        };
        let wire = serde_json::json!({ "revision": revision, "intent": "translation" });
        assert_eq!(serde_json::to_value(&request).unwrap(), wire);
        assert_eq!(serde_json::from_value::<SettingsPageRequest>(wire).unwrap(), request);
    }
    for invalid in [
        serde_json::json!({ "revision": -1, "intent": "translation" }),
        serde_json::json!({ "revision": 4294967296_u64, "intent": "translation" }),
        serde_json::json!({ "revision": "1", "intent": "translation" }),
        serde_json::json!({ "revision": 1, "intent": "general" }),
        serde_json::json!({ "intent": "translation" }),
    ] {
        assert!(serde_json::from_value::<SettingsPageRequest>(invalid).is_err());
    }
}

#[test]
fn settings_navigation_types_are_exported_by_the_actual_rust_contract_exporter() {
    let declarations = app_core::contracts::typescript_declarations();
    assert!(declarations.contains("export type SettingsPageIntent = \"translation\";"));
    assert!(declarations.contains(
        "export type SettingsPageRequest = { revision: number, intent: SettingsPageIntent, };"
    ));
}
