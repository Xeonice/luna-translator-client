//! Pure catalog validation. Descriptions never grant runtime authorization.
use crate::contracts::*;
use std::collections::{BTreeMap, BTreeSet};

fn invalid(code: &str, message: &str) -> AppError {
    AppError::new(code, "service_catalog", message)
}
/// One authentication interpretation shared by form presentation and draft compilation.
pub fn service_form_auth_mode(
    protocol: &str,
    default_kind: &CatalogAuthKind,
    auth_mode: Option<&str>,
    google_method: Option<&str>,
) -> AppResult<&'static str> {
    if protocol == "google_translate_v3" {
        return match google_method {
            Some("oauth") => Ok("google_oauth"),
            Some("service_account") => Ok("google_service_account"),
            None => Ok("none"),
            _ => Err(invalid("invalid_service_authentication", "Google认证方式无效")),
        };
    }
    match auth_mode {
        Some("none") => Ok("none"),
        Some("api_key") => Ok("api_key"),
        Some("baidu_app_secret") => Ok("baidu_app_secret"),
        Some("youdao_app_secret") => Ok("youdao_app_secret"),
        None => Ok(match default_kind {
            CatalogAuthKind::None => "none",
            CatalogAuthKind::BaiduAppSecret => "baidu_app_secret",
            CatalogAuthKind::YoudaoAppSecret => "youdao_app_secret",
            _ => "api_key",
        }),
        _ => Err(invalid("invalid_service_authentication", "服务认证方式无效")),
    }
}
pub fn service_credential_capture(
    protocol: &str,
    default_kind: &CatalogAuthKind,
    auth_mode: Option<&str>,
    google_method: Option<&str>,
) -> AppResult<ServiceCredentialCapture> {
    if protocol == "google_translate_v3" && google_method.is_none() {
        return Ok(ServiceCredentialCapture::Unknown);
    }
    Ok(match service_form_auth_mode(protocol, default_kind, auth_mode, google_method)? {
        "none" => ServiceCredentialCapture::None,
        "api_key" => ServiceCredentialCapture::ApiKey,
        "baidu_app_secret" => ServiceCredentialCapture::BaiduAppSecret,
        "youdao_app_secret" => ServiceCredentialCapture::YoudaoAppSecret,
        "google_service_account" => ServiceCredentialCapture::GoogleServiceAccountFile,
        "google_oauth" => ServiceCredentialCapture::GoogleOwnedGrant,
        _ => return Err(invalid("unsupported_service_authentication", "认证材料接入方式尚不支持")),
    })
}
pub fn validate_catalog_id(id: &str) -> AppResult<()> {
    if id.is_empty()
        || id.len() > 160
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(invalid(
            "invalid_catalog_id",
            "服务目录身份必须为有界且无控制字符的稳定标识",
        ));
    }
    Ok(())
}
pub fn validate_catalog_label(label: &str) -> AppResult<()> {
    if label.trim().is_empty() || label.len() > 512 || label.chars().any(char::is_control) {
        return Err(invalid(
            "invalid_catalog_label",
            "服务名称为空、过长或包含控制字符",
        ));
    }
    Ok(())
}
pub fn validate_protocol_key(key: &ProtocolKey) -> AppResult<()> {
    validate_catalog_id(&key.protocol_id)?;
    if key.contract_revision == 0 {
        return Err(invalid(
            "invalid_catalog_revision",
            "协议契约版本必须为正整数",
        ));
    }
    Ok(())
}
pub fn validate_profile_key(key: &CompatibilityProfileKey) -> AppResult<()> {
    validate_catalog_id(&key.profile_id)?;
    if key.profile_revision == 0 {
        return Err(invalid(
            "invalid_catalog_revision",
            "兼容规则版本必须为正整数",
        ));
    }
    Ok(())
}
pub fn validate_binding(binding: &AdapterBinding) -> AppResult<()> {
    validate_protocol_key(&binding.protocol)?;
    validate_profile_key(&binding.profile)
}
pub fn validate_preset_key(key: &ProviderPresetKey, namespace: CatalogNamespace) -> AppResult<()> {
    validate_catalog_id(&key.preset_id)?;
    let prefix = match namespace {
        CatalogNamespace::Builtin => "builtin.",
        CatalogNamespace::User => "user.",
    };
    if !key.preset_id.starts_with(prefix)
        || key.preset_id.len() == prefix.len()
        || key.preset_revision == 0
    {
        return Err(invalid(
            "invalid_preset_namespace",
            "预设身份或版本不属于当前可写命名空间",
        ));
    }
    Ok(())
}
fn unique<T: PartialEq>(values: &[T]) -> bool {
    !values
        .iter()
        .enumerate()
        .any(|(i, v)| values[i + 1..].contains(v))
}
pub fn validate_protocol(protocol: &ProtocolDescriptor) -> AppResult<()> {
    validate_protocol_key(&protocol.key)?;
    validate_catalog_label(&protocol.label)?;
    if protocol.auth_kinds.is_empty()
        || protocol.allowed_purposes.is_empty()
        || !unique(&protocol.allowed_purposes)
        || !unique(&protocol.auth_kinds)
    {
        return Err(invalid(
            "invalid_protocol_descriptor",
            "协议认证和用途描述不完整或重复",
        ));
    }
    if protocol.family == CatalogFamily::Translation
        && protocol
            .allowed_purposes
            .iter()
            .any(|p| *p != CatalogPurpose::Translation)
    {
        return Err(invalid(
            "invalid_protocol_purpose",
            "传统翻译协议不能声明学习用途",
        ));
    }
    let mut seen = BTreeSet::new();
    for capability in &protocol.capabilities {
        if !seen.insert(capability.purpose)
            || (capability.support.state == CapabilityState::Supported
                && !protocol.allowed_purposes.contains(&capability.purpose))
        {
            return Err(invalid(
                "invalid_protocol_capability",
                "协议能力重复或超出已声明的硬上限",
            ));
        }
        if protocol.implementation.state != CapabilityState::Supported
            && capability.support.state == CapabilityState::Supported
        {
            return Err(invalid(
                "unimplemented_protocol_capability",
                "未实现协议不得提升实际能力为支持",
            ));
        }
    }
    Ok(())
}
pub fn validate_profile(
    profile: &CompatibilityProfileDescriptor,
    protocol: &ProtocolDescriptor,
) -> AppResult<()> {
    validate_profile_key(&profile.key)?;
    if profile.protocol != protocol.key
        || profile.allowed_purposes.is_empty()
        || !unique(&profile.allowed_purposes)
        || profile
            .allowed_purposes
            .iter()
            .any(|p| !protocol.allowed_purposes.contains(p))
        || profile.auth_kinds.is_empty()
        || !unique(&profile.auth_kinds)
        || profile
            .auth_kinds
            .iter()
            .any(|a| !protocol.auth_kinds.contains(a))
    {
        return Err(invalid(
            "profile_protocol_mismatch",
            "兼容规则的精确协议版本、认证或用途超出协议上限",
        ));
    }
    let mut seen = BTreeSet::new();
    for id in &profile.option_field_ids {
        validate_catalog_id(id)?;
        if !seen.insert(id) || forbidden_import_field(id) {
            return Err(invalid(
                "invalid_profile_option",
                "兼容规则选项重复或包含受保护字段",
            ));
        }
    }
    Ok(())
}
fn valid_scalar(value: &CatalogScalar) -> bool {
    match value {
        CatalogScalar::Text(s) => s.len() <= 16384 && !s.contains('\0'),
        CatalogScalar::Number(n) => n.is_finite(),
        CatalogScalar::Boolean(_) => true,
    }
}
pub fn validate_condition(condition: &ServiceFieldCondition) -> AppResult<()> {
    for clause in condition.all.iter().chain(&condition.any) {
        validate_catalog_id(&clause.field)?;
        if clause.values.is_empty()
            || clause.values.len() > 64
            || clause.values.iter().any(|v| !valid_scalar(v))
            || (clause.operator == ServiceConditionOperator::Eq && clause.values.len() != 1)
        {
            return Err(invalid(
                "invalid_field_condition",
                "字段条件必须有明确的有界值及有效比较操作",
            ));
        }
    }
    Ok(())
}
fn clause_state(
    clause: &ServiceConditionClause,
    values: &BTreeMap<String, CatalogScalar>,
) -> CapabilityState {
    match values.get(&clause.field) {
        None => CapabilityState::Unknown,
        Some(value) if !valid_scalar(value) => CapabilityState::Unknown,
        Some(value) if clause.values.contains(value) => CapabilityState::Supported,
        Some(_) => CapabilityState::Unsupported,
    }
}
/// Missing observations remain Unknown; they do not become false or a selected first choice.
pub fn evaluate_condition(
    condition: &ServiceFieldCondition,
    values: &BTreeMap<String, CatalogScalar>,
) -> CapabilityState {
    let all: Vec<_> = condition
        .all
        .iter()
        .map(|c| clause_state(c, values))
        .collect();
    let any: Vec<_> = condition
        .any
        .iter()
        .map(|c| clause_state(c, values))
        .collect();
    if all.contains(&CapabilityState::Unsupported)
        || (!any.is_empty() && any.iter().all(|s| *s == CapabilityState::Unsupported))
    {
        return CapabilityState::Unsupported;
    }
    if all.contains(&CapabilityState::Unknown)
        || (!any.is_empty() && !any.contains(&CapabilityState::Supported))
    {
        return CapabilityState::Unknown;
    }
    CapabilityState::Supported
}
pub fn resolve_field_default<'a>(
    field: &'a ServiceFieldDescriptor,
    values: &BTreeMap<String, CatalogScalar>,
) -> Option<&'a CatalogScalar> {
    if let Some(by) = &field.default_by {
        let selected = values.get(&by.field)?;
        return by
            .values
            .iter()
            .find(|v| &v.selector_value == selected)
            .map(|v| &v.value);
    }
    field.default_value.as_ref()
}
fn scalar_matches_field(field: &ServiceFieldDescriptor, value: &CatalogScalar) -> bool {
    match field.field_type {
        ServiceFieldType::Number => matches!(value, CatalogScalar::Number(_)),
        ServiceFieldType::Boolean => matches!(value, CatalogScalar::Boolean(_)),
        ServiceFieldType::Select => field.choices.iter().any(|choice| &choice.value == value),
        ServiceFieldType::Secret | ServiceFieldType::Action => false,
        _ => matches!(value, CatalogScalar::Text(_)),
    }
}
pub fn validate_fields(fields: &[ServiceFieldDescriptor]) -> AppResult<()> {
    let mut ids = BTreeSet::new();
    for field in fields {
        validate_catalog_id(&field.id)?;
        validate_catalog_label(&field.label)?;
        if !ids.insert(field.id.as_str()) {
            return Err(invalid("duplicate_catalog_field", "同一服务重复定义字段"));
        }
        validate_condition(&field.condition)?;
        if field.field_type == ServiceFieldType::Secret
            && (!field.masked || field.default_value.is_some() || field.default_by.is_some())
        {
            return Err(invalid(
                "secret_catalog_default",
                "秘密字段必须遮罩且不得包含默认秘密",
            ));
        }
        for (i, choice) in field.choices.iter().enumerate() {
            validate_catalog_label(&choice.label)?;
            if field.choices[i + 1..]
                .iter()
                .any(|other| other.value == choice.value)
            {
                return Err(invalid("duplicate_field_choice", "字段选项身份重复"));
            }
        }
        if field
            .default_value
            .as_ref()
            .is_some_and(|v| !valid_scalar(v) || !scalar_matches_field(field, v))
            || field.choices.iter().any(|v| !valid_scalar(&v.value))
        {
            return Err(invalid("invalid_catalog_value", "字段默认值或选项无效"));
        }
        if let Some(by) = &field.default_by {
            validate_catalog_id(&by.field)?;
            if by.values.is_empty()
                || by.values.iter().any(|v| {
                    !valid_scalar(&v.value)
                        || !valid_scalar(&v.selector_value)
                        || !scalar_matches_field(field, &v.value)
                })
            {
                return Err(invalid(
                    "invalid_conditional_default",
                    "条件默认值缺少明确的有效选择",
                ));
            }
            if by.values.iter().enumerate().any(|(i, v)| {
                by.values[i + 1..]
                    .iter()
                    .any(|other| other.selector_value == v.selector_value)
            }) {
                return Err(invalid(
                    "duplicate_conditional_default",
                    "条件默认值的选择身份重复",
                ));
            }
        }
    }
    for field in fields {
        for name in field
            .condition
            .all
            .iter()
            .chain(&field.condition.any)
            .map(|c| c.field.as_str())
            .chain(field.default_by.iter().map(|v| v.field.as_str()))
        {
            if !ids.contains(name) {
                return Err(invalid(
                    "unknown_condition_field",
                    "条件引用了当前服务中不存在的字段",
                ));
            }
        }
    }
    Ok(())
}
pub fn forbidden_import_field(id: &str) -> bool {
    matches!(
        id,
        "api_key"
            | "app_secret"
            | "service_account_json"
            | "credential_ref"
            | "vault_ref"
            | "credential_pool_id"
            | "headers"
            | "authorization"
            | "script"
            | "wire_mapping"
            | "protocol"
            | "model"
            | "endpoint"
            | "auth_mode"
    )
}
/// A preset import is data, never a capability declaration or an authorization transfer.
pub fn validate_user_preset(
    input: &UserProviderPresetInput,
    protocol: &ProtocolDescriptor,
    profile: &CompatibilityProfileDescriptor,
    fields: &[ServiceFieldDescriptor],
) -> AppResult<()> {
    validate_preset_key(&input.key, CatalogNamespace::User)?;
    validate_catalog_id(&input.provider_id)?;
    validate_catalog_label(&input.display_name)?;
    validate_catalog_label(&input.provider_label)?;
    validate_binding(&input.binding)?;
    validate_profile(profile, protocol)?;
    if input.schema_version != 1
        || input.binding.protocol != protocol.key
        || input.binding.profile != profile.key
        || !profile.auth_kinds.contains(&input.auth_kind)
    {
        return Err(invalid(
            "invalid_user_preset_binding",
            "用户预设格式、精确版本或认证不兼容",
        ));
    }
    if input.options.len() > 64 {
        return Err(invalid("preset_options_too_large", "用户预设包含过多选项"));
    }
    for (key, value) in &input.options {
        let field = fields
            .iter()
            .find(|f| &f.id == key)
            .ok_or_else(|| invalid("unknown_preset_option", "用户预设含有未登记的选项"))?;
        if forbidden_import_field(key)
            || !profile.option_field_ids.contains(key)
            || !valid_scalar(value)
            || matches!(
                field.field_type,
                ServiceFieldType::Secret | ServiceFieldType::Status | ServiceFieldType::Action
            )
            || matches!(
                field.classification,
                ServiceFieldClassification::Readonly | ServiceFieldClassification::NotApplicable
            )
        {
            return Err(invalid(
                "protected_preset_option",
                "用户预设选项不可包含秘密、执行指令或受保护字段",
            ));
        }
        let type_valid = scalar_matches_field(field, value);
        if !type_valid {
            return Err(invalid(
                "invalid_preset_option_type",
                "用户预设字段类型或选项不符合已注册规则",
            ));
        }
    }
    Ok(())
}

/// The binding purpose is an explicit typed value; unknown strings never mean translation.
pub fn parse_purpose(purpose: &str) -> AppResult<CatalogPurpose> {
    match purpose {
        "translation" => Ok(CatalogPurpose::Translation),
        "grammar" => Ok(CatalogPurpose::Grammar),
        "context_word" => Ok(CatalogPurpose::ContextWord),
        _ => Err(invalid("unsupported_purpose", "未知服务用途")),
    }
}
/// Storage material and wire header scheme are different identities.
pub fn credential_kind_for_auth_mode(
    mode: &str,
) -> AppResult<Option<super::credentials::CredentialMaterialKind>> {
    use super::credentials::CredentialMaterialKind as K;
    match mode {
        "none" => Ok(None),
        "api_key" => Ok(Some(K::ApiKey)),
        "baidu_app_secret" => Ok(Some(K::BaiduAppSecret)),
        "youdao_app_secret" => Ok(Some(K::YoudaoAppSecret)),
        "google_service_account" => Ok(Some(K::GoogleServiceAccount)),
        _ => Err(invalid("unsupported_auth_kind", "连接认证材料类型尚不支持")),
    }
}
