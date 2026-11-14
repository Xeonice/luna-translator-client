//! Synthetic loopback wires; not live-provider or repository-authority proof.
use super::*;
use app_core::{
    contracts::*,
    domain::model_config::credentials::{CredentialVersionRef, ProviderAuthLease, SecretText},
};
use serde_json::json;
use std::{collections::BTreeMap, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn frozen(protocol: &str, options: HttpServiceOptionsV1) -> FrozenHttpServicePlan<RequestPlan> {
    let binding = AdapterBinding {
        protocol: ProtocolKey {
            protocol_id: protocol.into(),
            contract_revision: 1,
        },
        profile: CompatibilityProfileKey {
            profile_id: "synthetic-wire-profile".into(),
            profile_revision: 1,
        },
    };
    let connection: app_core::domain::model_config::service_graph::ConnectionV3 =
        serde_json::from_value(json!({
            "connectionId":"connection", "displayName":"Synthetic wire", "protocol":protocol,
            "deployment":"cloud", "authMode":"api_key", "endpoint":"http://127.0.0.1:1",
            "proxyMode":"disabled", "networkMetadata":{}, "availabilityIntent":true,
            "adapterBinding":binding, "httpOptions":options, "origin":{"kind":"custom"}
        }))
        .unwrap();
    let actual_model = if protocol == "azure_translator" {
        None
    } else {
        Some("synthetic-model".to_owned())
    };
    let model = serde_json::from_value(json!({"modelId":"model","connectionId":"connection",
        "actualModelId":actual_model,"displayName":"Synthetic model","selectionSource":"manual",
        "capabilities":{},"parameters":{},"defaultTemplates":{},"requestDefaults":{}}))
    .unwrap();
    FrozenHttpServicePlan {
        http_options: options,
        origin: connection.origin.clone().unwrap(),
        connection,
        model,
        template: None,
        request: RequestPlan {
            protocol: protocol.into(),
            endpoint: "http://127.0.0.1:1".into(),
            model: actual_model,
            source_text: "雨がやんだ。".into(),
            source_language: "ja".into(),
            target_language: "zh-Hans".into(),
            messages: vec![
                json!({"role":"system","content":"Translate"}),
                json!({"role":"user","content":"Synthetic"}),
            ],
            parameters: BTreeMap::new(),
            structured: false,
            stream: false,
            timeout_ms: 2000,
            adapter_binding: Some(binding),
            purpose: Some(CatalogPurpose::Translation),
        },
        reference_material: Default::default(),
    }
}

#[test]
fn header_identity_injection_and_legacy_region_shadow_are_rejected_or_isolated() {
    let mut value = frozen(
        "openai_compatible",
        HttpServiceOptionsV1::OpenaiCompatible {
            organization_id: Some("org-test".into()),
            project_id: Some("project-test".into()),
        },
    );
    let headers = project(&value).unwrap().headers;
    assert_eq!(headers["openai-organization"], "org-test");
    assert_eq!(headers["openai-project"], "project-test");
    value.connection.http_options = Some(HttpServiceOptionsV1::Anthropic { workspace_id: None });
    assert_eq!(
        project(&value).err().unwrap().code,
        "service_plan_identity_mismatch"
    );
    let value = frozen(
        "openai_responses",
        HttpServiceOptionsV1::OpenaiResponses {
            organization_id: Some("org\r\nAuthorization: injected".into()),
            project_id: None,
        },
    );
    assert_eq!(project(&value).err().unwrap().code, "invalid_http_options");
    let mut value = frozen(
        "azure_translator",
        HttpServiceOptionsV1::AzureTranslator {
            resource_type: Some(AzureResourceType::Regional),
            region: Some("japaneast".into()),
        },
    );
    value
        .connection
        .network_metadata
        .insert("region".into(), json!("old-region"));
    let prepared = project(&value).unwrap();
    assert_eq!(
        prepared.headers["ocp-apim-subscription-region"],
        "japaneast"
    );
    assert!(!prepared.connection.network_metadata.contains_key("region"));
    // HTTP projection cannot accidentally inherit another request's headers.
    let plain = frozen("ollama_native", HttpServiceOptionsV1::OllamaNative {});
    assert!(project(&plain).unwrap().headers.is_empty());
}

#[test]
fn dedicated_reference_data_survives_and_unsupported_material_never_disappears() {
    let mut value = frozen(
        "tencent_tokenhub_translation",
        HttpServiceOptionsV1::TencentTokenhubTranslation {},
    );
    value
        .reference_material
        .context
        .push(json!({"translations":[{"text":"前一句","targetLanguage":"zh-Hans"}]}));
    value
        .reference_material
        .glossary
        .push(json!({"source":"雨","target":"雨水"}));
    let prepared = project(&value).unwrap();
    let options = prepared.options.tokenhub.unwrap();
    let data: serde_json::Value = serde_json::from_str(options.context.as_ref().unwrap()).unwrap();
    assert_eq!(data, json!(value.reference_material.context));
    assert!(!options.context.unwrap().contains("source"));
    assert_eq!(options.references[0].text, "雨");
    assert_eq!(options.references[0].translation, "雨水");
    assert!(options.glossary_ids.is_empty());
    let mut plain = frozen(
        "google_translate_v2",
        HttpServiceOptionsV1::GoogleTranslateV2 {},
    );
    plain.reference_material = value.reference_material;
    assert_eq!(
        project(&plain).err().unwrap().code,
        "reference_not_supported"
    );
}

#[tokio::test]
async fn actual_http_wires_send_only_the_selected_typed_option_headers() {
    let cases = [
        (
            "openai_compatible",
            HttpServiceOptionsV1::OpenaiCompatible {
                organization_id: Some("org-real-wire".into()),
                project_id: Some("project-real-wire".into()),
            },
            json!({"choices":[{"message":{"content":"译文"},"finish_reason":"stop"}]}),
        ),
        (
            "openai_responses",
            HttpServiceOptionsV1::OpenaiResponses {
                organization_id: Some("org-real-wire".into()),
                project_id: Some("project-real-wire".into()),
            },
            json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"译文"}]}]}),
        ),
        (
            "anthropic",
            HttpServiceOptionsV1::Anthropic {
                workspace_id: Some("wrkspc_real_wire".into()),
            },
            json!({"content":[{"type":"text","text":"译文"}],"stop_reason":"end_turn"}),
        ),
        (
            "azure_translator",
            HttpServiceOptionsV1::AzureTranslator {
                resource_type: Some(AzureResourceType::Regional),
                region: Some("japaneast".into()),
            },
            json!([{"translations":[{"text":"译文"}]}]),
        ),
    ];
    for (protocol, options, response) in cases {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut data = Vec::new();
            let mut bytes = [0_u8; 2048];
            let end = loop {
                let read = socket.read(&mut bytes).await.unwrap();
                assert!(read > 0);
                data.extend_from_slice(&bytes[..read]);
                assert!(data.len() < 65536);
                if let Some(end) = data.windows(4).position(|part| part == b"\r\n\r\n") {
                    break end + 4;
                }
            };
            let head = String::from_utf8(data[..end].to_vec())
                .unwrap()
                .to_ascii_lowercase();
            let length = head
                .lines()
                .find_map(|line| {
                    line.strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            while data.len() < end + length {
                let read = socket.read(&mut bytes).await.unwrap();
                assert!(read > 0);
                data.extend_from_slice(&bytes[..read]);
            }
            let body = serde_json::to_vec(&response).unwrap();
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(header.as_bytes()).await.unwrap();
            socket.write_all(&body).await.unwrap();
            head
        });
        let mut value = frozen(protocol, options);
        value.connection.endpoint = Some(format!("http://{address}"));
        value.request.endpoint = format!("http://{address}");
        if protocol == "azure_translator" {
            value
                .connection
                .network_metadata
                .insert("region".into(), json!("old-region"));
        }
        let prepared = project(&value).unwrap();
        let source = CredentialVersionRef {
            credential_id: "synthetic-key".into(),
            credential_version_id: "synthetic-version".into(),
        };
        let key = SecretText::from_bytes(b"synthetic-only-key".to_vec()).unwrap();
        let auth = ProviderAuthLease::ApiKey {
            source: &source,
            key: &key,
        };
        let output = tokio::time::timeout(
            Duration::from_secs(3),
            crate::providers::adapters::dispatch::execute_with_headers(
                &value.request,
                &prepared.connection,
                CatalogPurpose::Translation,
                &auth,
                &prepared.options,
                &prepared.headers,
                |_| {},
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(output.text, "译文");
        let head = server.await.unwrap();
        for (name, header) in &prepared.headers {
            assert!(head.contains(&format!(
                "{name}: {}\r\n",
                header.to_str().unwrap().to_ascii_lowercase()
            )));
        }
        assert!(!head.contains("old-region"));
        if protocol == "anthropic" {
            assert!(!head.contains("openai-project"));
            assert!(head.contains("x-api-key: synthetic-only-key"));
        }
    }
}
