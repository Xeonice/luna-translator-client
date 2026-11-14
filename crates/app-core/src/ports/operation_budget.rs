//! Shared operation lifetime. This grants no resource or execution authority.
//! A short terminal database decision and explicit cancellation share one fence;
//! network/Vault/worker I/O must never run while that fence is held.
use crate::contracts::{AppError, AppResult};
use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::watch;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Active,
    Cancelled,
    Committed,
}
/// A stopped operation may persist its true terminal fact; this cannot authorize
/// a successful result after cancellation or expiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationTerminalDisposition {
    Active,
    Cancelled,
    TimedOut,
}
struct Inner {
    started: Instant,
    deadline_offset_ns: AtomicU64,
    state: watch::Sender<State>,
    terminal: Mutex<()>,
}
#[derive(Clone)]
pub struct OperationBudget {
    inner: Arc<Inner>,
}
fn failure(code: &str, message: &str) -> AppError {
    AppError::new(code, "operation_budget", message)
}
impl OperationBudget {
    pub fn new(duration: Duration) -> AppResult<Self> {
        let started = Instant::now();
        let _deadline = started
            .checked_add(duration)
            .ok_or_else(|| failure("operation_timeout", "操作时间预算无效"))?;
        let (state, _) = watch::channel(State::Active);
        Ok(Self {
            inner: Arc::new(Inner {
                started,
                deadline_offset_ns: AtomicU64::new(
                    u64::try_from(duration.as_nanos())
                        .map_err(|_| failure("operation_timeout", "操作时间预算无效"))?,
                ),
                state,
                terminal: Mutex::new(()),
            }),
        })
    }
    pub fn deadline(&self) -> Instant {
        self.inner.started
            + Duration::from_nanos(self.inner.deadline_offset_ns.load(Ordering::Acquire))
    }
    /// Preparation may discover a shorter saved policy. It may never reset the
    /// clock or extend an existing deadline. Call before starting material I/O.
    pub fn restrict_timeout_from_start(&self, duration: Duration) -> AppResult<()> {
        let _fence = self
            .inner
            .terminal
            .lock()
            .map_err(|_| failure("operation_state", "操作状态不可核验"))?;
        self.check()?;
        let offset = u64::try_from(duration.as_nanos())
            .map_err(|_| failure("operation_timeout", "操作时间预算无效"))?;
        self.inner
            .deadline_offset_ns
            .fetch_min(offset, Ordering::AcqRel);
        self.check()
    }
    /// Identity of the actual shared control, not a caller-provided timestamp.
    pub fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
    pub fn check(&self) -> AppResult<()> {
        if self.inner.terminal.is_poisoned() {
            return Err(failure("operation_state", "操作状态不可核验"));
        }
        match *self.inner.state.borrow() {
            State::Cancelled => return Err(failure("operation_cancelled", "操作已取消")),
            State::Committed => return Err(failure("operation_completed", "操作已完成")),
            State::Active => {}
        }
        if Instant::now() >= self.deadline() {
            return Err(failure("operation_timeout", "操作已超时"));
        }
        Ok(())
    }
    /// Never acquire this while holding a live-registry map or a database lock.
    /// If terminal commit won, cancellation cannot relabel its durable outcome.
    pub fn cancel(&self) {
        let _fence = self
            .inner
            .terminal
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let active = *self.inner.state.borrow() == State::Active;
        if active {
            self.inner.state.send_replace(State::Cancelled);
        }
    }
    pub fn is_committed(&self) -> bool {
        *self.inner.state.borrow() == State::Committed
    }
    pub async fn cancelled(&self) {
        let mut receiver = self.inner.state.subscribe();
        loop {
            if *receiver.borrow() != State::Active {
                return;
            }
            if receiver.changed().await.is_err() {
                return;
            }
        }
    }
    /// Order: terminal fence -> SQLite actor/transaction -> live-map read only.
    /// `commit` must check this budget immediately before its SQL COMMIT. That
    /// decision is the deadline linearization point; this does not promise to
    /// interrupt an OS fsync. It must not call cancel or recursively acquire the
    /// fence. A failed/rolled-back commit leaves cancellation/retry possible.
    pub fn with_terminal_fence<T>(&self, commit: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
        self.with_terminal_fence_for(OperationTerminalDisposition::Active, commit)
    }
    /// The repository must match this disposition to the actual terminal outcome
    /// and repeat check_terminal immediately before SQL COMMIT. Cancelled and
    /// timed-out records preserve facts; they do not regain execution permission.
    pub fn with_terminal_fence_for<T>(
        &self,
        disposition: OperationTerminalDisposition,
        commit: impl FnOnce() -> AppResult<T>,
    ) -> AppResult<T> {
        let _fence = self
            .inner
            .terminal
            .lock()
            .map_err(|_| failure("operation_state", "操作状态不可核验"))?;
        self.check_terminal(disposition)?;
        let result = commit();
        if result.is_ok() {
            self.inner.state.send_replace(State::Committed);
        }
        result
    }
    /// Lock-free under the terminal fence, including when called by the SQLite
    /// actor. Never use a stopped disposition to authorize a wire/Vault action.
    pub fn check_terminal(&self, disposition: OperationTerminalDisposition) -> AppResult<()> {
        if disposition == OperationTerminalDisposition::Active {
            return self.check();
        }
        if self.inner.terminal.is_poisoned() {
            return Err(failure("operation_state", "操作状态不可核验"));
        }
        let state = *self.inner.state.borrow();
        if state == State::Committed {
            return Err(failure("operation_completed", "操作已完成"));
        }
        let matches = match disposition {
            OperationTerminalDisposition::Active => unreachable!(),
            OperationTerminalDisposition::Cancelled => state == State::Cancelled,
            OperationTerminalDisposition::TimedOut => {
                state == State::Active && Instant::now() >= self.deadline()
            }
        };
        if matches {
            Ok(())
        } else {
            Err(failure(
                "operation_terminal_mismatch",
                "终态记录与实际取消或超时状态不一致",
            ))
        }
    }
}
impl fmt::Debug for OperationBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OperationBudget([operation-owned])")
    }
}
