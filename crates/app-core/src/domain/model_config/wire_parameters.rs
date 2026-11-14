//! Protocol contract v1 generation parameters. This is a wire ceiling, not proof that a
//! particular model/profile accepts a parameter. No UI-field inference or secret access.
//! Native mappings/ranges are extracted from the existing llm wire builder; traditional
//! translation accepts no generation parameters. Chat/Anthropic use their pinned text
//! contracts, not arbitrary provider tools or newer API-version fields.
use crate::contracts::{AppError, AppResult, ProtocolKey};
use serde_json::Value;
use std::collections::BTreeMap;

fn error(code: &str) -> AppError {
    AppError::new(
        code,
        "wire_parameters",
        "生成参数的名称、类型、范围或别名不适用于精确协议",
    )
}
/// Return exact wire names after validating contract identity, values and alias collisions.
/// Infrastructure builders consume this map instead of keeping another parameter schema.
pub fn generation_wire_parameters(
    protocol: &ProtocolKey,
    parameters: &BTreeMap<String, Value>,
) -> AppResult<BTreeMap<String, Value>> {
    let id = protocol.protocol_id.as_str();
    if protocol.contract_revision != 1
        || !matches!(
            id,
            "openai_compatible"
                | "anthropic"
                | "openai_responses"
                | "gemini_generate_content"
                | "ollama_native"
                | "lmstudio_native_v1"
                | "azure_translator"
                | "deepl_v2"
                | "google_translate_v2"
                | "google_translate_v3"
                | "baidu_vip"
                | "youdao_v3"
                | "tencent_tokenhub_translation"
        )
    {
        return Err(error("unknown_protocol_version"));
    }
    if super::is_translation_protocol(id) {
        return if parameters.is_empty() {
            Ok(BTreeMap::new())
        } else {
            Err(error("unsupported_parameter"))
        };
    }
    let mut result = BTreeMap::new();
    for (key, value) in parameters {
        let mapped = match (id, key.as_str()) {
            (
                "openai_compatible",
                "temperature"
                | "top_p"
                | "max_tokens"
                | "max_completion_tokens"
                | "presence_penalty"
                | "frequency_penalty"
                | "seed"
                | "stop",
            ) => key.as_str(),
            ("anthropic", "temperature" | "top_p" | "top_k" | "max_tokens" | "stop_sequences") => {
                key.as_str()
            }
            ("openai_responses", "temperature" | "top_p") => key.as_str(),
            ("openai_responses" | "lmstudio_native_v1", "max_tokens") => "max_output_tokens",
            (
                "lmstudio_native_v1",
                "temperature" | "top_p" | "top_k" | "min_p" | "repeat_penalty"
                | "max_output_tokens" | "context_length",
            ) => key.as_str(),
            ("gemini_generate_content", "temperature") => "temperature",
            ("gemini_generate_content", "top_p") => "topP",
            ("gemini_generate_content", "top_k") => "topK",
            ("gemini_generate_content", "max_tokens") => "maxOutputTokens",
            (
                "ollama_native",
                "temperature" | "top_p" | "top_k" | "min_p" | "repeat_penalty" | "seed",
            ) => key.as_str(),
            ("ollama_native", "max_tokens") => "num_predict",
            _ => return Err(error("unsupported_parameter")),
        };
        let valid = match key.as_str() {
            "stop" => {
                value.as_str().is_some_and(valid_stop)
                    || value.as_array().is_some_and(|items| {
                        !items.is_empty()
                            && items.len() <= 4
                            && items.iter().all(|v| v.as_str().is_some_and(valid_stop))
                    })
            }
            "stop_sequences" => value
                .as_array()
                .is_some_and(|items| items.iter().all(|v| v.as_str().is_some_and(valid_stop))),
            "max_tokens" | "max_output_tokens" | "max_completion_tokens" | "context_length" => {
                value
                    .as_u64()
                    .is_some_and(|n| n > 0 && n <= i32::MAX as u64)
            }
            "top_k" => value.as_u64().is_some_and(|n| n <= i32::MAX as u64),
            "seed" if id == "openai_compatible" => value.as_i64().is_some(),
            "seed" => value
                .as_i64()
                .is_some_and(|n| (-1..=i32::MAX as i64).contains(&n)),
            _ => value
                .as_f64()
                .filter(|n| n.is_finite())
                .is_some_and(|n| match key.as_str() {
                    "temperature" => match id {
                        "anthropic" | "lmstudio_native_v1" => (0.0..=1.0).contains(&n),
                        "ollama_native" => n >= 0.0 && n <= f32::MAX as f64,
                        _ => (0.0..=2.0).contains(&n),
                    },
                    "top_p" | "min_p" => (0.0..=1.0).contains(&n),
                    "repeat_penalty" => n >= 0.0 && n <= f32::MAX as f64,
                    "presence_penalty" | "frequency_penalty" => (-2.0..=2.0).contains(&n),
                    _ => false,
                }),
        };
        if !valid {
            return Err(error("invalid_parameter"));
        }
        if result.insert(mapped.into(), value.clone()).is_some() {
            return Err(error("conflicting_parameter"));
        }
    }
    if id == "openai_compatible"
        && result.contains_key("max_tokens")
        && result.contains_key("max_completion_tokens")
    {
        return Err(error("conflicting_parameter"));
    }
    Ok(result)
}
fn valid_stop(value: &str) -> bool {
    !value.is_empty() && !value.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn run(id: &str, values: &[(&str, Value)]) -> AppResult<BTreeMap<String, Value>> {
        generation_wire_parameters(
            &ProtocolKey {
                protocol_id: id.into(),
                contract_revision: 1,
            },
            &values
                .iter()
                .map(|(k, v)| ((*k).into(), v.clone()))
                .collect(),
        )
    }
    #[test]
    fn native_mappings_preserve_zero_and_reject_aliases_and_invalid_domains() {
        for (id, mapped) in [
            ("openai_responses", "max_output_tokens"),
            ("gemini_generate_content", "maxOutputTokens"),
            ("ollama_native", "num_predict"),
            ("lmstudio_native_v1", "max_output_tokens"),
        ] {
            let output = run(id, &[("temperature", json!(0)), ("max_tokens", json!(7))]).unwrap();
            assert_eq!(output[mapped], 7);
            assert_eq!(output["temperature"], 0);
            for bad in [
                json!(-1),
                json!(0),
                json!(0.5),
                json!(u64::MAX),
                json!("7"),
                Value::Null,
            ] {
                assert_eq!(
                    run(id, &[("max_tokens", bad)]).unwrap_err().code,
                    "invalid_parameter"
                );
            }
            assert_eq!(
                run(id, &[("top_p", json!(1.01))]).unwrap_err().code,
                "invalid_parameter"
            );
            assert_eq!(
                run(id, &[("arbitrary", json!(true))]).unwrap_err().code,
                "unsupported_parameter"
            );
        }
        assert_eq!(
            run(
                "lmstudio_native_v1",
                &[("max_tokens", json!(1)), ("max_output_tokens", json!(2))]
            )
            .unwrap_err()
            .code,
            "conflicting_parameter"
        );
        assert!(
            run(
                "ollama_native",
                &[("temperature", json!(3)), ("seed", json!(-1))]
            )
            .is_ok()
        );
        assert_eq!(
            run("lmstudio_native_v1", &[("temperature", json!(1.1))])
                .unwrap_err()
                .code,
            "invalid_parameter"
        );
    }
    #[test]
    fn translation_is_generation_free_and_contract_revision_is_exact() {
        for id in [
            "azure_translator",
            "deepl_v2",
            "google_translate_v2",
            "google_translate_v3",
            "baidu_vip",
            "youdao_v3",
            "tencent_tokenhub_translation",
        ] {
            assert!(run(id, &[]).unwrap().is_empty());
            assert_eq!(
                run(id, &[("temperature", json!(0))]).unwrap_err().code,
                "unsupported_parameter"
            );
        }
        assert_eq!(
            generation_wire_parameters(
                &ProtocolKey {
                    protocol_id: "openai_compatible".into(),
                    contract_revision: 2
                },
                &BTreeMap::new()
            )
            .unwrap_err()
            .code,
            "unknown_protocol_version"
        );
    }
    #[test]
    fn pinned_chat_and_anthropic_text_contracts_do_not_grant_tools_or_model_capability() {
        assert!(
            run(
                "openai_compatible",
                &[
                    ("temperature", json!(0)),
                    ("presence_penalty", json!(-2)),
                    ("stop", json!(["END"]))
                ]
            )
            .is_ok()
        );
        assert!(
            run(
                "anthropic",
                &[
                    ("temperature", json!(0)),
                    ("max_tokens", json!(2048)),
                    ("stop_sequences", json!(["END"]))
                ]
            )
            .is_ok()
        );
        assert_eq!(
            run("anthropic", &[("temperature", json!(2))])
                .unwrap_err()
                .code,
            "invalid_parameter"
        );
        assert_eq!(
            run("openai_compatible", &[("temperature", json!(false))])
                .unwrap_err()
                .code,
            "invalid_parameter"
        );
        assert_eq!(
            run("openai_compatible", &[("tools", json!([]))])
                .unwrap_err()
                .code,
            "unsupported_parameter"
        );
        assert_eq!(
            run("anthropic", &[("stop", json!("END"))])
                .unwrap_err()
                .code,
            "unsupported_parameter"
        );
    }
}
