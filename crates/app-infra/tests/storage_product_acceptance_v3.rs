//! Schema-three HTTP identity acceptance retains the complete original storage
//! target. All component cases execute in this process before the same-run
//! receipt assembler; no historical receipt can satisfy these obligations.
#![allow(clippy::duplicate_mod)]
#[path = "provider_credential_envelopes.rs"]
mod provider_credential_envelopes;
#[path = "provider_gateway_identity.rs"]
mod provider_gateway_identity;
#[path = "storage_http_identity_crash.rs"]
mod storage_http_identity_crash;
#[path = "storage_http_identity_v3.rs"]
mod storage_http_identity_v3;
#[path = "storage_product_acceptance_v2.rs"]
mod storage_product_acceptance_v2;
