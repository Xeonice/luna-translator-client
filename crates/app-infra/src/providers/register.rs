//! The only built-in assembly entry: all declared targets, with actual implementation state kept separate.
use super::{
    presets, profiles,
    registry::{ProviderRegistry, ProviderRegistryBuilder},
};
use app_core::contracts::*;
pub fn protocol_auth_kinds(id: &str) -> Vec<CatalogAuthKind> {
    use CatalogAuthKind::*;
    match id {
        "openai_compatible" => vec![None, BearerApiKey],
        "openai_responses" => vec![BearerApiKey],
        "anthropic" => vec![AnthropicApiKey],
        "gemini_generate_content" => vec![GoogleApiKey],
        "ollama_native" => vec![None, BearerApiKey],
        "lmstudio_native_v1" => vec![None, BearerApiKey],
        "azure_translator" => vec![AzureSubscriptionKey],
        "deepl_v2" => vec![DeeplAuthKey],
        "google_translate_v2" => vec![GoogleApiKey],
        "google_translate_v3" => vec![GoogleOauth, GoogleServiceAccount],
        "baidu_vip" => vec![BaiduAppSecret],
        "youdao_v3" => vec![YoudaoAppSecret],
        "tencent_tokenhub_translation" => vec![BearerApiKey],
        _ => vec![],
    }
}
pub fn builtin_protocols() -> Vec<ProtocolDescriptor> {
    vec![
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "openai_compatible".into(),
                contract_revision: 1,
            },
            label: "OpenAI Chat Completions".into(),
            family: CatalogFamily::Llm,
            auth_kinds: protocol_auth_kinds("openai_compatible"),
            model_identity: CatalogModelIdentity::Required,
            wire_formats: vec![CatalogWireFormat::Json, CatalogWireFormat::Sse],
            allowed_purposes: vec![
                CatalogPurpose::Translation,
                CatalogPurpose::Grammar,
                CatalogPurpose::ContextWord,
            ],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Supported,
                        reason: None,
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Supported,
                        reason: None,
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Supported,
                        reason: None,
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Supported,
                reason: Some(CatalogReason::ExistingAdapterRequiresRegistryMigration),
            },
            source_refs: vec!["protocol.openai_compatible".into()],
        },
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "openai_responses".into(),
                contract_revision: 1,
            },
            label: "OpenAI Responses".into(),
            family: CatalogFamily::Llm,
            auth_kinds: protocol_auth_kinds("openai_responses"),
            model_identity: CatalogModelIdentity::Required,
            wire_formats: vec![CatalogWireFormat::Json, CatalogWireFormat::TypedSse],
            allowed_purposes: vec![
                CatalogPurpose::Translation,
                CatalogPurpose::Grammar,
                CatalogPurpose::ContextWord,
            ],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Unsupported,
                reason: Some(CatalogReason::AdapterNotImplemented),
            },
            source_refs: vec!["protocol.openai_responses".into()],
        },
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "anthropic".into(),
                contract_revision: 1,
            },
            label: "Anthropic Messages".into(),
            family: CatalogFamily::Llm,
            auth_kinds: protocol_auth_kinds("anthropic"),
            model_identity: CatalogModelIdentity::Required,
            wire_formats: vec![CatalogWireFormat::Json, CatalogWireFormat::Sse],
            allowed_purposes: vec![
                CatalogPurpose::Translation,
                CatalogPurpose::Grammar,
                CatalogPurpose::ContextWord,
            ],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Supported,
                        reason: None,
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Supported,
                        reason: None,
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Supported,
                        reason: None,
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Supported,
                reason: Some(CatalogReason::ExistingAdapterRequiresRegistryMigration),
            },
            source_refs: vec!["protocol.anthropic".into()],
        },
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "gemini_generate_content".into(),
                contract_revision: 1,
            },
            label: "Gemini 原生 generateContent".into(),
            family: CatalogFamily::Llm,
            auth_kinds: protocol_auth_kinds("gemini_generate_content"),
            model_identity: CatalogModelIdentity::Required,
            wire_formats: vec![CatalogWireFormat::Json, CatalogWireFormat::Sse],
            allowed_purposes: vec![
                CatalogPurpose::Translation,
                CatalogPurpose::Grammar,
                CatalogPurpose::ContextWord,
            ],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Unsupported,
                reason: Some(CatalogReason::AdapterNotImplemented),
            },
            source_refs: vec!["protocol.gemini_generate_content".into()],
        },
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "ollama_native".into(),
                contract_revision: 1,
            },
            label: "Ollama 原生 /api/chat".into(),
            family: CatalogFamily::Llm,
            auth_kinds: protocol_auth_kinds("ollama_native"),
            model_identity: CatalogModelIdentity::Required,
            wire_formats: vec![CatalogWireFormat::Json, CatalogWireFormat::Ndjson],
            allowed_purposes: vec![
                CatalogPurpose::Translation,
                CatalogPurpose::Grammar,
                CatalogPurpose::ContextWord,
            ],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Unsupported,
                reason: Some(CatalogReason::AdapterNotImplemented),
            },
            source_refs: vec!["protocol.ollama_native".into()],
        },
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "lmstudio_native_v1".into(),
                contract_revision: 1,
            },
            label: "LM Studio 原生 /api/v1/chat".into(),
            family: CatalogFamily::Llm,
            auth_kinds: protocol_auth_kinds("lmstudio_native_v1"),
            model_identity: CatalogModelIdentity::Required,
            wire_formats: vec![CatalogWireFormat::Json, CatalogWireFormat::Sse],
            allowed_purposes: vec![
                CatalogPurpose::Translation,
                CatalogPurpose::Grammar,
                CatalogPurpose::ContextWord,
            ],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Unsupported,
                reason: Some(CatalogReason::AdapterNotImplemented),
            },
            source_refs: vec!["protocol.lmstudio_native_v1".into()],
        },
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "azure_translator".into(),
                contract_revision: 1,
            },
            label: "Microsoft Azure Translator".into(),
            family: CatalogFamily::Translation,
            auth_kinds: protocol_auth_kinds("azure_translator"),
            model_identity: CatalogModelIdentity::NotApplicable,
            wire_formats: vec![CatalogWireFormat::Json],
            allowed_purposes: vec![CatalogPurpose::Translation],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Supported,
                        reason: None,
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Supported,
                reason: Some(CatalogReason::ExistingAdapterRequiresRegistryMigration),
            },
            source_refs: vec!["protocol.azure_translator".into()],
        },
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "deepl_v2".into(),
                contract_revision: 1,
            },
            label: "DeepL Text API".into(),
            family: CatalogFamily::Translation,
            auth_kinds: protocol_auth_kinds("deepl_v2"),
            model_identity: CatalogModelIdentity::NotApplicable,
            wire_formats: vec![CatalogWireFormat::Json],
            allowed_purposes: vec![CatalogPurpose::Translation],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Unsupported,
                reason: Some(CatalogReason::AdapterNotImplemented),
            },
            source_refs: vec!["protocol.deepl_v2".into()],
        },
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "google_translate_v2".into(),
                contract_revision: 1,
            },
            label: "Google Cloud Translation Basic v2".into(),
            family: CatalogFamily::Translation,
            auth_kinds: protocol_auth_kinds("google_translate_v2"),
            model_identity: CatalogModelIdentity::NotApplicable,
            wire_formats: vec![CatalogWireFormat::Json],
            allowed_purposes: vec![CatalogPurpose::Translation],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Unsupported,
                reason: Some(CatalogReason::AdapterNotImplemented),
            },
            source_refs: vec!["protocol.google_translate_v2".into()],
        },
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "google_translate_v3".into(),
                contract_revision: 1,
            },
            label: "Google Cloud Translation Advanced v3".into(),
            family: CatalogFamily::Translation,
            auth_kinds: protocol_auth_kinds("google_translate_v3"),
            model_identity: CatalogModelIdentity::Optional,
            wire_formats: vec![CatalogWireFormat::Json],
            allowed_purposes: vec![CatalogPurpose::Translation],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Unsupported,
                reason: Some(CatalogReason::AdapterNotImplemented),
            },
            source_refs: vec!["protocol.google_translate_v3".into()],
        },
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "baidu_vip".into(),
                contract_revision: 1,
            },
            label: "百度通用翻译".into(),
            family: CatalogFamily::Translation,
            auth_kinds: protocol_auth_kinds("baidu_vip"),
            model_identity: CatalogModelIdentity::NotApplicable,
            wire_formats: vec![CatalogWireFormat::FormJson],
            allowed_purposes: vec![CatalogPurpose::Translation],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Unsupported,
                reason: Some(CatalogReason::AdapterNotImplemented),
            },
            source_refs: vec!["protocol.baidu_vip".into()],
        },
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "youdao_v3".into(),
                contract_revision: 1,
            },
            label: "有道文本翻译".into(),
            family: CatalogFamily::Translation,
            auth_kinds: protocol_auth_kinds("youdao_v3"),
            model_identity: CatalogModelIdentity::NotApplicable,
            wire_formats: vec![CatalogWireFormat::FormJson],
            allowed_purposes: vec![CatalogPurpose::Translation],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Unsupported,
                reason: Some(CatalogReason::AdapterNotImplemented),
            },
            source_refs: vec!["protocol.youdao_v3".into()],
        },
        ProtocolDescriptor {
            key: ProtocolKey {
                protocol_id: "tencent_tokenhub_translation".into(),
                contract_revision: 1,
            },
            label: "腾讯 TokenHub 专用翻译 /api/translations".into(),
            family: CatalogFamily::Translation,
            auth_kinds: protocol_auth_kinds("tencent_tokenhub_translation"),
            model_identity: CatalogModelIdentity::Required,
            wire_formats: vec![CatalogWireFormat::Json, CatalogWireFormat::TranslationDelta],
            allowed_purposes: vec![CatalogPurpose::Translation],
            capabilities: vec![
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Translation,
                    support: CatalogDecision {
                        state: CapabilityState::Unknown,
                        reason: Some(CatalogReason::AdapterNotImplemented),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::Grammar,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
                CatalogPurposeCapability {
                    purpose: CatalogPurpose::ContextWord,
                    support: CatalogDecision {
                        state: CapabilityState::Unsupported,
                        reason: Some(CatalogReason::PurposeNotSupported),
                    },
                },
            ],
            implementation: CatalogDecision {
                state: CapabilityState::Unsupported,
                reason: Some(CatalogReason::AdapterNotImplemented),
            },
            source_refs: vec!["protocol.tencent_tokenhub_translation".into()],
        },
    ]
}
pub fn builtin_sources() -> Vec<CatalogSource> {
    vec![
CatalogSource { source_id:"prd".into(), kind:CatalogSourceKind::LocalContract, reference:"docs/product/model-configuration-prd.md".into(), checked_at:"2026-10-30".into(), scope:"AI-CFG typed credentials, independent purposes, explicit versions and only applicable parameters".into() },
CatalogSource { source_id:"auth".into(), kind:CatalogSourceKind::LocalContract, reference:"docs/architecture/japanese-study-and-llm.md".into(), checked_at:"2026-10-30".into(), scope:"Exact registered auth kinds; salt/signing/refresh are internal".into() },
CatalogSource { source_id:"registry".into(), kind:CatalogSourceKind::LocalContract, reference:"docs/architecture/model-service-registry-design.json".into(), checked_at:"2026-10-30".into(), scope:"Exact protocol/profile/preset/engine identities and purpose ceilings".into() },
CatalogSource { source_id:"offline".into(), kind:CatalogSourceKind::LocalContract, reference:"docs/product/model-configuration-prd.md#offline-translation-prd".into(), checked_at:"2026-10-30".into(), scope:"Four engine conditions, real resource observation and unknown blocking".into() },
CatalogSource { source_id:"deepseek".into(), kind:CatalogSourceKind::Official, reference:"https://api-docs.deepseek.com/".into(), checked_at:"2026-10-30".into(), scope:"OpenAI base URL, API Key and actual model input; examples are not model defaults".into() },
CatalogSource { source_id:"qwen".into(), kind:CatalogSourceKind::Official, reference:"https://help.aliyun.com/zh/model-studio/get-api-key/".into(), checked_at:"2026-10-30".into(), scope:"Region-specific key/address/model; Beijing compatible base verified; unverified built-in sites are independently blocked as incomplete preset materials; user endpoint input is required only for an explicit custom-server configuration".into() },
CatalogSource { source_id:"ark".into(), kind:CatalogSourceKind::Official, reference:"https://ark.volcengine.com/region:cn-beijing/docs/base-url-and-authentication.md".into(), checked_at:"2026-10-30".into(), scope:"Official Markdown fetched directly; Beijing data-plane base and API-key/model contract verified. Other sites not guessed.".into() },
CatalogSource { source_id:"gemini_compatible".into(), kind:CatalogSourceKind::Official, reference:"https://ai.google.dev/gemini-api/docs/openai".into(), checked_at:"2026-10-30".into(), scope:"Gemini API key, compatible base URL and actual model".into() },
CatalogSource { source_id:"gemini_native".into(), kind:CatalogSourceKind::Official, reference:"https://ai.google.dev/api/generate-content".into(), checked_at:"2026-10-30".into(), scope:"Official generateContent reference opened; v1beta host and required model verified.".into() },
CatalogSource { source_id:"openrouter".into(), kind:CatalogSourceKind::Official, reference:"https://openrouter.ai/docs/quickstart".into(), checked_at:"2026-10-30".into(), scope:"Base URL, API Key and explicit model; app attribution optional".into() },
CatalogSource { source_id:"openai".into(), kind:CatalogSourceKind::Official, reference:"https://developers.openai.com/api/reference/overview".into(), checked_at:"2026-10-30".into(), scope:"Bearer key, v1 host; organization/project headers conditionally needed; Chat and Responses remain separate presets".into() },
CatalogSource { source_id:"anthropic".into(), kind:CatalogSourceKind::Official, reference:"https://platform.claude.com/docs/en/api/overview".into(), checked_at:"2026-10-30".into(), scope:"API host, API Key and model; workspace selection conditional; current registry keeps its exact Anthropic API-key contract".into() },
CatalogSource { source_id:"ollama".into(), kind:CatalogSourceKind::Official, reference:"https://docs.ollama.com/api/introduction".into(), checked_at:"2026-10-30".into(), scope:"Different native/compatible local base URLs; local no-auth and remote/cloud API-Key conditions".into() },
CatalogSource { source_id:"lmstudio".into(), kind:CatalogSourceKind::Official, reference:"https://lmstudio.ai/docs/developer/openai-compat".into(), checked_at:"2026-10-30".into(), scope:"Compatible local URL example assumes port 1234; editable assumption, not installed-server guarantee".into() },
CatalogSource { source_id:"lmstudio_native".into(), kind:CatalogSourceKind::Official, reference:"https://lmstudio.ai/docs/developer/rest/quickstart".into(), checked_at:"2026-10-30".into(), scope:"Official quickstart confirms default localhost:1234, api/v1/chat and optional server authentication; model example is not a chosen default.".into() },
CatalogSource { source_id:"azure".into(), kind:CatalogSourceKind::Official, reference:"https://learn.microsoft.com/en-us/azure/ai-services/translator/text-translation/reference/authentication".into(), checked_at:"2026-10-30".into(), scope:"Subscription key; regional/multiservice region required, global region optional; custom endpoints need explicit selection".into() },
CatalogSource { source_id:"deepl".into(), kind:CatalogSourceKind::Official, reference:"https://developers.deepl.com/docs/getting-started/auth".into(), checked_at:"2026-10-30".into(), scope:"Free versus Pro host and authentication key; no model ID required".into() },
CatalogSource { source_id:"google_v2".into(), kind:CatalogSourceKind::Official, reference:"https://docs.cloud.google.com/translate/docs/reference/rest/v2/translate".into(), checked_at:"2026-10-30".into(), scope:"Exact v2 translate URL; target language and API-key route already registered".into() },
CatalogSource { source_id:"google_v3".into(), kind:CatalogSourceKind::Official, reference:"https://docs.cloud.google.com/translate/docs/reference/rest/v3/projects.locations/translateText".into(), checked_at:"2026-10-30".into(), scope:"translate.googleapis.com/v3, project/location and optional model; OAuth/IAM contract remains local authority".into() },
CatalogSource { source_id:"baidu".into(), kind:CatalogSourceKind::OfficialSearchIndex, reference:"https://fanyi-api.baidu.com/doc_bd/21".into(), checked_at:"2026-10-30".into(), scope:"Current official indexed doc explicitly identifies HTTPS vip/translate endpoint and APPID+secret signing; page HTML shell omitted body, no live invocation.".into() },
CatalogSource { source_id:"youdao".into(), kind:CatalogSourceKind::Official, reference:"https://ai.youdao.com/DOCSIRMA/html/trans/api/wbfy/index.html".into(), checked_at:"2026-10-30".into(), scope:"Official HTML fetched directly (60366 bytes); text-translation HTTPS endpoint, appKey/secret and signing fields verified. No live call.".into() },
CatalogSource { source_id:"tokenhub".into(), kind:CatalogSourceKind::Official, reference:"https://cloud.tencent.com/document/product/1823/132252".into(), checked_at:"2026-10-30".into(), scope:"Separate Chat and dedicated translations routes, Bearer key and explicit model; no sample model silently selected".into() },
CatalogSource { source_id:"deepl_translate".into(), kind:CatalogSourceKind::Official, reference:"https://developers.deepl.com/api-reference/translate/request-translation".into(), checked_at:"2026-10-30".into(), scope:"Translation context is an optional service-specific field; not generic LLM prompting.".into() },
CatalogSource { source_id:"protocol.openai_compatible".into(), kind:CatalogSourceKind::Official, reference:"https://api-docs.deepseek.com/".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
CatalogSource { source_id:"protocol.openai_responses".into(), kind:CatalogSourceKind::Official, reference:"https://developers.openai.com/api/docs/guides/migrate-to-responses".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
CatalogSource { source_id:"protocol.anthropic".into(), kind:CatalogSourceKind::Official, reference:"https://platform.claude.com/docs/en/api/messages".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
CatalogSource { source_id:"protocol.gemini_generate_content".into(), kind:CatalogSourceKind::Official, reference:"https://ai.google.dev/api/generate-content".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
CatalogSource { source_id:"protocol.ollama_native".into(), kind:CatalogSourceKind::Official, reference:"https://docs.ollama.com/api/chat".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
CatalogSource { source_id:"protocol.lmstudio_native_v1".into(), kind:CatalogSourceKind::Official, reference:"https://lmstudio.ai/docs/developer/rest/chat".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
CatalogSource { source_id:"protocol.azure_translator".into(), kind:CatalogSourceKind::Official, reference:"https://learn.microsoft.com/en-us/azure/ai-services/translator/text-translation-overview".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
CatalogSource { source_id:"protocol.deepl_v2".into(), kind:CatalogSourceKind::Official, reference:"https://developers.deepl.com/api-reference/translate/request-translation".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
CatalogSource { source_id:"protocol.google_translate_v2".into(), kind:CatalogSourceKind::Official, reference:"https://docs.cloud.google.com/translate/docs/reference/rest/v2/translate".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
CatalogSource { source_id:"protocol.google_translate_v3".into(), kind:CatalogSourceKind::Official, reference:"https://docs.cloud.google.com/translate/docs/authentication".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
CatalogSource { source_id:"protocol.baidu_vip".into(), kind:CatalogSourceKind::Official, reference:"https://fanyi-api.baidu.com/doc_bd/21".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
CatalogSource { source_id:"protocol.youdao_v3".into(), kind:CatalogSourceKind::Official, reference:"https://ai.youdao.com/DOCSIRMA/html/trans/api/wbfy/index.html".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
CatalogSource { source_id:"protocol.tencent_tokenhub_translation".into(), kind:CatalogSourceKind::Official, reference:"https://cloud.tencent.com/document/product/1823/132252".into(), checked_at:"2026-10-30".into(), scope:"协议目录来源；未声明本次真实服务验证".into() },
 ]
}
pub fn builtin_registry() -> AppResult<ProviderRegistry> {
    let mut builder = ProviderRegistryBuilder::new();
    for source in builtin_sources() {
        builder.register_source(source)?;
    }
    for protocol in builtin_protocols() {
        builder.register_protocol(protocol)?;
    }
    for profile in profiles::builtin_profiles() {
        builder.register_profile(profile)?;
    }
    for preset in presets::builtin_presets() {
        builder.register_builtin_preset(preset)?;
    }
    for engine in presets::builtin_offline_engines() {
        builder.register_offline_engine(engine)?;
    }
    builder.freeze()
}
