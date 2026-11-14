//! Built-in compatibility ceilings and typed field presentations. Runtime never reads design files.
use app_core::{contracts::*, domain::model_config::catalog::forbidden_import_field};

struct FieldBuilder(ServiceFieldDescriptor);
fn field(
    id: &str,
    label: &str,
    field_type: ServiceFieldType,
    classification: ServiceFieldClassification,
    sources: &[&str],
) -> FieldBuilder {
    FieldBuilder(ServiceFieldDescriptor {
        id: id.into(),
        label: label.into(),
        field_type,
        classification,
        group: None,
        condition: ServiceFieldCondition::default(),
        choices: vec![],
        default_value: None,
        default_by: None,
        default_evidence: None,
        inherit_from: None,
        required_when_visible: false,
        masked: false,
        observed_only: false,
        help: None,
        source_refs: sources.iter().map(|s| (*s).into()).collect(),
    })
}
impl FieldBuilder {
    fn condition(mut self, value: ServiceFieldCondition) -> Self {
        self.0.condition = value;
        self
    }
    fn choices(mut self, value: Vec<ServiceFieldChoice>) -> Self {
        self.0.choices = value;
        self
    }
    fn default_value(mut self, value: CatalogScalar) -> Self {
        self.0.default_value = Some(value);
        self
    }
    fn default_by(mut self, value: ServiceFieldDefaultBy) -> Self {
        self.0.default_by = Some(value);
        self
    }
    fn default_evidence(mut self, value: CatalogDefaultEvidence) -> Self {
        self.0.default_evidence = Some(value);
        self
    }
    fn group(mut self, value: ServiceFieldGroup) -> Self {
        self.0.group = Some(value);
        self
    }
    fn inherit_from(mut self, value: ServiceFieldInheritance) -> Self {
        self.0.inherit_from = Some(value);
        self
    }
    fn required_when_visible(mut self) -> Self {
        self.0.required_when_visible = true;
        self
    }
    fn masked(mut self) -> Self {
        self.0.masked = true;
        self
    }
    fn observed_only(mut self) -> Self {
        self.0.observed_only = true;
        self
    }
    fn help(mut self, value: &str) -> Self {
        self.0.help = Some(value.into());
        self
    }
    fn finish(self) -> ServiceFieldDescriptor {
        self.0
    }
}
fn replace(fields: &mut Vec<ServiceFieldDescriptor>, field: ServiceFieldDescriptor) {
    if let Some(old) = fields.iter_mut().find(|f| f.id == field.id) {
        *old = field;
    } else {
        fields.push(field);
    }
}
fn order(fields: Vec<ServiceFieldDescriptor>, ids: &[&str]) -> Vec<ServiceFieldDescriptor> {
    ids.iter()
        .filter_map(|id| fields.iter().find(|f| f.id == *id).cloned())
        .collect()
}
fn fields_builtin_deepseek() -> Vec<ServiceFieldDescriptor> {
    vec![
field("name", "给这项服务起个名称", ServiceFieldType::Text, ServiceFieldClassification::Advanced, &["prd"]).finish(),
field("purpose", "用来做什么", ServiceFieldType::Select, ServiceFieldClassification::Required, &["registry"])
.choices(vec![ServiceFieldChoice { value: CatalogScalar::Text("translation".into()), label: "翻译台词".into() }, ServiceFieldChoice { value: CatalogScalar::Text("grammar".into()), label: "句子精读".into() }, ServiceFieldChoice { value: CatalogScalar::Text("context_word".into()), label: "语境查词".into() }])
.group(ServiceFieldGroup::Guidance)
.help("由页面用途引导区选择/展示，不在必要表单重复要求。").finish(),
field("connection_mode", "服务地址", ServiceFieldType::Select, ServiceFieldClassification::Advanced, &["deepseek"])
.choices(vec![ServiceFieldChoice { value: CatalogScalar::Text("preset".into()), label: "使用所选服务地址".into() }, ServiceFieldChoice { value: CatalogScalar::Text("custom".into()), label: "使用自定义地址".into() }])
.default_value(CatalogScalar::Text("preset".into())).finish(),
field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["deepseek"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://api.deepseek.com".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish(),
field("auth_mode", "服务器验证方式", ServiceFieldType::Select, ServiceFieldClassification::Conditional, &["auth"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.choices(vec![ServiceFieldChoice { value: CatalogScalar::Text("none".into()), label: "服务器不需要密钥".into() }, ServiceFieldChoice { value: CatalogScalar::Text("api_key".into()), label: "使用API Key".into() }])
.default_value(CatalogScalar::Text("api_key".into()))
.group(ServiceFieldGroup::Necessary)
.required_when_visible().finish(),
field("api_key", "API Key", ServiceFieldType::Secret, ServiceFieldClassification::Conditional, &["deepseek"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "auth_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("api_key".into())] }], any: vec![] })
.required_when_visible()
.masked().finish(),
field("model", "选择或填写模型", ServiceFieldType::Text, ServiceFieldClassification::Required, &["deepseek"])
.help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。").finish(),
field("template", "提示内容", ServiceFieldType::Textarea, ServiceFieldClassification::Advanced, &["prd"]).finish(),
field("temperature_mode", "如何设置温度", ServiceFieldType::Select, ServiceFieldClassification::Advanced, &["prd"])
.choices(vec![ServiceFieldChoice { value: CatalogScalar::Text("inherit".into()), label: "继承上层设置".into() }, ServiceFieldChoice { value: CatalogScalar::Text("set".into()), label: "明确设置数值".into() }, ServiceFieldChoice { value: CatalogScalar::Text("omit".into()), label: "不发送，使用服务默认".into() }])
.default_value(CatalogScalar::Text("inherit".into()))
.group(ServiceFieldGroup::Advanced).finish(),
field("temperature", "温度（仅支持时）", ServiceFieldType::Number, ServiceFieldClassification::Conditional, &["prd"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "temperature_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("set".into())] }], any: vec![] })
.group(ServiceFieldGroup::Advanced)
.required_when_visible()
.help("只在明确设置时发送数值；0是有效值，与省略发送不同。").finish(),
field("max_tokens", "最多输出长度", ServiceFieldType::Number, ServiceFieldClassification::Advanced, &["prd"]).finish(),
field("source_language", "原文语言", ServiceFieldType::Text, ServiceFieldClassification::Default, &["prd"])
.inherit_from(ServiceFieldInheritance::SourceLanguage).finish(),
field("target_language", "译文 / 解释语言", ServiceFieldType::Text, ServiceFieldClassification::Default, &["prd"])
.inherit_from(ServiceFieldInheritance::TargetLanguage).finish(),
field("context_count", "允许附加多少句上下文", ServiceFieldType::Number, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("timeout", "请求超时（秒）", ServiceFieldType::Number, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("concurrency", "同时请求数量", ServiceFieldType::Number, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("retries", "失败重试次数", ServiceFieldType::Number, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("fallback", "明确配置备用服务", ServiceFieldType::Text, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("cache", "复用相同配置的翻译结果", ServiceFieldType::Boolean, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("key_pool", "为此服务管理多枚密钥", ServiceFieldType::Boolean, ServiceFieldClassification::Advanced, &["prd"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "auth_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("api_key".into())] }], any: vec![] }).finish(),
field("availability", "配置与验证状态", ServiceFieldType::Status, ServiceFieldClassification::Readonly, &["registry"]).finish(),
field("protocol_details", "服务接口详情", ServiceFieldType::Status, ServiceFieldClassification::Readonly, &["registry"])
.default_value(CatalogScalar::Text("OpenAI Chat Completions".into()))
.group(ServiceFieldGroup::Details).finish()
]
}

fn fields_builtin_qwen() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "provider_site",
            "API Key所在地域",
            ServiceFieldType::Select,
            ServiceFieldClassification::Required,
            &["qwen"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("cn-beijing".into()),
                label: "华北2（北京）".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("other".into()),
                label: "其他地域 / 业务空间专属地址".into(),
            },
        ])
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["qwen"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["qwen"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_by(ServiceFieldDefaultBy { field: "provider_site".into(), values: vec![ServiceConditionalDefault { selector_value: CatalogScalar::Text("cn-beijing".into()), value: CatalogScalar::Text("https://dashscope.aliyuncs.com/compatible-mode/v1".into()) }] })
.default_evidence(CatalogDefaultEvidence::ReviewedSiteDefaultsOnly)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["qwen"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["qwen"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "provider_site",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature_mode",
            "temperature",
            "max_tokens",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_ark() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "provider_site",
            "服务站点",
            ServiceFieldType::Select,
            ServiceFieldClassification::Required,
            &["ark"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("cn-beijing".into()),
                label: "北京数据面 API".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("other".into()),
                label: "其他站点 / 专属服务".into(),
            },
        ])
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["ark"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["ark"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_by(ServiceFieldDefaultBy { field: "provider_site".into(), values: vec![ServiceConditionalDefault { selector_value: CatalogScalar::Text("cn-beijing".into()), value: CatalogScalar::Text("https://ark.cn-beijing.volces.com/api/v3".into()) }] })
.default_evidence(CatalogDefaultEvidence::ReviewedSiteDefaultsOnly)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["ark"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "模型 / 接入点ID",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["ark"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "provider_site",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature_mode",
            "temperature",
            "max_tokens",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_gemini_compatible() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["gemini_compatible"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["gemini_compatible"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://generativelanguage.googleapis.com/v1beta/openai".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["gemini_compatible"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["gemini_compatible"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature_mode",
            "temperature",
            "max_tokens",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_openrouter() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["openrouter"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["openrouter"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://openrouter.ai/api/v1".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["openrouter"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["openrouter"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature_mode",
            "temperature",
            "max_tokens",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_ollama() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["ollama"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["ollama"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("http://localhost:11434/v1".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "auth_mode",
            "服务器是否需要密钥",
            ServiceFieldType::Select,
            ServiceFieldClassification::Default,
            &["auth"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("none".into()),
                label: "服务器不需要密钥".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("api_key".into()),
                label: "使用API Key".into(),
            },
        ])
        .default_value(CatalogScalar::Text("none".into()))
        .group(ServiceFieldGroup::Necessary)
        .required_when_visible()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["ollama"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["ollama"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature_mode",
            "temperature",
            "max_tokens",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_lmstudio() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["lmstudio"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["lmstudio"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("http://localhost:1234/v1".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "auth_mode",
            "服务器是否需要密钥",
            ServiceFieldType::Select,
            ServiceFieldClassification::Default,
            &["auth"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("none".into()),
                label: "服务器不需要密钥".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("api_key".into()),
                label: "使用API Key".into(),
            },
        ])
        .default_value(CatalogScalar::Text("none".into()))
        .group(ServiceFieldGroup::Necessary)
        .required_when_visible()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["lmstudio"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["lmstudio"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature_mode",
            "temperature",
            "max_tokens",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_openai_chat() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["openai"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["openai"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://api.openai.com/v1".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["openai"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["openai"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "organization_id",
            "指定组织（如账户要求）",
            ServiceFieldType::Text,
            ServiceFieldClassification::Advanced,
            &["openai"],
        )
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "openai_project_id",
            "指定项目（如账户要求）",
            ServiceFieldType::Text,
            ServiceFieldClassification::Advanced,
            &["openai"],
        )
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature_mode",
            "temperature",
            "max_tokens",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "organization_id",
            "openai_project_id",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_anthropic() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["anthropic"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["anthropic"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://api.anthropic.com".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "auth_mode",
            "服务器验证方式",
            ServiceFieldType::Select,
            ServiceFieldClassification::Readonly,
            &["auth"],
        )
        .choices(vec![ServiceFieldChoice {
            value: CatalogScalar::Text("api_key".into()),
            label: "API Key".into(),
        }])
        .default_value(CatalogScalar::Text("api_key".into()))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["anthropic"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["anthropic"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "workspace_id",
            "指定工作区（密钥需要时）",
            ServiceFieldType::Text,
            ServiceFieldClassification::Advanced,
            &["anthropic"],
        )
        .help("未绑定工作区的密钥需要工作区身份；已绑定密钥不重复要求普通用户输入。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "protocol_details",
            "服务接口详情",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["registry"],
        )
        .default_value(CatalogScalar::Text("Anthropic Messages".into()))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature_mode",
            "temperature",
            "max_tokens",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "workspace_id",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_azure_translator() -> Vec<ServiceFieldDescriptor> {
    vec![
field("name", "给这项服务起个名称", ServiceFieldType::Text, ServiceFieldClassification::Advanced, &["prd"]).finish(),
field("purpose", "用来做什么", ServiceFieldType::Select, ServiceFieldClassification::Required, &["registry"])
.choices(vec![ServiceFieldChoice { value: CatalogScalar::Text("translation".into()), label: "翻译台词".into() }])
.group(ServiceFieldGroup::Guidance)
.help("由页面用途引导区选择/展示，不在必要表单重复要求。").finish(),
field("resource_type", "Azure资源类型", ServiceFieldType::Select, ServiceFieldClassification::Required, &["azure"])
.choices(vec![ServiceFieldChoice { value: CatalogScalar::Text("global".into()), label: "全局Translator资源".into() }, ServiceFieldChoice { value: CatalogScalar::Text("regional".into()), label: "区域Translator资源".into() }, ServiceFieldChoice { value: CatalogScalar::Text("multi_service".into()), label: "多服务资源".into() }]).finish(),
field("connection_mode", "服务地址", ServiceFieldType::Select, ServiceFieldClassification::Advanced, &["azure"])
.choices(vec![ServiceFieldChoice { value: CatalogScalar::Text("preset".into()), label: "使用所选服务地址".into() }, ServiceFieldChoice { value: CatalogScalar::Text("custom".into()), label: "使用自定义地址".into() }])
.default_value(CatalogScalar::Text("preset".into())).finish(),
field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["azure"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://api.cognitive.microsofttranslator.com".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish(),
field("auth_mode", "服务器验证方式", ServiceFieldType::Select, ServiceFieldClassification::Readonly, &["auth"])
.choices(vec![ServiceFieldChoice { value: CatalogScalar::Text("api_key".into()), label: "API Key".into() }])
.default_value(CatalogScalar::Text("api_key".into()))
.group(ServiceFieldGroup::Details).finish(),
field("api_key", "订阅密钥", ServiceFieldType::Secret, ServiceFieldClassification::Conditional, &["azure"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "auth_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("api_key".into())] }], any: vec![] })
.required_when_visible()
.masked().finish(),
field("region", "资源所属区域", ServiceFieldType::Text, ServiceFieldClassification::Conditional, &["azure"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "resource_type".into(), operator: ServiceConditionOperator::In, values: vec![CatalogScalar::Text("regional".into()), CatalogScalar::Text("multi_service".into())] }], any: vec![] })
.required_when_visible().finish(),
field("model", "模型ID", ServiceFieldType::Text, ServiceFieldClassification::NotApplicable, &["registry"])
.group(ServiceFieldGroup::Internal).finish(),
field("template", "LLM提示内容", ServiceFieldType::Textarea, ServiceFieldClassification::NotApplicable, &["registry"])
.group(ServiceFieldGroup::Internal).finish(),
field("temperature", "LLM温度", ServiceFieldType::Number, ServiceFieldClassification::NotApplicable, &["registry"])
.group(ServiceFieldGroup::Internal)
.help("只在明确设置时发送数值；0是有效值，与省略发送不同。").finish(),
field("source_language", "原文语言", ServiceFieldType::Text, ServiceFieldClassification::Default, &["prd"])
.inherit_from(ServiceFieldInheritance::SourceLanguage).finish(),
field("target_language", "译文 / 解释语言", ServiceFieldType::Text, ServiceFieldClassification::Default, &["prd"])
.inherit_from(ServiceFieldInheritance::TargetLanguage).finish(),
field("context_count", "允许附加多少句上下文", ServiceFieldType::Number, ServiceFieldClassification::NotApplicable, &["prd"])
.group(ServiceFieldGroup::Internal)
.help("当前所选服务/profile未声明独立上下文输入，不为简化前端拼装额外待翻译原文。").finish(),
field("timeout", "请求超时（秒）", ServiceFieldType::Number, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("concurrency", "同时请求数量", ServiceFieldType::Number, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("retries", "失败重试次数", ServiceFieldType::Number, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("fallback", "明确配置备用服务", ServiceFieldType::Text, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("cache", "复用相同配置的翻译结果", ServiceFieldType::Boolean, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("key_pool", "为此服务管理多枚密钥", ServiceFieldType::Boolean, ServiceFieldClassification::Advanced, &["prd"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "auth_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("api_key".into())] }], any: vec![] }).finish(),
field("availability", "配置与验证状态", ServiceFieldType::Status, ServiceFieldClassification::Readonly, &["registry"]).finish(),
field("protocol_details", "服务接口详情", ServiceFieldType::Status, ServiceFieldClassification::Readonly, &["registry"])
.default_value(CatalogScalar::Text("Microsoft Azure Translator".into()))
.group(ServiceFieldGroup::Details).finish()
]
}

fn fields_builtin_deepl() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_azure_translator();
    replace(
        &mut fields,
        field(
            "plan",
            "DeepL API类型",
            ServiceFieldType::Select,
            ServiceFieldClassification::Required,
            &["deepl"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("free".into()),
                label: "API Free".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("pro".into()),
                label: "API Pro".into(),
            },
        ])
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["deepl"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["deepl"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_by(ServiceFieldDefaultBy { field: "plan".into(), values: vec![ServiceConditionalDefault { selector_value: CatalogScalar::Text("free".into()), value: CatalogScalar::Text("https://api-free.deepl.com".into()) }, ServiceConditionalDefault { selector_value: CatalogScalar::Text("pro".into()), value: CatalogScalar::Text("https://api.deepl.com".into()) }] })
.default_evidence(CatalogDefaultEvidence::ReviewedPlanDefaultsOnly)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["deepl"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "context_count",
            "允许附加的翻译上下文",
            ServiceFieldType::Number,
            ServiceFieldClassification::Advanced,
            &["deepl_translate"],
        )
        .group(ServiceFieldGroup::Purpose)
        .help("只通过此服务支持的context字段传递明确允许的材料。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "protocol_details",
            "服务接口详情",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["registry"],
        )
        .default_value(CatalogScalar::Text("DeepL Text API".into()))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "plan",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_google_v2() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_azure_translator();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["google_v2"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["google_v2"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://translation.googleapis.com/language/translate/v2".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["google_v2"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "protocol_details",
            "服务接口详情",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["registry"],
        )
        .default_value(CatalogScalar::Text(
            "Google Cloud Translation Basic v2".into(),
        ))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_baidu() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_azure_translator();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["baidu"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["baidu"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://fanyi-api.baidu.com/api/trans/vip/translate".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "auth_mode",
            "服务器验证方式",
            ServiceFieldType::Select,
            ServiceFieldClassification::NotApplicable,
            &["auth"],
        )
        .choices(vec![ServiceFieldChoice {
            value: CatalogScalar::Text("baidu_app_secret".into()),
            label: "由所选服务的专用认证流程处理".into(),
        }])
        .default_value(CatalogScalar::Text("baidu_app_secret".into()))
        .group(ServiceFieldGroup::Internal)
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "app_id",
            "APP ID",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["baidu"],
        )
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "app_secret",
            "应用密钥",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Required,
            &["baidu"],
        )
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "key_pool",
            "管理多套应用凭据",
            ServiceFieldType::Boolean,
            ServiceFieldClassification::Advanced,
            &["prd"],
        )
        .help("完整应用ID与secret作为一份typed凭据管理，不把半个字段当作独立Key轮询。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "protocol_details",
            "服务接口详情",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["registry"],
        )
        .default_value(CatalogScalar::Text("百度通用翻译".into()))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "app_id",
            "app_secret",
            "model",
            "template",
            "temperature",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_openai_responses() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["openai"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["openai"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://api.openai.com/v1".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "auth_mode",
            "服务器验证方式",
            ServiceFieldType::Select,
            ServiceFieldClassification::Readonly,
            &["auth"],
        )
        .choices(vec![ServiceFieldChoice {
            value: CatalogScalar::Text("api_key".into()),
            label: "API Key".into(),
        }])
        .default_value(CatalogScalar::Text("api_key".into()))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["openai"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["openai"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "organization_id",
            "指定组织（如账户要求）",
            ServiceFieldType::Text,
            ServiceFieldClassification::Advanced,
            &["openai"],
        )
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "openai_project_id",
            "指定项目（如账户要求）",
            ServiceFieldType::Text,
            ServiceFieldClassification::Advanced,
            &["openai"],
        )
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "protocol_details",
            "服务接口详情",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["registry"],
        )
        .default_value(CatalogScalar::Text("OpenAI Responses".into()))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature_mode",
            "temperature",
            "max_tokens",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "organization_id",
            "openai_project_id",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_gemini_native() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["gemini_native"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["gemini_native"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://generativelanguage.googleapis.com/v1beta".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "auth_mode",
            "服务器验证方式",
            ServiceFieldType::Select,
            ServiceFieldClassification::Readonly,
            &["auth"],
        )
        .choices(vec![ServiceFieldChoice {
            value: CatalogScalar::Text("api_key".into()),
            label: "API Key".into(),
        }])
        .default_value(CatalogScalar::Text("api_key".into()))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["gemini_native"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["gemini_native"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "protocol_details",
            "服务接口详情",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["registry"],
        )
        .default_value(CatalogScalar::Text("Gemini 原生 generateContent".into()))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature_mode",
            "temperature",
            "max_tokens",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_youdao() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_azure_translator();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["youdao"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["youdao"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://openapi.youdao.com/api".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "auth_mode",
            "服务器验证方式",
            ServiceFieldType::Select,
            ServiceFieldClassification::NotApplicable,
            &["auth"],
        )
        .choices(vec![ServiceFieldChoice {
            value: CatalogScalar::Text("youdao_app_secret".into()),
            label: "由所选服务的专用认证流程处理".into(),
        }])
        .default_value(CatalogScalar::Text("youdao_app_secret".into()))
        .group(ServiceFieldGroup::Internal)
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "app_key",
            "应用ID（appKey）",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["youdao"],
        )
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "app_secret",
            "应用密钥",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Required,
            &["youdao"],
        )
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "key_pool",
            "管理多套应用凭据",
            ServiceFieldType::Boolean,
            ServiceFieldClassification::Advanced,
            &["prd"],
        )
        .help("完整应用ID与secret作为一份typed凭据管理，不把半个字段当作独立Key轮询。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "protocol_details",
            "服务接口详情",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["registry"],
        )
        .default_value(CatalogScalar::Text("有道文本翻译".into()))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "app_key",
            "app_secret",
            "model",
            "template",
            "temperature",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_google_v3() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_azure_translator();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["google_v3"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["google_v3"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://translate.googleapis.com/v3".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "auth_mode",
            "服务器验证方式",
            ServiceFieldType::Select,
            ServiceFieldClassification::NotApplicable,
            &["auth"],
        )
        .choices(vec![ServiceFieldChoice {
            value: CatalogScalar::Text("google_oauth".into()),
            label: "由所选服务的专用认证流程处理".into(),
        }])
        .default_value(CatalogScalar::Text("google_oauth".into()))
        .group(ServiceFieldGroup::Internal)
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "project_id",
            "Google Cloud项目ID",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["google_v3"],
        )
        .finish(),
    );
    replace(&mut fields, field("location", "项目所在位置（location）", ServiceFieldType::Text, ServiceFieldClassification::Advanced, &["google_v3"])
.default_value(CatalogScalar::Text("global".into()))
.help("官方允许普通全局调用使用global。使用区域模型/术语库时明确改为其所在位置，不能跨区混用。").finish());
    replace(
        &mut fields,
        field(
            "google_auth_method",
            "如何授权",
            ServiceFieldType::Select,
            ServiceFieldClassification::Required,
            &["auth"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("oauth".into()),
                label: "登录Google授权".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("service_account".into()),
                label: "使用服务账号".into(),
            },
        ])
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "oauth_login",
            "登录并授权Google",
            ServiceFieldType::Action,
            ServiceFieldClassification::Conditional,
            &["auth"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "google_auth_method".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("oauth".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "service_account_json",
            "导入服务账号凭据",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["auth"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "google_auth_method".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("service_account".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(&mut fields, field("oauth_client_id", "OAuth客户端ID", ServiceFieldType::Text, ServiceFieldClassification::Conditional, &["auth"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "google_auth_method".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("oauth".into())] }, ServiceConditionClause { field: "oauth_client_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_evidence(CatalogDefaultEvidence::ApplicationOauthClientNotVerified)
.group(ServiceFieldGroup::Advanced)
.required_when_visible()
.help("仅使用你自己的OAuth客户端时填写。应用内置OAuth入口未准备好时显示资料/授权前置阻断，普通用户不负责补应用客户端资料。").finish());
    replace(
        &mut fields,
        field(
            "model",
            "指定翻译模型资源（可选）",
            ServiceFieldType::Text,
            ServiceFieldClassification::Advanced,
            &["google_v3"],
        )
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "key_pool",
            "为此服务管理多枚密钥",
            ServiceFieldType::Boolean,
            ServiceFieldClassification::NotApplicable,
            &["prd"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .group(ServiceFieldGroup::Internal)
        .help("OAuth/服务账号授权不是普通API Key池，不在此冒用Key轮询。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "protocol_details",
            "服务接口详情",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["registry"],
        )
        .default_value(CatalogScalar::Text(
            "Google Cloud Translation Advanced v3".into(),
        ))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "oauth_client_mode",
            "Google登录客户端",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["auth"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "google_auth_method".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("oauth".into())],
            }],
            any: vec![],
        })
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("application".into()),
                label: "使用应用提供的登录入口".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用我自己的OAuth客户端".into(),
            },
        ])
        .default_value(CatalogScalar::Text("application".into()))
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "application_oauth_status",
            "Google登录入口准备状态",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["auth"],
        )
        .condition(ServiceFieldCondition {
            all: vec![
                ServiceConditionClause {
                    field: "google_auth_method".into(),
                    operator: ServiceConditionOperator::Eq,
                    values: vec![CatalogScalar::Text("oauth".into())],
                },
                ServiceConditionClause {
                    field: "oauth_client_mode".into(),
                    operator: ServiceConditionOperator::Eq,
                    values: vec![CatalogScalar::Text("application".into())],
                },
            ],
            any: vec![],
        })
        .default_value(CatalogScalar::Text("unverified_application_setup".into()))
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "project_id",
            "location",
            "google_auth_method",
            "oauth_login",
            "service_account_json",
            "oauth_client_id",
            "model",
            "template",
            "temperature",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
            "oauth_client_mode",
            "application_oauth_status",
        ],
    )
}

fn fields_builtin_ollama_native() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["ollama"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["ollama"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("http://localhost:11434/api".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "auth_mode",
            "服务器是否需要密钥",
            ServiceFieldType::Select,
            ServiceFieldClassification::Default,
            &["auth"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("none".into()),
                label: "服务器不需要密钥".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("api_key".into()),
                label: "使用API Key".into(),
            },
        ])
        .default_value(CatalogScalar::Text("none".into()))
        .group(ServiceFieldGroup::Necessary)
        .required_when_visible()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["ollama"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["ollama"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "protocol_details",
            "服务接口详情",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["registry"],
        )
        .default_value(CatalogScalar::Text("Ollama 原生 /api/chat".into()))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature_mode",
            "temperature",
            "max_tokens",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_lmstudio_native() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["lmstudio_native"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["lmstudio_native"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("http://localhost:1234/api/v1".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "auth_mode",
            "服务器是否需要密钥",
            ServiceFieldType::Select,
            ServiceFieldClassification::Default,
            &["auth"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("none".into()),
                label: "服务器不需要密钥".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("api_key".into()),
                label: "使用API Key".into(),
            },
        ])
        .default_value(CatalogScalar::Text("none".into()))
        .group(ServiceFieldGroup::Necessary)
        .required_when_visible()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["lmstudio_native"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["lmstudio_native"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "protocol_details",
            "服务接口详情",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["registry"],
        )
        .default_value(CatalogScalar::Text("LM Studio 原生 /api/v1/chat".into()))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature_mode",
            "temperature",
            "max_tokens",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_tencent_tokenhub_chat() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_deepseek();
    replace(
        &mut fields,
        field(
            "purpose",
            "用来做什么",
            ServiceFieldType::Select,
            ServiceFieldClassification::Required,
            &["registry"],
        )
        .choices(vec![ServiceFieldChoice {
            value: CatalogScalar::Text("translation".into()),
            label: "翻译台词".into(),
        }])
        .group(ServiceFieldGroup::Guidance)
        .help("由页面用途引导区选择/展示，不在必要表单重复要求。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["tokenhub"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["tokenhub"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://tokenhub.tencentmaas.com/v1".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["tokenhub"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["tokenhub"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "template",
            "翻译提示内容",
            ServiceFieldType::Textarea,
            ServiceFieldClassification::Advanced,
            &["prd"],
        )
        .help("仅所选翻译模型/profile支持时可编辑，不赋予精读或查词能力。")
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_builtin_tencent_tokenhub_translation() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_builtin_azure_translator();
    replace(
        &mut fields,
        field(
            "connection_mode",
            "服务地址",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["tokenhub"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("preset".into()),
                label: "使用所选服务地址".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("custom".into()),
                label: "使用自定义地址".into(),
            },
        ])
        .default_value(CatalogScalar::Text("preset".into()))
        .finish(),
    );
    replace(&mut fields, field("endpoint", "服务地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["tokenhub"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "connection_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("custom".into())] }], any: vec![] })
.default_value(CatalogScalar::Text("https://tokenhub.tencentmaas.com/v1/api/translations".into()))
.default_evidence(CatalogDefaultEvidence::ReviewedOfficialSource)
.required_when_visible()
.help("所选内置地址由预设带入；只有你明确使用自定义服务器时才填写。预设资料未完成会单独阻断，不要求你代填。").finish());
    replace(
        &mut fields,
        field(
            "api_key",
            "API Key",
            ServiceFieldType::Secret,
            ServiceFieldClassification::Conditional,
            &["tokenhub"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "auth_mode".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("api_key".into())],
            }],
            any: vec![],
        })
        .required_when_visible()
        .masked()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "model",
            "选择或填写模型",
            ServiceFieldType::Text,
            ServiceFieldClassification::Required,
            &["tokenhub"],
        )
        .help("使用此账户/服务器实际可用的模型；文档样例不作为默认选择。")
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "context_count",
            "允许附加多少句上下文",
            ServiceFieldType::Number,
            ServiceFieldClassification::Advanced,
            &["prd"],
        )
        .group(ServiceFieldGroup::Purpose)
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "protocol_details",
            "服务接口详情",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["registry"],
        )
        .default_value(CatalogScalar::Text(
            "腾讯 TokenHub 专用翻译 /api/translations".into(),
        ))
        .group(ServiceFieldGroup::Details)
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "connection_mode",
            "endpoint",
            "auth_mode",
            "api_key",
            "model",
            "template",
            "temperature",
            "source_language",
            "target_language",
            "context_count",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "key_pool",
            "availability",
            "protocol_details",
        ],
    )
}

fn fields_apple_translation() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_jbeijing();
    replace(
        &mut fields,
        field(
            "system_status",
            "系统翻译是否可用",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["offline"],
        )
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "language_status",
            "语言对与语言包状态",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["offline"],
        )
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "prepare_language",
            "准备此语言对",
            ServiceFieldType::Action,
            ServiceFieldClassification::Conditional,
            &["offline"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "language_status".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Text("downloadable".into())],
            }],
            any: vec![],
        })
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "source_language",
            "target_language",
            "system_status",
            "language_status",
            "prepare_language",
            "check_local_conditions",
            "sample_test",
            "resource_status",
            "model",
            "api_key",
            "endpoint",
            "template",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
        ],
    )
}

fn fields_argos_translate() -> Vec<ServiceFieldDescriptor> {
    let mut fields = fields_jbeijing();
    replace(
        &mut fields,
        field(
            "installation_path",
            "语言包 / 资源位置",
            ServiceFieldType::Path,
            ServiceFieldClassification::Required,
            &["offline"],
        )
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "language_package",
            "选择语言包（如未由资源目录确定）",
            ServiceFieldType::Path,
            ServiceFieldClassification::Conditional,
            &["offline"],
        )
        .condition(ServiceFieldCondition {
            all: vec![ServiceConditionClause {
                field: "package_detected".into(),
                operator: ServiceConditionOperator::Eq,
                values: vec![CatalogScalar::Boolean(false)],
            }],
            any: vec![],
        })
        .required_when_visible()
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "route_policy",
            "翻译路径",
            ServiceFieldType::Select,
            ServiceFieldClassification::Advanced,
            &["offline"],
        )
        .choices(vec![
            ServiceFieldChoice {
                value: CatalogScalar::Text("direct_only".into()),
                label: "只使用直达语言对".into(),
            },
            ServiceFieldChoice {
                value: CatalogScalar::Text("pivot".into()),
                label: "明确允许中转".into(),
            },
        ])
        .default_value(CatalogScalar::Text("direct_only".into()))
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "runtime_status",
            "运行路线是否就绪",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["offline"],
        )
        .finish(),
    );
    replace(
        &mut fields,
        field(
            "package_detected",
            "是否已识别语言包",
            ServiceFieldType::Status,
            ServiceFieldClassification::Readonly,
            &["offline"],
        )
        .observed_only()
        .help("来自本机条件检查；未观察为unknown，不当作false或已安装。")
        .finish(),
    );
    order(
        fields,
        &[
            "name",
            "purpose",
            "source_language",
            "target_language",
            "installation_path",
            "language_package",
            "route_policy",
            "runtime_status",
            "check_local_conditions",
            "sample_test",
            "resource_status",
            "model",
            "api_key",
            "endpoint",
            "template",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
            "package_detected",
        ],
    )
}

fn fields_jbeijing() -> Vec<ServiceFieldDescriptor> {
    vec![
field("name", "配置名称", ServiceFieldType::Text, ServiceFieldClassification::Advanced, &["prd"]).finish(),
field("purpose", "用途", ServiceFieldType::Status, ServiceFieldClassification::Readonly, &["registry"])
.default_value(CatalogScalar::Text("translation".into()))
.group(ServiceFieldGroup::Guidance)
.help("由页面用途引导区选择/展示，不在必要表单重复要求。").finish(),
field("source_language", "原文语言", ServiceFieldType::Select, ServiceFieldClassification::Required, &["offline"])
.choices(vec![ServiceFieldChoice { value: CatalogScalar::Text("ja".into()), label: "日语".into() }, ServiceFieldChoice { value: CatalogScalar::Text("zh-Hans".into()), label: "简体中文".into() }, ServiceFieldChoice { value: CatalogScalar::Text("zh-Hant".into()), label: "繁體中文".into() }, ServiceFieldChoice { value: CatalogScalar::Text("en".into()), label: "English".into() }])
.help("这里只选择所需语言方向；选项存在不证明当前引擎/资源支持，仍按实际检查阻断未知或不支持。").finish(),
field("target_language", "译文语言", ServiceFieldType::Select, ServiceFieldClassification::Required, &["offline"])
.choices(vec![ServiceFieldChoice { value: CatalogScalar::Text("ja".into()), label: "日语".into() }, ServiceFieldChoice { value: CatalogScalar::Text("zh-Hans".into()), label: "简体中文".into() }, ServiceFieldChoice { value: CatalogScalar::Text("zh-Hant".into()), label: "繁體中文".into() }, ServiceFieldChoice { value: CatalogScalar::Text("en".into()), label: "English".into() }])
.help("这里只选择所需语言方向；选项存在不证明当前引擎/资源支持，仍按实际检查阻断未知或不支持。").finish(),
field("installation_path", "选择已安装的软件目录", ServiceFieldType::Path, ServiceFieldClassification::Required, &["offline"]).finish(),
field("runtime", "软件运行在哪里", ServiceFieldType::Select, ServiceFieldClassification::Required, &["offline"])
.choices(vec![ServiceFieldChoice { value: CatalogScalar::Text("windows".into()), label: "Windows".into() }, ServiceFieldChoice { value: CatalogScalar::Text("wine".into()), label: "CrossOver / Wine容器".into() }]).finish(),
field("bottle", "CrossOver / Wine容器", ServiceFieldType::Text, ServiceFieldClassification::Conditional, &["offline"])
.condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "runtime".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("wine".into())] }], any: vec![] })
.required_when_visible().finish(),
field("engine_architecture", "软件与运行器位数", ServiceFieldType::Status, ServiceFieldClassification::Readonly, &["offline"]).finish(),
field("resource_version", "检测到的引擎 / 数据版本", ServiceFieldType::Status, ServiceFieldClassification::Readonly, &["offline"]).finish(),
field("check_local_conditions", "检查本机条件", ServiceFieldType::Action, ServiceFieldClassification::Readonly, &["offline"]).finish(),
field("sample_test", "用样句测试翻译", ServiceFieldType::Action, ServiceFieldClassification::Readonly, &["offline"]).finish(),
field("resource_status", "资源与语言对状态", ServiceFieldType::Status, ServiceFieldClassification::Readonly, &["offline"]).finish(),
field("model", "通用大模型ID", ServiceFieldType::Text, ServiceFieldClassification::NotApplicable, &["offline"])
.group(ServiceFieldGroup::Internal).finish(),
field("api_key", "API Key", ServiceFieldType::Secret, ServiceFieldClassification::NotApplicable, &["offline"])
.group(ServiceFieldGroup::Internal)
.masked().finish(),
field("endpoint", "HTTP服务地址", ServiceFieldType::Url, ServiceFieldClassification::NotApplicable, &["offline"])
.group(ServiceFieldGroup::Internal).finish(),
field("template", "LLM提示内容", ServiceFieldType::Textarea, ServiceFieldClassification::NotApplicable, &["offline"])
.group(ServiceFieldGroup::Internal).finish(),
field("timeout", "请求超时（秒）", ServiceFieldType::Number, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("concurrency", "同时请求数量", ServiceFieldType::Number, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("retries", "失败重试次数", ServiceFieldType::Number, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("fallback", "明确配置备用服务", ServiceFieldType::Text, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish(),
field("cache", "复用相同配置的翻译结果", ServiceFieldType::Boolean, ServiceFieldClassification::Advanced, &["prd"])
.group(ServiceFieldGroup::Purpose).finish()
]
}

fn fields_kingsoft_fastait() -> Vec<ServiceFieldDescriptor> {
    let fields = fields_jbeijing();
    order(
        fields,
        &[
            "name",
            "purpose",
            "source_language",
            "target_language",
            "installation_path",
            "runtime",
            "bottle",
            "engine_architecture",
            "resource_version",
            "check_local_conditions",
            "sample_test",
            "resource_status",
            "model",
            "api_key",
            "endpoint",
            "template",
            "timeout",
            "concurrency",
            "retries",
            "fallback",
            "cache",
        ],
    )
}

pub(super) fn fields(id: &str) -> Vec<ServiceFieldDescriptor> {
    let mut fields = match id {
        "builtin.deepseek" => fields_builtin_deepseek(),
        "builtin.qwen" => fields_builtin_qwen(),
        "builtin.ark" => fields_builtin_ark(),
        "builtin.gemini-compatible" => fields_builtin_gemini_compatible(),
        "builtin.openrouter" => fields_builtin_openrouter(),
        "builtin.ollama" => fields_builtin_ollama(),
        "builtin.lmstudio" => fields_builtin_lmstudio(),
        "builtin.openai-chat" => fields_builtin_openai_chat(),
        "builtin.anthropic" => fields_builtin_anthropic(),
        "builtin.azure-translator" => fields_builtin_azure_translator(),
        "builtin.deepl" => fields_builtin_deepl(),
        "builtin.google-v2" => fields_builtin_google_v2(),
        "builtin.baidu" => fields_builtin_baidu(),
        "builtin.openai-responses" => fields_builtin_openai_responses(),
        "builtin.gemini-native" => fields_builtin_gemini_native(),
        "builtin.youdao" => fields_builtin_youdao(),
        "builtin.google-v3" => fields_builtin_google_v3(),
        "builtin.ollama-native" => fields_builtin_ollama_native(),
        "builtin.lmstudio-native" => fields_builtin_lmstudio_native(),
        "builtin.tencent-tokenhub-chat" => fields_builtin_tencent_tokenhub_chat(),
        "builtin.tencent-tokenhub-translation" => fields_builtin_tencent_tokenhub_translation(),
        "apple_translation" => fields_apple_translation(),
        "argos_translate" => fields_argos_translate(),
        "jbeijing" => fields_jbeijing(),
        "kingsoft_fastait" => fields_kingsoft_fastait(),
        _ => vec![],
    };
    // The approved guided network section is shared by every HTTP service.
    // Offline workers never acquire a network policy through this directory.
    if id.starts_with("builtin.") && !fields.is_empty() {
        fields.push(field("proxy_mode", "网络连接方式", ServiceFieldType::Select, ServiceFieldClassification::Advanced, &["prd"])
            .choices(vec![
                ServiceFieldChoice { value: CatalogScalar::Text("inherit".into()), label: "继承系统代理".into() },
                ServiceFieldChoice { value: CatalogScalar::Text("disabled".into()), label: "直接连接".into() },
                ServiceFieldChoice { value: CatalogScalar::Text("override".into()), label: "此服务独立代理".into() },
            ]).default_value(CatalogScalar::Text("inherit".into()))
            .group(ServiceFieldGroup::Advanced)
            .help("仅用于当前服务连接；登录、测试和翻译共用此网络策略。")
            .finish());
        fields.push(field("proxy_url", "独立代理地址", ServiceFieldType::Url, ServiceFieldClassification::Conditional, &["prd"])
            .condition(ServiceFieldCondition { all: vec![ServiceConditionClause { field: "proxy_mode".into(), operator: ServiceConditionOperator::Eq, values: vec![CatalogScalar::Text("override".into())] }], any: vec![] })
            .group(ServiceFieldGroup::Advanced).required_when_visible()
            .help("填写完整HTTP(S)代理地址；不能包含账号、密码、路径、查询参数或片段。")
            .finish());
    }
    fields
}

pub fn builtin_profiles() -> Vec<CompatibilityProfileDescriptor> {
    super::presets::builtin_presets()
        .into_iter()
        .map(|p| {
            let auth_kinds = super::register::protocol_auth_kinds(&p.binding.protocol.protocol_id);
            CompatibilityProfileDescriptor {
                key: p.binding.profile,
                protocol: p.binding.protocol,
                allowed_purposes: p.allowed_purposes,
                auth_kinds,
                option_field_ids: p
                    .fields
                    .iter()
                    .filter(|f| {
                        !forbidden_import_field(&f.id)
                            && !matches!(
                                f.field_type,
                                ServiceFieldType::Secret
                                    | ServiceFieldType::Status
                                    | ServiceFieldType::Action
                            )
                            && !matches!(
                                f.classification,
                                ServiceFieldClassification::Readonly
                                    | ServiceFieldClassification::NotApplicable
                            )
                    })
                    .map(|f| f.id.clone())
                    .collect(),
                source_refs: p.source_refs,
            }
        })
        .collect()
}
