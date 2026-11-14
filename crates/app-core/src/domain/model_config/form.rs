//! Pure, bounded form interpretation. Registry resolves trusted exact descriptors before calling it.
use super::catalog::{
    evaluate_condition, resolve_field_default, validate_condition, validate_fields,
};
use crate::contracts::*;
use std::collections::{BTreeMap, BTreeSet};

fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, "service_form", message)
}
fn editable(field: &ServiceFieldDescriptor) -> bool {
    field.id != "purpose"
        && !field.masked
        && !field.observed_only
        && !matches!(
            field.classification,
            ServiceFieldClassification::Readonly | ServiceFieldClassification::NotApplicable
        )
        && !matches!(
            field.field_type,
            ServiceFieldType::Secret | ServiceFieldType::Action | ServiceFieldType::Status
        )
}
fn issue(field: &ServiceFieldDescriptor, value: &CatalogScalar) -> Option<ServiceFormIssue> {
    match field.field_type {
        ServiceFieldType::Number => {
            let CatalogScalar::Number(n) = value else {
                return Some(ServiceFormIssue::TypeMismatch);
            };
            let valid = match field.id.as_str() {
                "context_count" => n.fract() == 0.0 && (0.0..=50.0).contains(n),
                "timeout" | "concurrency" | "retries" => {
                    let (key, number) = if field.id == "timeout" {
                        ("timeoutMs", *n * 1000.0)
                    } else {
                        (field.id.as_str(), *n)
                    };
                    number.is_finite()
                        && number >= 0.0
                        && (number - number.round()).abs() <= number.abs() * f64::EPSILON * 2.0
                        && number <= 9_007_199_254_740_991.0
                        && super::validate_schedule(&BTreeMap::from([(
                            key.into(),
                            serde_json::json!(number.round() as u64),
                        )]))
                        .is_ok()
                }
                "max_tokens" => n.fract() == 0.0 && *n >= 1.0 && *n <= u32::MAX as f64,
                "temperature" => *n >= 0.0 && *n <= 9_007_199_254_740_991.0,
                _ => n.abs() <= 9_007_199_254_740_991.0,
            };
            (!valid).then_some(ServiceFormIssue::InvalidNumber)
        }
        ServiceFieldType::Boolean => {
            (!matches!(value, CatalogScalar::Boolean(_))).then_some(ServiceFormIssue::TypeMismatch)
        }
        ServiceFieldType::Select => (!field.choices.iter().any(|c| &c.value == value))
            .then_some(ServiceFormIssue::InvalidChoice),
        _ => {
            let CatalogScalar::Text(text) = value else {
                return Some(ServiceFormIssue::TypeMismatch);
            };
            if text.trim().is_empty() {
                return Some(ServiceFormIssue::EmptyValue);
            }
            if field.field_type == ServiceFieldType::Url {
                let host = text
                    .strip_prefix("https://")
                    .or_else(|| text.strip_prefix("http://"))
                    .and_then(|rest| rest.split('/').next());
                if host.is_none_or(str::is_empty)
                    || text.chars().any(|c| c.is_control() || c.is_whitespace())
                    || text.contains(['@', '?', '#'])
                    || text.len() > 4096
                {
                    return Some(ServiceFormIssue::InvalidUrl);
                }
            }
            None
        }
    }
}
fn purpose_value(purpose: CatalogPurpose) -> CatalogScalar {
    CatalogScalar::Text(
        match purpose {
            CatalogPurpose::Translation => "translation",
            CatalogPurpose::Grammar => "grammar",
            CatalogPurpose::ContextWord => "context_word",
        }
        .into(),
    )
}
fn base_value(
    field: &ServiceFieldDescriptor,
    values: &BTreeMap<String, CatalogScalar>,
) -> Option<CatalogScalar> {
    if field.masked
        || field.observed_only
        || field.classification == ServiceFieldClassification::NotApplicable
        || matches!(
            field.field_type,
            ServiceFieldType::Secret | ServiceFieldType::Action
        )
    {
        return None;
    }
    resolve_field_default(field, values).cloned()
}
/// This is not a save validator, credential probe, eligibility decision or runtime authority.
pub fn project_service_form(
    request: &ServiceFormRequest,
    fields: &[ServiceFieldDescriptor],
    blockers: &[ConditionalCatalogBlocker],
    implementation: &CatalogDecision,
) -> AppResult<ServiceFormProjection> {
    validate_fields(fields)?;
    if fields.iter().any(|f| {
        (f.masked || f.field_type == ServiceFieldType::Secret)
            && (f.default_value.is_some() || f.default_by.is_some() || !f.choices.is_empty())
    }) {
        return Err(error(
            "protected_form_descriptor",
            "秘密字段描述不得携带值或选项",
        ));
    }
    for blocker in blockers {
        validate_condition(&blocker.condition)?;
        if blocker
            .condition
            .all
            .iter()
            .chain(&blocker.condition.any)
            .any(|c| !fields.iter().any(|f| f.id == c.field))
        {
            return Err(error(
                "unknown_material_condition",
                "材料条件引用了未声明字段",
            ));
        }
    }
    if fields.len() > 128 || request.values.len() > 128 {
        return Err(error("excessive_form_fields", "表单字段超过边界"));
    }
    for (id, value) in &request.values {
        let field = fields
            .iter()
            .find(|f| &f.id == id)
            .ok_or_else(|| error("unknown_form_field", "输入包含当前服务未声明字段"))?;
        if !editable(field) {
            return Err(error(
                "protected_form_field",
                "用途、秘密、动作、观察或不可编辑字段不得经普通表单输入",
            ));
        }
        if match value {
            CatalogScalar::Text(s) => s.len() > 16384 || s.contains('\0'),
            CatalogScalar::Number(n) => !n.is_finite(),
            CatalogScalar::Boolean(_) => false,
        } {
            return Err(error(
                "invalid_form_scalar",
                "字段含过长文本、NUL或非有限数值",
            ));
        }
    }
    let mut context = BTreeMap::from([("purpose".into(), purpose_value(request.purpose))]);
    let mut seen = Vec::<BTreeMap<String, CatalogScalar>>::new();
    let mut cyclic = BTreeSet::new();
    for _ in 0..fields.len() * 4 + 4 {
        let mut next = BTreeMap::from([("purpose".into(), purpose_value(request.purpose))]);
        for field in fields {
            if field.id == "purpose" || cyclic.contains(&field.id) {
                continue;
            }
            let mut value = base_value(field, &context);
            if let Some(input) = request.values.get(&field.id) {
                match evaluate_condition(&field.condition, &context) {
                    CapabilityState::Supported => {
                        value = if issue(field, input).is_none() {
                            Some(input.clone())
                        } else {
                            None
                        };
                    }
                    CapabilityState::Unknown => value = None,
                    CapabilityState::Unsupported => {}
                }
            }
            if let Some(value) = value {
                next.insert(field.id.clone(), value);
            }
        }
        if next == context {
            break;
        }
        if let Some(start) = seen.iter().position(|old| *old == next) {
            for field in fields {
                let v = next.get(&field.id);
                if seen[start..].iter().any(|old| old.get(&field.id) != v)
                    || context.get(&field.id) != v
                {
                    cyclic.insert(field.id.clone());
                }
            }
            for id in &cyclic {
                next.remove(id);
            }
            seen.clear();
        } else {
            seen.push(context.clone());
        }
        context = next;
    }
    // A cycle with no effective selector can reach a fixed Unknown state without oscillating.
    // Detect it explicitly; known trusted/default/user selectors already in context are terminal evidence.
    fn unresolved_reaches(
        start: &str,
        current: &str,
        fields: &[ServiceFieldDescriptor],
        context: &BTreeMap<String, CatalogScalar>,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if !visited.insert(current.into()) {
            return false;
        }
        let Some(field) = fields.iter().find(|f| f.id == current) else {
            return false;
        };
        field
            .condition
            .all
            .iter()
            .chain(&field.condition.any)
            .map(|c| c.field.as_str())
            .chain(field.default_by.iter().map(|by| by.field.as_str()))
            .any(|next| {
                !context.contains_key(next)
                    && (next == start || unresolved_reaches(start, next, fields, context, visited))
            })
    }
    for field in fields {
        if !context.contains_key(&field.id)
            && unresolved_reaches(&field.id, &field.id, fields, &context, &mut BTreeSet::new())
        {
            cyclic.insert(field.id.clone());
        }
    }
    let rows = fields
        .iter()
        .map(|field| {
            let visibility = if field.classification == ServiceFieldClassification::NotApplicable {
                CapabilityState::Unsupported
            } else {
                evaluate_condition(&field.condition, &context)
            };
            let input = request.values.get(&field.id);
            let mut issues = Vec::new();
            if let Some(input) = input {
                if let Some(problem) = issue(field, input) {
                    issues.push(problem);
                }
                if visibility == CapabilityState::Unsupported {
                    issues.push(ServiceFormIssue::InactiveField);
                }
            }
            if visibility == CapabilityState::Unknown {
                issues.push(ServiceFormIssue::ConditionUnresolved);
            }
            if cyclic.contains(&field.id) {
                issues.push(ServiceFormIssue::DependencyCycle);
            }
            let required = visibility == CapabilityState::Supported
                && (field.classification == ServiceFieldClassification::Required
                    || field.required_when_visible);
            let value = context.get(&field.id).cloned();
            let source = if field.id == "purpose" {
                ServiceFormValueSource::Purpose
            } else if input.is_some()
                && visibility == CapabilityState::Supported
                && issues.is_empty()
            {
                ServiceFormValueSource::User
            } else if value.is_some() {
                ServiceFormValueSource::Default
            } else if field.inherit_from.is_some() && input.is_none() {
                ServiceFormValueSource::Inherited
            } else {
                ServiceFormValueSource::Unset
            };
            if required && editable(field) && value.is_none() && input.is_none() {
                issues.push(ServiceFormIssue::RequiredValueMissing);
            }
            ServiceFormRow {
                descriptor: field.clone(),
                visibility,
                resolved: ServiceFormResolvedValue {
                    value,
                    source,
                    inherit_from: field.inherit_from,
                },
                input_value: input.cloned(),
                input_allowed: editable(field),
                required,
                issues,
            }
        })
        .collect();
    Ok(ServiceFormProjection {
        target: request.target.clone(),
        purpose: request.purpose,
        credential_capture: ServiceCredentialCapture::Unknown,
        fields: rows,
        material_blockers: blockers
            .iter()
            .map(|b| ServiceFormMaterialBlocker {
                descriptor: b.clone(),
                condition_state: evaluate_condition(&b.condition, &context),
            })
            .collect(),
        implementation: implementation.clone(),
    })
}
