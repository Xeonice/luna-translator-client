//! Google bearer execution for a genuine admitted codec3 target. Authorization
//! has no independently configured destination: both SQL and this adapter bind
//! the complete frozen ConnectionV3. Legacy tests cannot manufacture an attempt.
use super::{
    adapters::dispatch, failure, google_authorization::GoogleAuthorizationService,
    typed_http::HttpTransport,
};
use app_core::{
    contracts::AppResult,
    domain::{service_dispatch::ServiceTaskDispatch, service_execution::ServiceExecutionTarget},
    ports::{ProductRepository, google_authorization::*},
};
use std::{num::NonZeroU32, sync::Arc, time::Duration};

/// User-saved HTTPS destinations are explicit configuration intent. There is no
/// extra domain approval switch. Google token/revoke hosts are independently
/// fixed by the token transport, and authenticated redirects are never followed.
pub(super) fn validate_destination(endpoint: &str) -> AppResult<()> {
    let url = reqwest::Url::parse(endpoint)
        .map_err(|_| failure("invalid_google_destination", "Google翻译服务地址无效"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port() == Some(0)
    {
        return Err(failure(
            "invalid_google_destination",
            "Google短期授权仅发送到明确保存的HTTPS服务地址",
        ));
    }
    Ok(())
}

struct AttemptBudget(GoogleBudget);
impl Drop for AttemptBudget {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
pub(super) struct AuthorizedGoogleAttempt {
    pub lease: super::google_authorization::GoogleAccessLease,
    _budget: AttemptBudget,
}

fn request(task: &ServiceTaskDispatch) -> AppResult<GoogleLeaseRequest> {
    let ServiceExecutionTarget::Http { plan } = &task.plan.target else {
        return Err(failure(
            "google_target_mismatch",
            "Google授权必须属于HTTP服务",
        ));
    };
    let connection = &plan.connection;
    if connection.protocol != "google_translate_v3"
        || !matches!(
            connection.auth_mode.as_str(),
            "google_service_account" | "google_oauth"
        )
    {
        return Err(failure(
            "google_target_mismatch",
            "Google授权类型与明确协议不符",
        ));
    }
    validate_destination(
        connection
            .endpoint
            .as_deref()
            .ok_or_else(|| failure("missing_endpoint", "Google翻译服务地址尚未填写"))?,
    )?;
    let request = GoogleLeaseRequest {
        target: GoogleLeaseTarget::Attempt {
            attempt_id: task.attempt_id.clone(),
            task_id: task.task_id.clone(),
        },
        config_revision_id: task.plan.identity.configuration_revision.clone(),
        connection_id: connection.connection_id.clone(),
        target_policy_sha256: google_target_policy_sha256(connection)?,
        profile_generation: NonZeroU32::new(task.profile_generation)
            .ok_or_else(|| failure("profile_generation_conflict", "Google授权配置档代次无效"))?,
        server_instance_id: task.server_instance_id.clone(),
    };
    request.validate()?;
    Ok(request)
}

pub(super) async fn execute(
    service: &GoogleAuthorizationService,
    repository: &dyn ProductRepository,
    task: &ServiceTaskDispatch,
    transport: &HttpTransport,
    on_delta: Arc<dyn Fn(String) + Send + Sync>,
) -> AppResult<String> {
    let authorized = authorize(service, repository, task).await?;
    let lease = &authorized.lease;
    let ServiceExecutionTarget::Http { plan } = &task.plan.target else {
        unreachable!("checked typed HTTP target")
    };
    let auth = lease.authorization()?;
    let output = tokio::select! {
        biased;
        error = lease.invalidated() => return Err(error),
        result = dispatch::execute_with_headers(
            &plan.request, &transport.connection, task.plan.identity.purpose,
            &auth, &transport.options, &transport.headers,
            move |text| on_delta(text.to_owned()),
        ) => result?,
    };
    lease.check_current_provenance()?;
    repository.validate_service_dispatch(task)?;
    Ok(output.text)
}

pub(super) async fn authorize(
    service: &GoogleAuthorizationService,
    repository: &dyn ProductRepository,
    task: &ServiceTaskDispatch,
) -> AppResult<AuthorizedGoogleAttempt> {
    let request = request(task)?;
    repository.validate_service_dispatch(task)?;
    let budget = AttemptBudget(GoogleBudget::new(Duration::from_millis(u64::from(
        task.plan.policy.execution.timeout_ms,
    )))?);
    let lease = service.lease(request, budget.0.clone()).await?;
    // Vault/token I/O cannot grant a late, stopped or replaced attempt authority.
    repository.validate_service_dispatch(task)?;
    lease.check_current_provenance()?;
    Ok(AuthorizedGoogleAttempt {
        lease,
        _budget: budget,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_google_destination_rejects_plaintext_and_embedded_material() {
        for value in [
            "http://127.0.0.1:1234",
            "https://user:secret@example.org/v3",
            "https://example.org/v3?key=secret",
            "https://example.org/v3#token",
            "ftp://example.org",
            "https://example.org:0",
            "not a url",
        ] {
            assert_eq!(
                validate_destination(value).unwrap_err().code,
                "invalid_google_destination",
                "{value}"
            );
        }
        // A user may explicitly configure a secure translation proxy. Tests of
        // this parser are not live provider or endpoint-ownership acceptance.
        validate_destination("https://translation.example.org/v3").unwrap();
        validate_destination("https://translation.googleapis.com").unwrap();
    }
}
