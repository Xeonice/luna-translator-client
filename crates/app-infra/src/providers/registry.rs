//! Immutable exact-version registry. Registration is composition, not service activation.
use app_core::{contracts::*, domain::model_config::catalog};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct ProviderRegistryBuilder {
    protocols: BTreeMap<ProtocolKey, ProtocolDescriptor>,
    profiles: BTreeMap<CompatibilityProfileKey, CompatibilityProfileDescriptor>,
    presets: BTreeMap<ProviderPresetKey, ProviderPresetDescriptor>,
    offline_engines: BTreeMap<(String, u32), OfflineEngineDescriptor>,
    sources: BTreeMap<String, CatalogSource>,
}
#[derive(Debug, Clone)]
pub struct ProviderRegistry {
    frozen: ProviderRegistryBuilder,
}
impl app_core::ports::service_configuration::ServiceDirectory for ProviderRegistry {
    fn catalog(&self) -> ServiceCatalog { ProviderRegistry::catalog(self) }
    fn form(&self, request: &ServiceFormRequest) -> AppResult<ServiceFormProjection> { self.project_form(request) }
}
fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, "service_catalog", message)
}
fn endpoint(value: &str) -> AppResult<()> {
    if value.len() > 4096 {
        return Err(error("invalid_preset_endpoint", "预设地址超过长度上限"));
    }
    let url =
        reqwest::Url::parse(value).map_err(|_| error("invalid_preset_endpoint", "预设地址无效"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || value.chars().any(char::is_control)
    {
        return Err(error(
            "secret_in_preset_endpoint",
            "预设地址仅允许无凭据、查询参数和片段的HTTP(S)地址",
        ));
    }
    Ok(())
}
impl ProviderRegistryBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register_source(&mut self, source: CatalogSource) -> AppResult<()> {
        catalog::validate_catalog_id(&source.source_id)?;
        catalog::validate_catalog_label(&source.reference)?;
        if self.sources.contains_key(&source.source_id) {
            return Err(error("duplicate_catalog_source", "目录来源身份重复"));
        }
        self.sources.insert(source.source_id.clone(), source);
        Ok(())
    }
    pub fn register_protocol(&mut self, protocol: ProtocolDescriptor) -> AppResult<()> {
        catalog::validate_protocol(&protocol)?;
        if self.protocols.contains_key(&protocol.key) {
            return Err(error("duplicate_protocol_version", "精确协议版本重复注册"));
        }
        self.protocols.insert(protocol.key.clone(), protocol);
        Ok(())
    }
    pub fn register_profile(&mut self, profile: CompatibilityProfileDescriptor) -> AppResult<()> {
        let protocol = self
            .protocols
            .get(&profile.protocol)
            .ok_or_else(|| error("unknown_protocol_version", "兼容规则所引用的精确协议未注册"))?;
        catalog::validate_profile(&profile, protocol)?;
        if self.profiles.contains_key(&profile.key) {
            return Err(error(
                "duplicate_profile_version",
                "精确兼容规则版本重复注册",
            ));
        }
        self.profiles.insert(profile.key.clone(), profile);
        Ok(())
    }
    pub fn register_builtin_preset(&mut self, preset: ProviderPresetDescriptor) -> AppResult<()> {
        if preset.namespace != CatalogNamespace::Builtin {
            return Err(error(
                "invalid_preset_namespace",
                "内置登记入口不接受用户命名空间",
            ));
        }
        self.register_preset(preset)
    }
    fn register_preset(&mut self, preset: ProviderPresetDescriptor) -> AppResult<()> {
        catalog::validate_preset_key(&preset.key, preset.namespace)?;
        catalog::validate_catalog_id(&preset.provider_id)?;
        catalog::validate_catalog_id(&preset.brand_id)?;
        self.validate_brand(&preset.brand_id, &preset.brand_label)?;
        catalog::validate_catalog_label(&preset.label)?;
        catalog::validate_binding(&preset.binding)?;
        catalog::validate_fields(&preset.fields)?;
        let resolved = self.resolve(&preset.binding)?;
        if preset.allowed_purposes.is_empty()
            || preset
                .allowed_purposes
                .iter()
                .any(|p| !resolved.profile.allowed_purposes.contains(p))
            || !resolved
                .profile
                .auth_kinds
                .contains(&preset.default_auth_kind)
            || preset.implementation != resolved.protocol.implementation
            || preset.eligibility.state != CapabilityState::Unknown
        {
            return Err(error(
                "preset_capability_mismatch",
                "预设不得扩大协议或兼容规则能力，也不能声明服务已获运行资格",
            ));
        }
        if let Some(value) = &preset.default_endpoint {
            endpoint(value)?;
        }
        for field in &preset.fields {
            if field.field_type == ServiceFieldType::Url {
                if let Some(CatalogScalar::Text(value)) = &field.default_value {
                    endpoint(value)?;
                }
                if let Some(by) = &field.default_by {
                    for item in &by.values {
                        if let CatalogScalar::Text(value) = &item.value {
                            endpoint(value)?;
                        }
                    }
                }
            }
        }
        for blocker in &preset.material_blockers {
            catalog::validate_condition(&blocker.condition)?;
            if blocker
                .condition
                .all
                .iter()
                .chain(&blocker.condition.any)
                .any(|c| !preset.fields.iter().any(|f| f.id == c.field))
            {
                return Err(error(
                    "unknown_material_condition",
                    "预设资料阻断引用了不存在的字段",
                ));
            }
        }
        if self.presets.contains_key(&preset.key) {
            return Err(error("duplicate_preset_version", "预设不可变版本重复注册"));
        }
        self.presets.insert(preset.key.clone(), preset);
        Ok(())
    }
    /// Declarative user data can select an existing exact profile, never install code or auth headers.
    pub fn register_user_preset(&mut self, input: UserProviderPresetInput) -> AppResult<()> {
        let resolved = self.resolve(&input.binding)?;
        let template = self
            .presets
            .values()
            .find(|p| p.namespace == CatalogNamespace::Builtin && p.binding == input.binding)
            .cloned()
            .ok_or_else(|| {
                error(
                    "profile_presentation_unavailable",
                    "当前兼容规则没有已登记的必要字段目录",
                )
            })?;
        catalog::validate_user_preset(
            &input,
            &resolved.protocol,
            &resolved.profile,
            &template.fields,
        )?;
        if let Some(value) = &input.endpoint {
            endpoint(value)?;
        }
        let mut preset = template;
        preset.key = input.key;
        preset.namespace = CatalogNamespace::User;
        preset.provider_id = input.provider_id.clone();
        preset.brand_id = input.provider_id;
        preset.brand_label = input.provider_label;
        preset.label = input.display_name;
        preset.default_auth_kind = input.auth_kind;
        preset.default_endpoint = input.endpoint;
        // Imports never inherit a built-in endpoint when the import leaves it unspecified.
        for field in &mut preset.fields {
            if field.id == "endpoint" {
                field.default_value = preset.default_endpoint.clone().map(CatalogScalar::Text);
                field.default_by = None;
                field.default_evidence = None;
            } else if field.id == "connection_mode" {
                field.default_value = Some(CatalogScalar::Text("custom".into()));
            } else if field.id == "auth_mode" {
                let mode = match preset.default_auth_kind {
                    CatalogAuthKind::None => "none",
                    CatalogAuthKind::BaiduAppSecret => "baidu_app_secret",
                    CatalogAuthKind::YoudaoAppSecret => "youdao_app_secret",
                    CatalogAuthKind::GoogleOauth | CatalogAuthKind::GoogleServiceAccount => {
                        "google_oauth"
                    }
                    _ => "api_key",
                };
                field.default_value = Some(CatalogScalar::Text(mode.into()));
            } else if let Some(value) = input.options.get(&field.id) {
                field.default_value = Some(value.clone());
                field.default_by = None;
                field.default_evidence = None;
            }
        }
        preset.interface_choice_required = true;
        // Site/app-owned fixed material is not fabricated by importing a custom-server preset.
        self.register_preset(preset)
    }
    pub fn register_offline_engine(&mut self, engine: OfflineEngineDescriptor) -> AppResult<()> {
        catalog::validate_catalog_id(&engine.engine_id)?;
        catalog::validate_catalog_label(&engine.label)?;
        catalog::validate_catalog_id(&engine.brand_id)?;
        self.validate_brand(&engine.brand_id, &engine.brand_label)?;
        catalog::validate_fields(&engine.fields)?;
        if engine.contract_revision == 0
            || engine.allowed_purposes != vec![CatalogPurpose::Translation]
            || engine.model_identity != CatalogModelIdentity::NotApplicable
            || engine.eligibility.state != CapabilityState::Unknown
        {
            return Err(error(
                "invalid_offline_descriptor",
                "本机翻译描述必须保留独立身份、翻译用途与未观察资格",
            ));
        }
        for forbidden in ["model", "api_key", "endpoint"] {
            if engine.fields.iter().any(|f| {
                f.id == forbidden && f.classification != ServiceFieldClassification::NotApplicable
            }) {
                return Err(error(
                    "offline_http_field",
                    "本机引擎不能要求HTTP、密钥或通用大模型字段",
                ));
            }
        }
        let key = (engine.engine_id.clone(), engine.contract_revision);
        if self.offline_engines.contains_key(&key) {
            return Err(error(
                "duplicate_offline_version",
                "本机引擎精确版本重复注册",
            ));
        }
        self.offline_engines.insert(key, engine);
        Ok(())
    }
    fn validate_brand(&self, id: &str, label: &str) -> AppResult<()> {
        catalog::validate_catalog_label(label)?;
        let prior = self
            .presets
            .values()
            .map(|p| (&p.brand_id, &p.brand_label))
            .chain(
                self.offline_engines
                    .values()
                    .map(|e| (&e.brand_id, &e.brand_label)),
            );
        if prior
            .into_iter()
            .any(|(old_id, old_label)| old_id == id && old_label != label)
        {
            return Err(error(
                "catalog_brand_conflict",
                "同一品牌身份不能对应不同显示名称",
            ));
        }
        Ok(())
    }
    fn resolve(&self, binding: &AdapterBinding) -> AppResult<ResolvedProviderDescriptor> {
        catalog::validate_binding(binding)?;
        let protocol = self.protocols.get(&binding.protocol).ok_or_else(|| {
            error(
                "unknown_protocol_version",
                "精确协议版本未注册，禁止降级到其他协议或最新版本",
            )
        })?;
        let profile = self
            .profiles
            .get(&binding.profile)
            .ok_or_else(|| error("unknown_profile_version", "精确兼容规则版本未注册"))?;
        catalog::validate_profile(profile, protocol)?;
        Ok(ResolvedProviderDescriptor {
            protocol: protocol.clone(),
            profile: profile.clone(),
            implementation: protocol.implementation.clone(),
        })
    }
    pub fn freeze(self) -> AppResult<ProviderRegistry> {
        let references = self
            .protocols
            .values()
            .flat_map(|p| p.source_refs.iter())
            .chain(self.profiles.values().flat_map(|p| p.source_refs.iter()))
            .chain(self.presets.values().flat_map(|p| {
                p.source_refs
                    .iter()
                    .chain(p.fields.iter().flat_map(|f| f.source_refs.iter()))
                    .chain(
                        p.material_blockers
                            .iter()
                            .flat_map(|b| b.source_refs.iter()),
                    )
            }))
            .chain(self.offline_engines.values().flat_map(|p| {
                p.source_refs
                    .iter()
                    .chain(p.fields.iter().flat_map(|f| f.source_refs.iter()))
            }));
        if references
            .into_iter()
            .any(|id| !self.sources.contains_key(id))
        {
            return Err(error(
                "unknown_catalog_source",
                "目录引用的来源不存在，无法冻结",
            ));
        }
        for preset in self.presets.values() {
            let profile = self
                .profiles
                .get(&preset.binding.profile)
                .ok_or_else(|| error("unknown_profile_version", "预设的兼容规则不存在"))?;
            if profile
                .option_field_ids
                .iter()
                .any(|id| !preset.fields.iter().any(|f| &f.id == id))
            {
                return Err(error(
                    "unknown_profile_field",
                    "兼容规则选项引用了不存在的字段",
                ));
            }
        }
        Ok(ProviderRegistry { frozen: self })
    }
}
impl ProviderRegistry {
    pub fn catalog(&self) -> ServiceCatalog {
        let brands: BTreeMap<_, _> = self
            .frozen
            .presets
            .values()
            .map(|p| (p.brand_id.clone(), p.brand_label.clone()))
            .chain(
                self.frozen
                    .offline_engines
                    .values()
                    .map(|p| (p.brand_id.clone(), p.brand_label.clone())),
            )
            .collect();
        ServiceCatalog {
            brands: brands
                .into_iter()
                .map(|(brand_id, label)| CatalogBrandDescriptor { brand_id, label })
                .collect(),
            schema_version: 1,
            protocols: self.frozen.protocols.values().cloned().collect(),
            profiles: self.frozen.profiles.values().cloned().collect(),
            presets: self.frozen.presets.values().cloned().collect(),
            offline_engines: self.frozen.offline_engines.values().cloned().collect(),
            sources: self.frozen.sources.values().cloned().collect(),
        }
    }
    pub fn resolve(&self, binding: &AdapterBinding) -> AppResult<ResolvedProviderDescriptor> {
        self.frozen.resolve(binding)
    }
    pub fn preset(&self, key: &ProviderPresetKey) -> AppResult<ProviderPresetDescriptor> {
        self.frozen
            .presets
            .get(key)
            .cloned()
            .ok_or_else(|| error("unknown_preset_version", "精确预设版本未注册"))
    }
    /// Immutable presentation only; no network, Vault, configuration save or eligibility promotion.
    pub fn project_form(&self, request: &ServiceFormRequest) -> AppResult<ServiceFormProjection> {
        let (fields, blockers, implementation, authentication) = match &request.target {
            ServiceFormTarget::Http { preset } => {
                let descriptor = self.preset(preset)?;
                let resolved = self.resolve(&descriptor.binding)?;
                if !descriptor.allowed_purposes.contains(&request.purpose)
                    || !resolved.profile.allowed_purposes.contains(&request.purpose)
                    || !resolved
                        .protocol
                        .allowed_purposes
                        .contains(&request.purpose)
                {
                    return Err(error(
                        "form_purpose_not_supported",
                        "该精确服务版本不支持所选用途",
                    ));
                }
                (
                    descriptor.fields,
                    descriptor.material_blockers,
                    descriptor.implementation,
                    Some((descriptor.binding.protocol.protocol_id, descriptor.default_auth_kind)),
                )
            }
            ServiceFormTarget::Offline {
                engine_id,
                contract_revision,
            } => {
                catalog::validate_catalog_id(engine_id)?;
                let descriptor = self
                    .frozen
                    .offline_engines
                    .get(&(engine_id.clone(), *contract_revision))
                    .ok_or_else(|| {
                        error("unknown_offline_engine_version", "精确本机引擎版本未注册")
                    })?;
                if !descriptor.allowed_purposes.contains(&request.purpose) {
                    return Err(error(
                        "form_purpose_not_supported",
                        "本机翻译引擎不支持所选用途",
                    ));
                }
                (
                    descriptor.fields.clone(),
                    Vec::new(),
                    descriptor.implementation.clone(),
                    None,
                )
            }
        };
        let mut projection = app_core::domain::model_config::form::project_service_form(
            request,
            &fields,
            &blockers,
            &implementation,
        )?;
        let value = |id: &str| projection.fields.iter().find(|row| row.descriptor.id == id)
            .and_then(|row| row.resolved.value.as_ref()).and_then(|value| match value {
                CatalogScalar::Text(text) => Some(text.as_str()), _ => None,
            });
        let capture = if let Some((protocol, default_kind)) = authentication {
            catalog::service_credential_capture(&protocol, &default_kind, value("auth_mode"), value("google_auth_method"))
                .unwrap_or(ServiceCredentialCapture::Unknown)
        } else { ServiceCredentialCapture::None };
        projection.credential_capture = capture;
        Ok(projection)
    }
    pub fn with_user_preset(&self, input: UserProviderPresetInput) -> AppResult<Self> {
        let mut builder = self.frozen.clone();
        builder.register_user_preset(input)?;
        builder.freeze()
    }
}
