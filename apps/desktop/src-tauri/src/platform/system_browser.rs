//! Native-only default browser intent. No URL IPC, shell interpolation, or embedded login.
use app_core::{
    contracts::{AppError, AppResult},
    ports::{google_authorization::GoogleBudget, google_interactive::*},
};
pub struct NativeGoogleSystemBrowser;
fn validated(intent: &GoogleSystemBrowserIntent) -> AppResult<String> {
    let url = tauri::Url::parse(intent.native_url()).map_err(|_| {
        AppError::new(
            "google_browser_intent_invalid",
            "google_interactive",
            "系统授权地址不可核验",
        )
    })?;
    if url.scheme() != "https"
        || url.host_str() != Some("accounts.google.com")
        || url.path() != "/o/oauth2/v2/auth"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::new(
            "google_browser_intent_invalid",
            "google_interactive",
            "只允许本次Google系统授权地址",
        ));
    }
    Ok(url.into())
}
impl GoogleSystemBrowser for NativeGoogleSystemBrowser {
    fn open<'a>(
        &'a self,
        intent: &'a GoogleSystemBrowserIntent,
        budget: &'a GoogleBudget,
    ) -> GoogleInteractiveFuture<'a, ()> {
        Box::pin(async move {
            budget.check()?;
            let url = validated(intent)?;
            #[cfg(target_os = "macos")]
            {
                let check = budget.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    check.check()?;
                    let result = std::process::Command::new("/usr/bin/open")
                        .arg(url)
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .map_err(|_| {
                            AppError::new(
                                "google_browser_open_failed",
                                "google_interactive",
                                "无法打开系统浏览器，请检查默认浏览器",
                            )
                        })?;
                    check.check()?;
                    if result.success() {
                        Ok(())
                    } else {
                        Err(AppError::new(
                            "google_browser_open_failed",
                            "google_interactive",
                            "系统浏览器未能打开授权页",
                        ))
                    }
                })
                .await
                .map_err(|_| {
                    AppError::new(
                        "google_browser_worker_failed",
                        "google_interactive",
                        "系统浏览器操作未能收尾",
                    )
                })?
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = url;
                Err(AppError::new(
                    "google_browser_platform_unsupported",
                    "google_interactive",
                    "此平台尚未安装系统授权浏览器适配器",
                ))
            }
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_fixed_google_system_authorization_origin_is_launchable() {
        let allowed=GoogleSystemBrowserIntent::from_broker_url("https://accounts.google.com/o/oauth2/v2/auth?client_id=public&code_challenge=controlled".into());
        assert!(validated(&allowed).is_ok());
        for value in [
            "http://accounts.google.com/o/oauth2/v2/auth",
            "https://accounts.google.com.attacker.invalid/o/oauth2/v2/auth",
            "https://name:password@accounts.google.com/o/oauth2/v2/auth",
            "https://accounts.google.com/o/oauth2/token",
            "https://accounts.google.com:8443/o/oauth2/v2/auth",
            "https://accounts.google.com/o/oauth2/v2/auth#fragment",
            "file:///tmp/login.html",
        ] {
            assert!(validated(&GoogleSystemBrowserIntent::from_broker_url(value.into())).is_err());
        }
        assert!(!format!("{allowed:?}").contains("code_challenge"));
    }
}
