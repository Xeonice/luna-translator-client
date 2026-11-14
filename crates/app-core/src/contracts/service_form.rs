//! Read-only form interpretation. No saved-service or execution authorization is granted here.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServiceCredentialCapture {
    #[default]
    Unknown,
    None,
    ApiKey,
    BaiduAppSecret,
    YoudaoAppSecret,
    GoogleServiceAccountFile,
    GoogleOwnedGrant,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServiceFormTarget {
    Http {
        preset: ProviderPresetKey,
    },
    Offline {
        #[serde(rename = "engineId")]
        engine_id: String,
        #[serde(rename = "contractRevision")]
        contract_revision: u32,
    },
}
#[derive(Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceFormRequest {
    pub target: ServiceFormTarget,
    pub purpose: CatalogPurpose,
    #[serde(deserialize_with = "unique_values")]
    pub values: BTreeMap<String, CatalogScalar>,
}
impl std::fmt::Debug for ServiceFormRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceFormRequest")
            .field("target", &self.target)
            .field("purpose", &self.purpose)
            .field("field_ids", &self.values.keys().collect::<Vec<_>>())
            .finish()
    }
}
pub(super) fn unique_values<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, CatalogScalar>, D::Error> {
    struct Values;
    impl<'de> serde::de::Visitor<'de> for Values {
        type Value = BTreeMap<String, CatalogScalar>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("unique non-secret field values")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut values = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, CatalogScalar>()? {
                if values.len() >= 128 || values.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom(
                        "duplicate or excessive form fields",
                    ));
                }
            }
            Ok(values)
        }
    }
    d.deserialize_map(Values)
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceFormValueSource {
    User,
    Default,
    Inherited,
    Unset,
    Purpose,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceFormResolvedValue {
    pub value: Option<CatalogScalar>,
    pub source: ServiceFormValueSource,
    pub inherit_from: Option<ServiceFieldInheritance>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceFormIssue {
    TypeMismatch,
    InvalidChoice,
    InvalidNumber,
    InvalidUrl,
    EmptyValue,
    RequiredValueMissing,
    InactiveField,
    ConditionUnresolved,
    DependencyCycle,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceFormRow {
    pub descriptor: ServiceFieldDescriptor,
    pub visibility: CapabilityState,
    pub resolved: ServiceFormResolvedValue,
    /// Caller non-secret edit retained for correction; not necessarily effective for dependencies.
    pub input_value: Option<CatalogScalar>,
    pub input_allowed: bool,
    pub required: bool,
    pub issues: Vec<ServiceFormIssue>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceFormMaterialBlocker {
    pub descriptor: ConditionalCatalogBlocker,
    /// Supported means that this blocker condition applies, not that the service works.
    pub condition_state: CapabilityState,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceFormProjection {
    pub target: ServiceFormTarget,
    pub purpose: CatalogPurpose,
    #[serde(default)]
    pub credential_capture: ServiceCredentialCapture,
    pub fields: Vec<ServiceFormRow>,
    pub material_blockers: Vec<ServiceFormMaterialBlocker>,
    /// Adapter availability remains separate from form visibility and required material.
    pub implementation: CatalogDecision,
}
pub(super) fn typescript_declarations() -> String {
    [
        ServiceFormTarget::decl(&ts_rs::Config::default()),
        ServiceCredentialCapture::decl(&ts_rs::Config::default()),
        ServiceFormRequest::decl(&ts_rs::Config::default()),
        ServiceFormValueSource::decl(&ts_rs::Config::default()),
        ServiceFormResolvedValue::decl(&ts_rs::Config::default()),
        ServiceFormIssue::decl(&ts_rs::Config::default()),
        ServiceFormRow::decl(&ts_rs::Config::default()),
        ServiceFormMaterialBlocker::decl(&ts_rs::Config::default()),
        ServiceFormProjection::decl(&ts_rs::Config::default()),
    ]
    .iter()
    .map(|s| format!("\nexport {s}\n"))
    .collect()
}
