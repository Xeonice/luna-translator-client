//! Shared original-budget and physical-join helpers, not a business scheduler.
use super::failure;
use app_core::{
    contracts::{AppError, AppResult},
    ports::operation_budget::OperationBudget,
};
use std::future::Future;
pub(super) async fn within_budget<T>(
    budget: &OperationBudget,
    future: impl Future<Output = AppResult<T>>,
) -> AppResult<T> {
    budget.check()?;
    tokio::select! {biased;
        _=budget.cancelled()=>Err(budget.check().err().unwrap_or_else(||failure("operation_state","操作控制状态不一致"))),
        _=tokio::time::sleep_until(budget.deadline().into())=>Err(budget.check().err().unwrap_or_else(||failure("operation_timeout","操作已超时"))),
        result=future=>{budget.check()?;result},
    }
}
pub(super) async fn blocking<T: Send + 'static, G: Send + 'static>(
    budget: &OperationBudget,
    physical: G,
    action: impl FnOnce() -> AppResult<T> + Send + 'static,
    join_error: fn() -> AppError,
) -> AppResult<T> {
    budget.check()?;
    let checked = budget.clone();
    let job = tokio::task::spawn_blocking(move || {
        let _physical = physical;
        checked.check()?;
        let result = action();
        checked.check()?;
        result
    });
    within_budget(budget, async move { job.await.map_err(|_| join_error())? }).await
}
/// The action owns its original terminal fence. A successful commit makes the
/// budget Committed, so an Active check after its physical return would be false.
pub(super) async fn terminal_blocking<T: Send + 'static, G: Send + 'static>(
    physical: G,
    action: impl FnOnce() -> AppResult<T> + Send + 'static,
    join_error: fn() -> AppError,
) -> AppResult<T> {
    tokio::task::spawn_blocking(move || {
        let _physical = physical;
        action()
    })
    .await
    .map_err(|_| join_error())?
}
