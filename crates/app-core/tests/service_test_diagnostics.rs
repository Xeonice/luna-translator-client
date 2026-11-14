use app_core::domain::service_testing::safe_test_failure;
#[test]
fn ordinary_wire_failures_have_actionable_safe_diagnoses() {
    for (code, part) in [
        ("authentication_failed", "授权"),
        ("rate_limited", "配额"),
        ("connection_failed", "网络"),
        ("timeout", "超时"),
        ("invalid_structured_output", "输出模式"),
        ("missing_credential", "凭据"),
        ("google_invalid_grant", "重新登录"),
        ("offline_resource_authority_unavailable", "本机引擎"),
    ] {
        let result = safe_test_failure(code, Some("provider"));
        assert_eq!(result.code, code);
        assert_eq!(result.stage.as_deref(), Some("provider"));
        assert!(
            result.recovery.contains(part),
            "{code}: {}",
            result.recovery
        );
    }
}
#[test]
fn raw_vendor_or_secret_like_strings_are_never_reflected() {
    for code in [
        "https://server.invalid/?key=secret-marker",
        "secret-marker",
        "apikeylooksvalidlowercase123",
        "{\"token\":\"secret-marker\"}",
        "认证失败",
        "",
    ] {
        let result = safe_test_failure(code, Some("https://server.invalid/?token=secret-marker"));
        assert_eq!(result.code, "service_test_failed");
        assert!(result.stage.is_none());
        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("secret-marker"));
        assert!(!json.contains("apikeylooksvalid"));
        assert!(!json.contains("server.invalid"));
    }
}
