//! Schema-two acceptance adds scoped-preference semantics to the preserved
//! complete storage target. The original v1 target and its registration remain
//! unchanged. Serial test execution runs the scope module before the complete
//! target's final same-run receipt assembler.
#![allow(clippy::duplicate_mod)]
#[path = "scoped_preferences.rs"]
mod scoped_preferences_integration;
#[path = "storage_product_acceptance.rs"]
mod storage_product_acceptance;
