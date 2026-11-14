use app_core::{
    application::google_test_operations::LiveGoogleTestOperations,
    domain::{content_hash, model_config::credentials::*},
    ports::google_authorization::*,
};
use std::{num::NonZeroU32, time::Duration};
fn nz(v: u32) -> NonZeroU32 {
    NonZeroU32::new(v).unwrap()
}
fn request(id: &str) -> GoogleLeaseRequest {
    GoogleLeaseRequest {
        target: GoogleLeaseTarget::TestOperation(GoogleTestOperation {
            operation_id: id.into(),
            input_scope: GoogleTestInputScope::Sample,
            input_sha256: content_hash("今日は晴れです。"),
            request_semantics_sha256: content_hash("controlled actual test plan"),
        }),
        config_revision_id: "saved-source".into(),
        connection_id: "source-connection".into(),
        target_policy_sha256: content_hash("frozen destination/policy"),
        profile_generation: nz(1),
        server_instance_id: "server-one".into(),
    }
}
fn op(request: &GoogleLeaseRequest) -> &GoogleTestOperation {
    let GoogleLeaseTarget::TestOperation(op) = &request.target else {
        panic!()
    };
    op
}
fn budget() -> GoogleBudget {
    GoogleBudget::new(Duration::from_secs(60)).unwrap()
}
fn source(member: &str) -> GoogleServiceAccountLeaseIdentity {
    GoogleServiceAccountLeaseIdentity {
        pool_id: "true-selected-pool".into(),
        pool_revision: nz(2),
        member_id: member.into(),
        credential_version: CredentialVersionRef {
            credential_id: member.into(),
            credential_version_id: format!("vault-{member}"),
        },
        lifecycle_revision: 1,
        scopes_hash: google_scope_hash(),
    }
}
#[test]
fn a_valid_request_is_inactive_without_its_own_living_application_guard() {
    let registry = LiveGoogleTestOperations::default();
    let request = request("test-one");
    assert_eq!(
        registry.check(&request, op(&request)).unwrap_err().code,
        "google_test_operation_inactive"
    );
    let owner = registry.register(request.clone(), budget(), None).unwrap();
    let cancellation = owner.budget();
    registry.check(&request, op(&request)).unwrap();
    assert_eq!(
        registry
            .service_account_source(&request, op(&request))
            .unwrap_err()
            .code,
        "google_test_source_unavailable"
    );
    drop(owner);
    assert_eq!(
        cancellation.check().unwrap_err().code,
        "authorization_cancelled"
    );
    assert_eq!(
        registry.check(&request, op(&request)).unwrap_err().code,
        "google_test_operation_inactive"
    );
}
#[test]
fn frozen_test_input_profile_target_and_source_cannot_be_rebound_or_cross_used() {
    let registry = LiveGoogleTestOperations::default();
    let a = request("test-a");
    let b = request("test-b");
    let selected_a = source("member-a");
    let selected_b = source("member-b");
    let a_owner = registry
        .register(a.clone(), budget(), Some(selected_a.clone()))
        .unwrap();
    let b_owner = registry
        .register(b.clone(), budget(), Some(selected_b.clone()))
        .unwrap();
    assert_eq!(
        registry.service_account_source(&a, op(&a)).unwrap(),
        selected_a
    );
    assert_eq!(
        registry.service_account_source(&b, op(&b)).unwrap(),
        selected_b
    );
    for mutation in 0..6 {
        let mut wrong = a.clone();
        match mutation {
            0 => wrong.config_revision_id = "another-source".into(),
            1 => wrong.target_policy_sha256 = content_hash("different actual target"),
            2 => wrong.profile_generation = nz(2),
            3 => wrong.server_instance_id = "other-server".into(),
            4 => {
                if let GoogleLeaseTarget::TestOperation(op) = &mut wrong.target {
                    op.input_sha256 = content_hash("game text not consented to");
                }
            }
            _ => {
                if let GoogleLeaseTarget::TestOperation(op) = &mut wrong.target {
                    op.request_semantics_sha256 = content_hash("different plan");
                }
            }
        }
        assert_eq!(
            registry.check(&wrong, op(&wrong)).unwrap_err().code,
            "google_test_operation_mismatch"
        );
    }
    assert_eq!(
        registry.check(&a, op(&b)).unwrap_err().code,
        "google_test_operation_mismatch"
    );
    drop(a_owner);
    registry.check(&b, op(&b)).unwrap();
    assert_eq!(
        registry.service_account_source(&b, op(&b)).unwrap(),
        selected_b
    );
    drop(b_owner);
}
#[test]
fn duplicate_and_cancelled_operations_never_replace_a_live_owner_or_gain_pool_authority() {
    let registry = LiveGoogleTestOperations::default();
    let request = request("test-one");
    let owner = registry
        .register(request.clone(), budget(), Some(source("member-a")))
        .unwrap();
    assert_eq!(
        registry
            .register(request.clone(), budget(), Some(source("member-b")))
            .unwrap_err()
            .code,
        "service_test_operation_conflict"
    );
    assert_eq!(
        registry
            .service_account_source(&request, op(&request))
            .unwrap(),
        source("member-a")
    );
    owner.budget().cancel();
    assert_eq!(
        registry.check(&request, op(&request)).unwrap_err().code,
        "authorization_cancelled"
    );
    assert_eq!(
        registry
            .service_account_source(&request, op(&request))
            .unwrap_err()
            .code,
        "authorization_cancelled"
    );
    let expired = GoogleBudget::new(Duration::ZERO).unwrap();
    assert_eq!(
        registry
            .register(super_request(), expired, None)
            .unwrap_err()
            .code,
        "authorization_timeout"
    );
    let mut attempt = super_request();
    attempt.target = GoogleLeaseTarget::Attempt {
        attempt_id: "true-attempt".into(),
        task_id: "true-task".into(),
    };
    assert_eq!(
        registry.register(attempt, budget(), None).unwrap_err().code,
        "google_test_operation_required"
    );
}
fn super_request() -> GoogleLeaseRequest {
    request("second-test")
}

#[test]
fn a_preparation_reservation_can_be_cancelled_without_granting_google_authority() {
    use app_core::ports::operation_budget::OperationBudget;
    let registry = LiveGoogleTestOperations::default();
    let budget = OperationBudget::new(Duration::from_secs(30)).unwrap();
    let reservation = registry
        .begin_service("preparing-only".into(), nz(3), budget.clone())
        .unwrap();
    assert_eq!(
        registry
            .cancel_service("preparing-only", nz(4))
            .unwrap_err()
            .code,
        "profile_generation_conflict"
    );
    budget.check().unwrap();
    let fake = request("preparing-only");
    assert_eq!(
        registry.check(&fake, op(&fake)).unwrap_err().code,
        "service_test_preparing"
    );
    registry.cancel_service("preparing-only", nz(3)).unwrap();
    assert_eq!(budget.check().unwrap_err().code, "operation_cancelled");
    drop(reservation);
    let next = OperationBudget::new(Duration::from_secs(30)).unwrap();
    let next = registry
        .begin_service("preparing-only".into(), nz(3), next)
        .unwrap();
    drop(next);
}
