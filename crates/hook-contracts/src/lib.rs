//! Versioned JSONL boundary for a single, headless LunaHook host process.
//!
//! ```
//! use hook_contracts::{Request, Command};
//! let request: Request = serde_json::from_str(
//!     r#"{"v":2,"id":"health-1","method":"ping","params":{}}"#,
//! )?;
//! assert!(matches!(request.command, Command::Ping(_)));
//! # Ok::<(), serde_json::Error>(())
//! ```

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

pub const PROTOCOL_VERSION: u16 = 2;
pub const CORE_ABI_VERSION: u32 = 1;
pub const CORE_ABI_PROFILE: &str = "lunahooks-core-abi1-cdecl-seven";
pub const CORE_COMMIT: &str = "e9242ebac32a09e911f306b7c90714891c217302";
pub const MAX_REQUEST_BYTES: usize = 65_536;

/// Full-width integers cross the JSON boundary as canonical decimal strings.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DecimalU64(pub u64);

impl Serialize for DecimalU64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for DecimalU64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.is_empty()
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return Err(de::Error::custom(
                "expected canonical unsigned decimal string",
            ));
        }
        value.parse::<u64>().map(Self).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct RequestId(String);

impl RequestId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RequestId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let id = String::deserialize(deserializer)?;
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
        {
            return Err(de::Error::custom(
                "id must contain 1..=128 ASCII letters, digits, -, _, . or :",
            ));
        }
        Ok(Self(id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Request {
    pub v: u16,
    pub id: RequestId,
    #[serde(flatten)]
    pub command: Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum Command {
    Ping(EmptyParams),
    Status(EmptyParams),
    Attach(AttachParams),
    Detach(DetachParams),
    Shutdown(EmptyParams),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct EmptyParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachParams {
    pub pid: u32,
    pub expected_image_path: String,
    pub expected_creation_time: DecimalU64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetachParams {
    pub pid: u32,
}

impl<'de> Deserialize<'de> for Request {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawRequest {
            v: u16,
            id: RequestId,
            method: String,
            params: Box<serde_json::value::RawValue>,
        }
        let raw = RawRequest::deserialize(deserializer)?;
        if raw.v != PROTOCOL_VERSION {
            return Err(de::Error::custom(format!(
                "unsupported protocol version {}; expected {PROTOCOL_VERSION}",
                raw.v
            )));
        }
        fn params<T: de::DeserializeOwned, E: de::Error>(
            value: &serde_json::value::RawValue,
        ) -> Result<T, E> {
            serde_json::from_str(value.get()).map_err(E::custom)
        }
        let command = match raw.method.as_str() {
            "ping" => Command::Ping(params(&raw.params)?),
            "status" => Command::Status(params(&raw.params)?),
            "shutdown" => Command::Shutdown(params(&raw.params)?),
            "attach" => {
                let value: AttachParams = params(&raw.params)?;
                let path = &value.expected_image_path;
                let bytes = path.as_bytes();
                let absolute = path.starts_with(r"\\")
                    || (bytes.len() >= 3
                        && bytes[0].is_ascii_alphabetic()
                        && bytes[1] == b':'
                        && matches!(bytes[2], b'\\' | b'/'));
                if value.pid == 0
                    || value.expected_creation_time.0 == 0
                    || path.contains('\0')
                    || !absolute
                {
                    return Err(de::Error::custom(
                        "attach requires pid > 0, absolute Windows image path and nonzero creation FILETIME",
                    ));
                }
                Command::Attach(value)
            }
            "detach" => {
                let value: DetachParams = params(&raw.params)?;
                if value.pid == 0 {
                    return Err(de::Error::custom("detach requires pid > 0"));
                }
                Command::Detach(value)
            }
            _ => return Err(de::Error::custom("unknown method")),
        };
        Ok(Self {
            v: raw.v,
            id: raw.id,
            command,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamId {
    pub pid: u32,
    pub address: DecimalU64,
    pub context: DecimalU64,
    pub context2: DecimalU64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Architecture {
    X86,
    X64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetStage {
    Pending,
    Connected,
    TimedOut,
    Failed,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetStatus {
    pub pid: u32,
    pub stage: TargetStage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub v: u16,
    #[serde(flatten)]
    pub payload: Event,
}

impl Envelope {
    pub fn new(payload: Event) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Ready {
        host_arch: Architecture,
        abi_version: u32,
        upstream_commit: String,
        abi_profile: String,
    },
    Response {
        id: RequestId,
        result: ResponseResult,
    },
    Error {
        id: Option<RequestId>,
        code: ErrorCode,
        message: String,
    },
    ProcessConnected {
        pid: u32,
    },
    ProcessDisconnected {
        pid: u32,
    },
    StreamCreated {
        stream: StreamId,
        hook_code: String,
        hook_name: String,
    },
    StreamDestroyed {
        stream: StreamId,
        hook_code: String,
        hook_name: String,
    },
    Text {
        stream: StreamId,
        hook_code: String,
        hook_name: String,
        text: String,
    },
    Log {
        kind: i32,
        message: String,
    },
    HookInserted {
        pid: u32,
        address: DecimalU64,
        hook_code: String,
    },
    InjectionFinished {
        pid: u32,
        architecture: Architecture,
        exit_code: Option<i32>,
        accepted_by_helper: bool,
    },
    Overflow {
        source: String,
        dropped_since_last: DecimalU64,
        dropped_total: DecimalU64,
        integrity_lost: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ResponseResult {
    Pong,
    Snapshot {
        targets: Vec<TargetStatus>,
        integrity_lost: bool,
    },
    Pending {
        pid: u32,
        connection_timeout_ms: u32,
    },
    DetachRequested {
        pid: u32,
    },
    ShuttingDown {
        deadline_ms: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    UnsupportedPlatform,
    InvalidRequest,
    DuplicateRequest,
    RuntimeInvalid,
    HostInitialization,
    TargetIdentity,
    UnsupportedArchitecture,
    AlreadyAttached,
    NotAttached,
    Busy,
    InjectionFailed,
    ConnectionTimeout,
    ShuttingDown,
    CallbackInvalid,
    ShutdownTimeout,
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_versions_methods_parameters_and_bad_identity() {
        for input in [
            r#"{"v":1,"id":"a","method":"ping","params":{}}"#,
            r#"{"v":2,"id":"a","method":"inject","params":{}}"#,
            r#"{"v":2,"id":"a","method":"ping","params":{"extra":1}}"#,
            r#"{"v":2,"id":"","method":"ping","params":{}}"#,
            r#"{"v":2,"id":"a","method":"detach","params":{"pid":0}}"#,
            r#"{"v":2,"id":"a","method":"attach","params":{"pid":5,"expected_image_path":"relative.exe","expected_creation_time":"123"}}"#,
            r#"{"v":2,"id":"a","method":"attach","params":{"pid":5,"expected_image_path":"C:\\game.exe","expected_creation_time":123}}"#,
            r#"{"v":2,"id":"a","method":"ping","params":{},"extra":1}"#,
            r#"{"v":2,"id":"a","method":"detach","params":{"pid":1,"pid":2}}"#,
        ] {
            assert!(serde_json::from_str::<Request>(input).is_err(), "{input}");
        }
    }

    #[test]
    fn integers_above_javascript_precision_roundtrip_without_loss() {
        let input = r#"{"v":2,"id":"a","method":"attach","params":{"pid":5,"expected_image_path":"C:\\game.exe","expected_creation_time":"18446744073709551615"}}"#;
        let request: Request = serde_json::from_str(input).expect("valid request");
        let Command::Attach(params) = &request.command else {
            panic!("attach expected")
        };
        assert_eq!(params.expected_creation_time.0, u64::MAX);
        let output = serde_json::to_string(&request).expect("serializable request");
        assert_eq!(
            serde_json::from_str::<Request>(&output).expect("roundtrip"),
            request
        );
        assert!(output.contains(r#""18446744073709551615""#));
        for invalid in ["01", "+1", "-1", "18446744073709551616", ""] {
            assert!(serde_json::from_value::<DecimalU64>(serde_json::json!(invalid)).is_err());
        }
    }

    #[test]
    fn text_event_preserves_full_width_addresses() {
        let event = Envelope::new(Event::Text {
            stream: StreamId {
                pid: 7,
                address: DecimalU64(u64::MAX),
                context: DecimalU64(1 << 60),
                context2: DecimalU64(0),
            },
            hook_code: "HS@0".into(),
            hook_name: "MAGES2".into(),
            text: "テスト".into(),
        });
        let value = serde_json::to_value(event).expect("event serializes");
        assert_eq!(value["stream"]["address"], u64::MAX.to_string());
        assert_eq!(value["stream"]["context"], (1_u64 << 60).to_string());
        let decoded: Envelope = serde_json::from_value(value).expect("event roundtrip");
        assert!(matches!(decoded.payload, Event::Text { .. }));
    }
}
