//! Value projection for Hook events and bounded diagnostic text. No subscriber or relay ownership.
//! Queue-loss records still allocate an event ID and timestamp at their original call site.
use app_core::{
    contracts::*,
    domain::{capture::*, content_hash},
    ports::runtime::*,
};
use hook_client::NativeEvent;

pub(super) fn queue_loss(session: Option<String>, count: u64, code: &str) -> HookUpdate {
    let diagnostic = Diagnostic {
        code: "integrity_lost".into(),
        stage: "capture".into(),
        message: "接收队列发生已知丢失，正文已停止，请重新接入".into(),
        recoverable: true,
    };
    let Some(session_id) = session else {
        return HookUpdate::Health {
            session_id: None,
            diagnostic,
        };
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0);
    HookUpdate::Control {
        event: Box::new(HookEventInput {
            event_id: format!("queue-loss:{}", uuid::Uuid::new_v4()),
            operation_id: None,
            session_id: Some(session_id),
            source: None,
            wire_sequence: None,
            observed_at_ms: now,
            detail: HookStageInput {
                level: "error".into(),
                stage: "capture".into(),
                code: code.into(),
                message: "接收队列已报告丢失，已知数量保留；当前范围不完整".into(),
            },
            lost_count: Some(count.to_string()),
            integrity: "lost".into(),
        }),
        diagnostic: Some(diagnostic),
    }
}

pub(super) fn stage(stage: &str, code: &str, message: &str) -> HookStageInput {
    HookStageInput {
        level: "info".into(),
        stage: stage.into(),
        code: code.into(),
        message: message.into(),
    }
}
/// Native messages are useful diagnostic evidence, but are not trusted text.
/// Bound work as well as output, remove terminal/bidi controls, and conservatively
/// omit the entire suffix at credential/path markers (including quoted secrets).
/// Unlabelled arbitrary secrets cannot be inferred from natural-language logs.
pub(super) fn diagnostic_text(input: &str, limit: usize) -> String {
    let mut clean = String::new();
    let mut truncated = false;
    for (offset, c) in input.char_indices() {
        if offset >= 8192 {
            truncated = true;
            break;
        }
        if c.is_control() && !c.is_whitespace()
            || matches!(c, '\u{00ad}' | '\u{061c}' | '\u{180e}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
        {
            continue;
        }
        let c = if c.is_whitespace() { ' ' } else { c };
        if clean.len() + c.len_utf8() > 8192 {
            truncated = true;
            break;
        }
        clean.push(c);
    }
    let lower = clean.to_ascii_lowercase();
    let mut sensitive = None;
    for marker in [
        "bearer",
        "basic",
        "authorization",
        "proxy-authorization",
        "api_key",
        "api-key",
        "apikey",
        "x-api-key",
        "access_token",
        "refresh_token",
        "token",
        "password",
        "passwd",
        "secret",
        "credential",
    ] {
        for (offset, _) in lower.match_indices(marker) {
            let before = lower.as_bytes().get(offset.wrapping_sub(1));
            let after = lower.as_bytes().get(offset + marker.len());
            if before.is_none_or(|c| !c.is_ascii_alphanumeric() && *c != b'_')
                && after.is_some_and(|c| c.is_ascii_whitespace() || b"\"':=".contains(c))
            {
                sensitive = Some(sensitive.map_or(offset, |old: usize| old.min(offset)));
            }
        }
    }
    for marker in [
        "/users/",
        "/home/",
        "/private/",
        "/var/",
        "/volumes/",
        "/applications/",
        "/library/",
        "/tmp/",
        "\\\\",
    ] {
        if let Some(offset) = lower.find(marker) {
            sensitive = Some(sensitive.map_or(offset, |old| old.min(offset)));
        }
    }
    for (offset, window) in lower.as_bytes().windows(3).enumerate() {
        if window[0].is_ascii_alphabetic() && window[1] == b':' && matches!(window[2], b'\\' | b'/')
        {
            sensitive = Some(sensitive.map_or(offset, |old| old.min(offset)));
        }
    }
    // URL user-info is sensitive even when it has no named password field.
    for (offset, _) in lower.match_indices("://") {
        let authority = &lower[offset + 3..];
        let end = authority
            .find(['/', ' ', '?', '#'])
            .unwrap_or(authority.len());
        if authority[..end].contains('@') {
            let start = lower[..offset]
                .rfind(|c: char| !c.is_ascii_alphabetic())
                .map_or(0, |n| n + 1);
            sensitive = Some(sensitive.map_or(start, |old| old.min(start)));
        }
    }
    let suffix = if let Some(offset) = sensitive {
        clean.truncate(offset);
        " [敏感信息已隐藏]"
    } else if truncated || clean.len() > limit {
        " [内容已截断]"
    } else {
        ""
    };
    let budget = limit.saturating_sub(suffix.len());
    if clean.len() > budget {
        let mut end = budget;
        while !clean.is_char_boundary(end) {
            end -= 1;
        }
        clean.truncate(end);
    }
    format!("{}{suffix}", clean.trim())
}
fn native_label(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}
fn source_input(
    current: &HookSessionInput,
    stream: &hook_client::NativeStreamId,
    hook_code: String,
    hook_name: String,
) -> HookSourceInput {
    let hash = content_hash(format!(
        "{}:{}:{}:{}:{}",
        current.session_id, stream.pid, stream.address.0, stream.context.0, stream.context2.0
    ));
    HookSourceInput {
        source_id: format!("source-{hash}"),
        host_instance_id: current.broker_instance_id.clone(),
        hook_address_u64: stream.address.0.to_string(),
        context_u64: stream.context.0.to_string(),
        subcontext_u64: stream.context2.0.to_string(),
        hook_code,
        hook_name,
        encoding: "core-decoded-unicode".into(),
        stream_identity_sha256: hash,
        identity_extensions_json: format!("{{\"pid\":{}}}", stream.pid),
    }
}
pub(super) fn project_event(
    current: &HookSessionInput,
    event: hook_client::ClientEvent,
) -> HookUpdate {
    let mut source = None;
    let mut diagnostic = None;
    let mut lost_count = None;
    let mut integrity = if event.integrity_lost {
        "lost"
    } else {
        "intact"
    };
    let mut detail = match event.payload {
        Some(NativeEvent::Text {
            stream,
            hook_code,
            hook_name,
            text,
        }) => {
            return HookUpdate::Text(Box::new(HookTextInput {
                session_id: current.session_id.clone(),
                event_id: format!("{}:{}", current.session_id, event.sequence),
                sequence_u64: event.sequence.to_string(),
                source: source_input(current, &stream, hook_code, hook_name),
                text,
                source_language: "und".into(),
            }));
        }
        Some(NativeEvent::Ready {
            host_arch,
            abi_version,
            upstream_commit,
            abi_profile,
        }) => stage(
            "handshake",
            "host_ready",
            &format!(
                "Core宿主完成ABI握手 · architecture={} · ABI={abi_version} · upstream_commit={} · abi_profile={}",
                native_label(&host_arch),
                diagnostic_text(&upstream_commit, 256),
                diagnostic_text(&abi_profile, 512),
            ),
        ),
        Some(NativeEvent::Response { .. }) => {
            stage("handshake", "host_response", "Core宿主已回应控制请求")
        }
        Some(NativeEvent::Error { id, code, message }) => {
            if integrity != "lost" {
                integrity = "unknown";
            }
            diagnostic = Some(Diagnostic {
                code: "capture_failed".into(),
                stage: "capture".into(),
                message: "Core接入失败，请核对目标与资源后重试".into(),
                recoverable: true,
            });
            stage(
                "injection",
                "core_error",
                &format!(
                    "Core错误 · code={} · request={} · {}",
                    native_label(&code),
                    id.as_ref().map_or("无", |id| id.as_str()),
                    diagnostic_text(&message, 3072)
                ),
            )
        }
        Some(NativeEvent::ProcessConnected { pid }) => {
            if pid == current.windows_pid {
                diagnostic = Some(Diagnostic {
                    code: "capture_connected".into(),
                    stage: "capture".into(),
                    message: "游戏已与Core建立连接，等待正文候选".into(),
                    recoverable: true,
                });
            }
            stage(
                "handshake",
                "process_connected",
                &format!("进程已连接 · Windows PID={pid}"),
            )
        }
        Some(NativeEvent::ProcessDisconnected { pid }) => {
            diagnostic = Some(Diagnostic {
                code: "game_disconnected".into(),
                stage: "capture".into(),
                message: "游戏已退出或断开，历史保留".into(),
                recoverable: true,
            });
            stage(
                "disconnect",
                "process_disconnected",
                &format!("进程已断开 · Windows PID={pid}"),
            )
        }
        Some(NativeEvent::StreamCreated {
            stream,
            hook_code,
            hook_name,
        }) => {
            source = Some(source_input(current, &stream, hook_code, hook_name));
            stage("stream", "stream_created", "已发现文本流")
        }
        Some(NativeEvent::StreamDestroyed {
            stream,
            hook_code,
            hook_name,
        }) => {
            source = Some(source_input(current, &stream, hook_code, hook_name));
            stage("stream", "stream_destroyed", "文本流已结束，既有记录保留")
        }
        Some(NativeEvent::Log { kind, message }) => {
            let mut d = stage(
                "capture",
                "core_log",
                &format!(
                    "Core日志 · kind={kind} · {}",
                    diagnostic_text(&message, 3072)
                ),
            );
            // HOSTINFO Warning=1 and EmuWarning=2; keep unknown numeric kinds.
            if matches!(kind, 1 | 2) {
                d.level = "warning".into();
            }
            d
        }
        Some(NativeEvent::HookInserted {
            pid,
            address,
            hook_code,
        }) => stage(
            "injection",
            "hook_inserted",
            &format!(
                "Hook已安装 · Windows PID={pid} · address_u64={} · address_hex=0x{:X} · hook_code={}",
                address.0,
                address.0,
                diagnostic_text(&hook_code, 2048),
            ),
        ),
        Some(NativeEvent::InjectionFinished {
            pid,
            architecture,
            exit_code,
            accepted_by_helper,
        }) => {
            let mut result = stage(
                "injection",
                if accepted_by_helper {
                    "helper_accepted"
                } else {
                    "helper_rejected"
                },
                &format!(
                    "注入辅助程序完成 · Windows PID={pid} · architecture={} · exit_code={} · accepted_by_helper={accepted_by_helper}{}",
                    native_label(&architecture),
                    exit_code.map_or_else(|| "未提供".into(), |value| value.to_string()),
                    if accepted_by_helper {
                        "；仍需等待Core连接回调"
                    } else {
                        "；请求未被接受"
                    },
                ),
            );
            if !accepted_by_helper {
                result.level = "error".into();
            }
            result
        }
        Some(NativeEvent::Overflow {
            dropped_since_last, ..
        }) => {
            lost_count = Some(dropped_since_last.0.to_string());
            integrity = "lost";
            diagnostic = Some(Diagnostic {
                code: "integrity_lost".into(),
                stage: "capture".into(),
                message: "Core回调队列发生丢失，请重新接入".into(),
                recoverable: true,
            });
            stage(
                "capture",
                "queue_overflow",
                "Core报告回调队列溢出，当前接收范围不完整",
            )
        }
        None if event.status == "error" => {
            if integrity != "lost" {
                integrity = "unknown";
            }
            diagnostic = Some(Diagnostic {
                code: "capture_failed".into(),
                stage: "capture".into(),
                message: "宿主启动、注入或通信失败".into(),
                recoverable: true,
            });
            stage("injection", "host_failed", "宿主报告接入失败")
        }
        None if event.status == "stopped" || event.status == "disconnected" => {
            diagnostic = Some(Diagnostic {
                code: "game_disconnected".into(),
                stage: "capture".into(),
                message: "宿主或游戏已断开，历史保留".into(),
                recoverable: true,
            });
            stage("disconnect", "host_stopped", "宿主或游戏已停止连接")
        }
        None => stage("handshake", "host_status", "宿主状态已更新"),
    };
    if diagnostic
        .as_ref()
        .is_some_and(|d| d.code == "capture_failed" || d.code == "integrity_lost")
    {
        detail.level = "error".into();
    }
    HookUpdate::Control {
        event: Box::new(HookEventInput {
            event_id: format!("core:{}:{}", current.session_id, event.sequence),
            operation_id: None,
            session_id: Some(current.session_id.clone()),
            source,
            wire_sequence: Some(event.sequence.to_string()),
            observed_at_ms: i64::try_from(event.received_at_ms).unwrap_or(i64::MAX),
            detail,
            lost_count,
            integrity: integrity.into(),
        }),
        diagnostic,
    }
}
